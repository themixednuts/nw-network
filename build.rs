use std::{
    env, fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use nw_resources::EmbeddedResource;
use nw_serialize_codegen::{
    CodegenContext, NETWORK_RUST_EMITTER_VERSION, NetworkFieldOverrideFile,
    NetworkReplicatedStateEmitOptions, NetworkRustEmitter, NetworkSchema, SerializeCodegenRootMode,
    SerializeCodegenRootSelection, SerializeCodegenUnit, SerializeContextCompileInputs,
    SerializeContextCompiler, SerializeContextDocument, complete_known_missing_reflected_bodies,
    module_descriptor_capture, module_descriptors_root, module_name_from_resource_name,
    resolve_codegen_root_type_ids,
};
use serde::Deserialize;
use serde_json::Value;

const CODEGEN_VERSION: &str = "nw-network-generated-payloads-v3";

fn main() -> Result<()> {
    let manifest_dir =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").context("CARGO_MANIFEST_DIR")?);
    let build_script = manifest_dir.join("build.rs");
    let registered_state_selection_file = manifest_dir.join("codegen/generated-states.json");
    let network_field_overrides_file = manifest_dir.join("codegen/network-field-overrides.json");
    let generated_type_selection_file =
        manifest_dir.join("crates/nw-network-types/codegen/selection.json");
    let network_schema_file =
        manifest_dir.join("crates/nw-network-types/codegen/network-schema.json");

    rerun_if_changed(&build_script);
    rerun_if_changed(&registered_state_selection_file);
    rerun_if_changed(&network_field_overrides_file);
    rerun_if_changed(&generated_type_selection_file);
    rerun_if_changed(&network_schema_file);

    let input_hash = input_hash(
        &build_script,
        &registered_state_selection_file,
        &network_field_overrides_file,
        &generated_type_selection_file,
        &network_schema_file,
    )?;
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").context("OUT_DIR")?);
    let output_root = out_dir.join("nw_network");
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
    let registered_state_selection =
        StateSelectionFile::from_path(&registered_state_selection_file)?;
    let replicated_state_type_indices = replicated_state_type_indices(&network_schema);
    let output = NetworkRustEmitter::emit_replicated_states_with_options(
        &network_schema,
        replicated_state_type_indices,
        NetworkReplicatedStateEmitOptions::register_only(registered_state_selection.type_indices),
    )
    .context("emit generated replicated states")?;
    let message_output =
        NetworkRustEmitter::emit_messages(&network_schema).context("emit generated messages")?;
    let generated_types = selected_generated_type_unit(&generated_type_selection_file)
        .context("compile selected generated network data types")?;
    let conversion_output =
        NetworkRustEmitter::emit_marshaler_conversions(generated_types.items.iter())
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
struct StateSelectionFile {
    type_indices: Vec<u32>,
}

impl StateSelectionFile {
    fn from_path(path: &Path) -> Result<Self> {
        let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
        serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))
    }
}

fn load_network_field_overrides(path: &Path) -> Result<NetworkFieldOverrideFile> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))
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

fn input_hash(
    build_script: &Path,
    registered_state_selection_file: &Path,
    network_field_overrides_file: &Path,
    generated_type_selection_file: &Path,
    network_schema_file: &Path,
) -> Result<String> {
    let mut hash = blake3::Hasher::new();
    hash.update(CODEGEN_VERSION.as_bytes());
    hash.update(NETWORK_RUST_EMITTER_VERSION.as_bytes());
    hash_file("build.rs", build_script, &mut hash)?;
    hash_file(
        "codegen/generated-states.json",
        registered_state_selection_file,
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
    hash_resource("serialize.json", nw_resources::SERIALIZE_JSON, &mut hash);
    for resource in nw_resources::module_descriptors() {
        hash_resource(resource.path, resource.bytes, &mut hash);
    }
    Ok(hash.finalize().to_hex().to_string())
}

fn load_network_schema(path: &Path) -> Result<NetworkSchema> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))
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

fn selected_generated_type_unit(selection_file: &Path) -> Result<SerializeCodegenUnit> {
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

    let roots = selection.root_specs();
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
