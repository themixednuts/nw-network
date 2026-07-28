use std::{
    collections::BTreeSet,
    env, fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use nw_resources::EmbeddedResource;
use nw_serialize_codegen::{
    CodegenContext, NETWORK_RUST_EMITTER_VERSION, NetworkFieldOverrideFile,
    NetworkMessageSignature, NetworkReplicatedStateEmitOptions, NetworkRustEmitter, NetworkSchema,
    NetworkSerializeRootPlanner, SerializeCodegenItem, SerializeCodegenItemKind,
    SerializeCodegenRootMode, SerializeCodegenRootSelection, SerializeCodegenUnit,
    SerializeContextCompileInputs, SerializeContextCompiler, SerializeContextDocument,
    complete_known_missing_reflected_bodies, module_descriptor_capture, module_descriptors_root,
    module_name_from_resource_name, resolve_codegen_root_type_ids, rust_type_ident,
};
use serde::Deserialize;
use serde_json::Value;

const CODEGEN_VERSION: &str = "nw-network-generated-payloads-v3";

const MANUAL_SOURCE_MARSHALERS: &[&str] = &[
    "AfflictionData",
    "DyeData",
    "GDEID",
    "RecipeCooldownData",
    "RemoteServerContextRef",
    "RemoteServerFacetRef<GameModeParticipantComponentServerFacet >",
    "RemoteServerFacetRef<HousingPlotComponentServerFacet >",
    "RemoteServerGDERef",
    "RemoteTypelessServerFacetRef",
    "TimePoint",
    "WallClockTimePoint",
];

fn main() -> Result<()> {
    let manifest_dir =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").context("CARGO_MANIFEST_DIR")?);
    let build_script = manifest_dir.join("build.rs");
    let generated_state_denylist_file = manifest_dir.join("codegen/generated-state-denylist.json");
    let network_field_overrides_file = manifest_dir.join("codegen/network-field-overrides.json");
    let generated_type_selection_file =
        manifest_dir.join("crates/nw-network-types/codegen/selection.json");
    let network_schema_file =
        manifest_dir.join("crates/nw-network-types/codegen/network-schema.json");
    let message_signatures_file =
        manifest_dir.join("crates/nw-network-types/codegen/message-signatures.json");

    rerun_if_changed(&build_script);
    rerun_if_changed(&generated_state_denylist_file);
    rerun_if_changed(&network_field_overrides_file);
    rerun_if_changed(&generated_type_selection_file);
    rerun_if_changed(&network_schema_file);
    rerun_if_changed(&message_signatures_file);

    let input_hash = input_hash(
        &build_script,
        &generated_state_denylist_file,
        &network_field_overrides_file,
        &generated_type_selection_file,
        &network_schema_file,
        &message_signatures_file,
    )?;
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").context("OUT_DIR")?);
    let output_root = stable_generated_root(&out_dir, "nw-network")?;
    println!(
        "cargo:rustc-env=NW_NETWORK_GENERATED_DIR={}",
        output_root.display()
    );
    let stamp_path = output_root.join(".generated-states-input-hash");
    let state_source_path = output_root.join("generated_states.rs");
    let state_report_path = output_root.join("generated-states.rust-report.json");
    let message_source_path = output_root.join("generated_messages.rs");
    let message_report_path = output_root.join("generated-messages.rust-report.json");
    let conversion_source_path = output_root.join("generated_conversions.rs");
    let conversion_report_path = output_root.join("generated-conversions.rust-report.json");

    if state_source_path.is_file()
        && state_report_path.is_file()
        && message_source_path.is_file()
        && message_report_path.is_file()
        && conversion_source_path.is_file()
        && conversion_report_path.is_file()
        && fs::read_to_string(&stamp_path).is_ok_and(|stamp| stamp == input_hash)
    {
        return Ok(());
    }

    let mut network_schema = load_network_schema(&network_schema_file)?;
    let message_signatures = load_message_signatures(&message_signatures_file)?;
    let signature_report = network_schema.merge_message_signatures(
        &message_signatures,
        Some(message_signatures_file.display().to_string()),
    );
    if signature_report.unmatched_message_count != 0
        || signature_report.ambiguous_message_count != 0
        || signature_report.field_index_mismatch_count != 0
        || signature_report.field_name_conflict_count != 0
    {
        bail!(
            "network message signatures did not resolve cleanly: {} unmatched message(s), {} ambiguous message(s), {} field-index mismatch(es), {} field-name conflict(s)",
            signature_report.unmatched_message_count,
            signature_report.ambiguous_message_count,
            signature_report.field_index_mismatch_count,
            signature_report.field_name_conflict_count
        );
    }
    let network_field_overrides = load_network_field_overrides(&network_field_overrides_file)?;
    let override_report = network_schema.merge_field_overrides(
        &network_field_overrides,
        Some(network_field_overrides_file.display().to_string()),
    );
    if override_report.unmatched_type_count != 0
        || override_report.ambiguous_type_count != 0
        || override_report.unmatched_field_count != 0
        || override_report.ambiguous_field_count != 0
    {
        bail!(
            "network field overrides did not resolve cleanly: {} unmatched type(s), {} ambiguous type(s), {} unmatched field(s), {} ambiguous field(s)",
            override_report.unmatched_type_count,
            override_report.ambiguous_type_count,
            override_report.unmatched_field_count,
            override_report.ambiguous_field_count
        );
    }
    let generated_types =
        selected_generated_type_unit(&generated_type_selection_file, &mut network_schema)
            .context("compile selected generated network data types")?;
    network_schema.merge_serialize_codegen_unit(
        &generated_types,
        Some(generated_type_selection_file.display().to_string()),
    );
    let generated_state_denylist =
        GeneratedStateDenylistFile::from_path(&generated_state_denylist_file)?;
    let replicated_state_type_indices = replicated_state_type_indices(&network_schema);
    let denied_type_indices = generated_state_denylist
        .denied_type_indices
        .into_iter()
        .collect::<BTreeSet<_>>();
    let registered_type_indices = replicated_state_type_indices
        .iter()
        .copied()
        .filter(|type_index| !denied_type_indices.contains(type_index))
        .collect::<Vec<_>>();
    let output = NetworkRustEmitter::emit_replicated_states_with_options(
        &network_schema,
        replicated_state_type_indices,
        NetworkReplicatedStateEmitOptions::register_only(registered_type_indices),
    )
    .context("emit generated replicated states")?;
    let message_output =
        NetworkRustEmitter::emit_messages(&network_schema).context("emit generated messages")?;
    let required_source_struct_types =
        required_source_struct_type_names(output.source.as_str(), message_output.source.as_str());
    let generated_type_index = generated_types.index();
    let mut required_conversion_type_ids = BTreeSet::new();
    for item in generated_types.items.iter().filter(|item| {
        item.kind == SerializeCodegenItemKind::Struct
            && required_source_struct_types
                .contains(&rust_type_ident(source_name_leaf(&item.source_name)))
            && !has_manual_source_marshaler(item)
    }) {
        generated_type_index
            .extend_transitive_dependency_type_ids(item, &mut required_conversion_type_ids);
    }
    let conversion_items = generated_types.items.iter().filter(|item| match item.kind {
        SerializeCodegenItemKind::Enum => true,
        SerializeCodegenItemKind::Struct => {
            required_conversion_type_ids.contains(&item.source_type_id)
                && !has_manual_source_marshaler(item)
        }
    });
    let conversion_output = NetworkRustEmitter::emit_marshaler_conversions(conversion_items)
        .context("emit generated marshaler conversions")?;

    fs::create_dir_all(&output_root)
        .with_context(|| format!("create {}", output_root.display()))?;
    write_file_if_changed(&state_source_path, output.source.as_bytes())
        .with_context(|| format!("write {}", state_source_path.display()))?;
    let mut report =
        serde_json::to_string_pretty(&output.report).context("serialize generated state report")?;
    report.push('\n');
    write_file_if_changed(&state_report_path, report.as_bytes())
        .with_context(|| format!("write {}", state_report_path.display()))?;
    write_file_if_changed(&message_source_path, message_output.source.as_bytes())
        .with_context(|| format!("write {}", message_source_path.display()))?;
    let mut message_report = serde_json::to_string_pretty(&message_output.report)
        .context("serialize generated message report")?;
    message_report.push('\n');
    write_file_if_changed(&message_report_path, message_report.as_bytes())
        .with_context(|| format!("write {}", message_report_path.display()))?;
    write_file_if_changed(&conversion_source_path, conversion_output.source.as_bytes())
        .with_context(|| format!("write {}", conversion_source_path.display()))?;
    let mut conversion_report = serde_json::to_string_pretty(&conversion_output.report)
        .context("serialize generated conversion report")?;
    conversion_report.push('\n');
    write_file_if_changed(&conversion_report_path, conversion_report.as_bytes())
        .with_context(|| format!("write {}", conversion_report_path.display()))?;
    write_file_if_changed(&stamp_path, input_hash.as_bytes())
        .with_context(|| format!("write {}", stamp_path.display()))?;

    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct GeneratedStateDenylistFile {
    denied_type_indices: Vec<u32>,
}

impl GeneratedStateDenylistFile {
    fn from_path(path: &Path) -> Result<Self> {
        let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
        serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))
    }
}

fn load_network_field_overrides(path: &Path) -> Result<NetworkFieldOverrideFile> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))
}

fn load_message_signatures(path: &Path) -> Result<Vec<NetworkMessageSignature>> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let root = serde_json::from_slice::<Value>(&bytes)
        .with_context(|| format!("parse {}", path.display()))?;
    if root.is_array() {
        return serde_json::from_value(root).with_context(|| format!("parse {}", path.display()));
    }
    if let Some(messages) = root.get("messages") {
        return serde_json::from_value(messages.clone())
            .with_context(|| format!("parse {}", path.display()));
    }
    bail!(
        "message signatures JSON {} must be an array or an object with `messages`",
        path.display()
    )
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct GeneratedTypeSelectionFile {
    roots: Vec<RootEntry>,
}

impl GeneratedTypeSelectionFile {
    fn from_path(path: &Path) -> Result<Self> {
        let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
        serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))
    }

    fn root_specs(&self) -> Vec<String> {
        self.roots.iter().map(RootEntry::spec).collect()
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum RootEntry {
    Spec(String),
    Object {
        root: String,
        #[allow(dead_code)]
        reason: Option<String>,
    },
}

impl RootEntry {
    fn spec(&self) -> String {
        match self {
            Self::Spec(spec) => spec.clone(),
            Self::Object { root, .. } => root.clone(),
        }
    }
}

fn rerun_if_changed(path: &Path) {
    println!("cargo:rerun-if-changed={}", path.display());
}

fn stable_generated_root(out_dir: &Path, name: &str) -> Result<PathBuf> {
    let build_dir = out_dir
        .ancestors()
        .find(|path| path.file_name().is_some_and(|name| name == "build"))
        .context("OUT_DIR is not under Cargo build directory")?;
    let profile_dir = build_dir
        .parent()
        .context("Cargo build directory has no profile parent")?;
    Ok(profile_dir.join("generated").join(name))
}

fn input_hash(
    build_script: &Path,
    generated_state_denylist_file: &Path,
    network_field_overrides_file: &Path,
    generated_type_selection_file: &Path,
    network_schema_file: &Path,
    message_signatures_file: &Path,
) -> Result<String> {
    let mut hash = blake3::Hasher::new();
    hash.update(CODEGEN_VERSION.as_bytes());
    hash.update(NETWORK_RUST_EMITTER_VERSION.as_bytes());
    hash_file("build.rs", build_script, &mut hash)?;
    hash_file(
        "codegen/generated-state-denylist.json",
        generated_state_denylist_file,
        &mut hash,
    )?;
    hash_file(
        "codegen/network-field-overrides.json",
        network_field_overrides_file,
        &mut hash,
    )?;
    hash_file(
        "crates/nw-network-types/codegen/selection.json",
        generated_type_selection_file,
        &mut hash,
    )?;
    hash_file(
        "crates/nw-network-types/codegen/network-schema.json",
        network_schema_file,
        &mut hash,
    )?;
    hash_file(
        "crates/nw-network-types/codegen/message-signatures.json",
        message_signatures_file,
        &mut hash,
    )?;
    hash_resource("serialize.json", nw_resources::SERIALIZE_JSON, &mut hash);
    for resource in nw_resources::module_descriptors() {
        hash_resource(resource.path, resource.bytes, &mut hash);
    }
    Ok(hash.finalize().to_hex().to_string())
}

fn load_network_schema(path: &Path) -> Result<NetworkSchema> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    let mut schema: NetworkSchema =
        serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))?;
    schema.normalize_derived_shapes();
    Ok(schema)
}

fn replicated_state_type_indices(schema: &NetworkSchema) -> Vec<u32> {
    let mut type_indices = schema
        .types
        .iter()
        .filter(|network_type| {
            network_type
                .capabilities
                .contains(&nw_serialize_codegen::NetworkTypeCapability::ReplicatedState)
        })
        .filter_map(|network_type| network_type.type_index)
        .collect::<Vec<_>>();
    type_indices.sort_unstable();
    type_indices.dedup();
    type_indices
}

fn selected_generated_type_unit(
    selection_file: &Path,
    network_schema: &mut NetworkSchema,
) -> Result<SerializeCodegenUnit> {
    let context = CodegenContext::automatic();
    let selection = GeneratedTypeSelectionFile::from_path(selection_file)?;
    let document = SerializeContextDocument::from_slice(nw_resources::SERIALIZE_JSON)
        .context("parse embedded nw-tools SerializeContext JSON")?;
    let module_descriptors = embedded_module_descriptors(&context)?;
    let compile_unit = SerializeContextCompiler::compile_with_inputs(
        document,
        SerializeContextCompileInputs {
            module_descriptors_root: Some(&module_descriptors),
            serialize_porting_root: None,
            class_registration_trace_root: None,
        },
        &context,
    );
    if compile_unit.has_errors() {
        bail!("SerializeContext codegen has errors");
    }

    network_schema.merge_serialize_type_catalog(&compile_unit.catalog);
    let roots = NetworkSerializeRootPlanner::new(network_schema)
        .with_compile_unit(&compile_unit)
        .plan(selection.root_specs())
        .root_specs;
    let root_type_ids = resolve_codegen_root_type_ids(
        &compile_unit.codegen_unit,
        roots.iter().map(String::as_str),
    )?;
    let selected = SerializeCodegenRootSelection::new(SerializeCodegenRootMode::Explicit)
        .with_explicit_roots(root_type_ids)
        .select_unit(&compile_unit.codegen_unit);
    let completed = complete_known_missing_reflected_bodies(selected, compile_unit.codegen_unit);
    Ok(completed.emitted)
}

fn required_source_struct_type_names(state_source: &str, message_source: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let compact_state = compact_rust_source(state_source);
    names.extend(source_type_names_after(
        &compact_state,
        "DefaultMarshaler<::nw_network::source::",
    ));
    names.extend(source_type_names_after(
        &compact_state,
        "ReplicatedFieldHandler<::nw_network::source::",
    ));
    names.extend(source_type_names(message_source));
    names
}

fn has_manual_source_marshaler(item: &SerializeCodegenItem) -> bool {
    MANUAL_SOURCE_MARSHALERS.contains(&item.source_name.as_str())
        || MANUAL_SOURCE_MARSHALERS.contains(&source_name_leaf(&item.source_name))
}

fn compact_rust_source(source: &str) -> String {
    source.chars().filter(|ch| !ch.is_whitespace()).collect()
}

fn source_type_names_after(source: &str, prefix: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut remaining = source;
    while let Some(offset) = remaining.find(prefix) {
        let after_prefix = &remaining[offset + prefix.len()..];
        if let Some((name, rest)) = split_leading_rust_ident(after_prefix) {
            names.insert(name.to_owned());
            remaining = rest;
        } else {
            remaining = after_prefix;
        }
    }
    names
}

fn source_type_names(source: &str) -> BTreeSet<String> {
    const PREFIX: &str = "::nw_network::source::";
    let mut names = BTreeSet::new();
    let mut remaining = source;
    while let Some(offset) = remaining.find(PREFIX) {
        let after_prefix = &remaining[offset + PREFIX.len()..];
        if let Some((name, rest)) = split_leading_rust_ident(after_prefix) {
            names.insert(name.to_owned());
            remaining = rest;
        } else {
            remaining = after_prefix;
        }
    }
    names
}

fn split_leading_rust_ident(value: &str) -> Option<(&str, &str)> {
    let ident_len = value
        .char_indices()
        .take_while(|(_, ch)| ch.is_ascii_alphanumeric() || *ch == '_')
        .map(|(index, ch)| index + ch.len_utf8())
        .last()
        .unwrap_or(0);
    (ident_len != 0).then_some((&value[..ident_len], &value[ident_len..]))
}

fn source_name_leaf(value: &str) -> &str {
    value.rsplit("::").next().unwrap_or(value).trim()
}

fn embedded_module_descriptors(context: &CodegenContext) -> Result<Value> {
    let mut resources = nw_resources::module_descriptors().collect::<Vec<_>>();
    resources.sort_by_key(|resource| resource.path);
    let modules = context
        .runner()
        .try_map(&resources, |resource| parse_module_descriptor(*resource))?;
    Ok(module_descriptors_root(modules))
}

fn parse_module_descriptor(resource: EmbeddedResource) -> Result<Value> {
    let root = serde_json::from_slice::<Value>(resource.bytes)
        .with_context(|| format!("parse embedded AZ::Module descriptor {}", resource.path))?;
    if root.get("descriptors").is_none() {
        bail!(
            "embedded AZ::Module descriptor {} does not contain `descriptors`",
            resource.path
        );
    }
    Ok(module_descriptor_capture(
        module_name_from_resource_name(resource.path),
        root,
    ))
}

fn hash_file(label: &str, path: &Path, hash: &mut blake3::Hasher) -> Result<()> {
    hash.update(label.as_bytes());
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    hash.update(&bytes);
    Ok(())
}

fn hash_resource(path: &str, bytes: &[u8], hash: &mut blake3::Hasher) {
    hash.update(path.as_bytes());
    hash.update(bytes);
}

fn write_file_if_changed(path: &Path, source: &[u8]) -> Result<bool> {
    let source_hash = blake3::hash(source);
    if existing_file_matches_hash(path, source.len() as u64, source_hash)? {
        return Ok(false);
    }
    fs::write(path, source)?;
    Ok(true)
}

fn existing_file_matches_hash(
    path: &Path,
    expected_len: u64,
    expected_hash: blake3::Hash,
) -> Result<bool> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.len() != expected_len => return Ok(false),
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(source) => return Err(source).with_context(|| format!("inspect {}", path.display())),
    }

    let mut file = fs::File::open(path).with_context(|| format!("open {}", path.display()))?;
    let mut hasher = blake3::Hasher::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    loop {
        let bytes_read = file
            .read(&mut buffer)
            .with_context(|| format!("read {}", path.display()))?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }
    Ok(hasher.finalize() == expected_hash)
}
