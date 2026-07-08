use std::{
    collections::BTreeSet,
    env, fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use nw_resources::EmbeddedResource;
use nw_serialize_codegen::{
    CodegenContext, NETWORK_RUST_EMITTER_VERSION, NetworkConfidence, NetworkEvidence,
    NetworkEvidenceKind, NetworkField, NetworkFieldOverrideFile, NetworkMessageSignature,
    NetworkReplicatedStateEmitOptions, NetworkRustEmitter, NetworkSchema, NetworkType,
    SerializeCodegenItemKind, SerializeCodegenRootMode, SerializeCodegenRootSelection,
    SerializeCodegenUnit, SerializeContextCompileInputs, SerializeContextCompiler,
    SerializeContextDocument, complete_known_missing_reflected_bodies, module_descriptor_capture,
    module_descriptors_root, module_name_from_resource_name,
    network_schema::NetworkFieldHandlerVtable, network_schema::NetworkNestedTypeShape,
    network_schema::NetworkReplicatedContainerShape, resolve_codegen_root_type_ids,
    rust_type_ident,
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
    apply_message_signature_field_overrides(&mut network_schema, &message_signatures);
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
        selected_generated_type_unit(&generated_type_selection_file, &network_schema)
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
    let conversion_items = generated_types.items.iter().filter(|item| {
        let rust_name = rust_type_ident(source_name_leaf(&item.source_name));
        match item.kind {
            SerializeCodegenItemKind::Enum => true,
            SerializeCodegenItemKind::Struct => {
                required_source_struct_types.contains(&rust_name)
                    && !MANUAL_SOURCE_MARSHALERS.contains(&item.source_name.as_str())
                    && !MANUAL_SOURCE_MARSHALERS.contains(&source_name_leaf(&item.source_name))
            }
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

fn apply_message_signature_field_overrides(
    schema: &mut NetworkSchema,
    signatures: &[NetworkMessageSignature],
) {
    for signature in signatures {
        if signature.fields.is_empty() {
            continue;
        }

        let candidates = schema
            .types
            .iter()
            .enumerate()
            .filter(|(_, network_type)| message_signature_identity_matches(network_type, signature))
            .map(|(index, _)| index)
            .collect::<Vec<_>>();
        let [network_type_index] = candidates.as_slice() else {
            continue;
        };

        schema.types[*network_type_index].fields = network_fields_from_signature(signature);
    }
}

fn message_signature_identity_matches(
    network_type: &NetworkType,
    signature: &NetworkMessageSignature,
) -> bool {
    let has_identity = signature.type_id.is_some() || signature.type_index.is_some();
    if has_identity {
        if let Some(type_id) = signature.type_id
            && network_type.type_id != Some(type_id)
        {
            return false;
        }
        if let Some(type_index) = signature.type_index
            && network_type.type_index != Some(type_index)
        {
            return false;
        }
        return true;
    }

    if let Some(name) = signature.name.as_deref()
        && network_type.name.as_deref() != Some(name)
        && network_type.registration_type_name.as_deref() != Some(name)
    {
        return false;
    }
    true
}

fn network_fields_from_signature(signature: &NetworkMessageSignature) -> Vec<NetworkField> {
    let source = signature
        .source
        .clone()
        .unwrap_or_else(|| "message-signature-notes".to_owned());
    signature
        .fields
        .iter()
        .map(|field| NetworkField {
            index: field.index,
            name: Some(field.name.clone()),
            name_address: None,
            group: None,
            registration_kind: None,
            filter_group_attribute: None,
            handler_offset: None,
            handler_expression: None,
            handler_kind: None,
            handler_vtable: None,
            handler_vtable_slots: None,
            physical_field_count: None,
            native_type: field.native_type.clone(),
            source_type_name: None,
            source_type_id: None,
            rust_type: field.rust_type.clone(),
            storage_expression: None,
            storage_offset: None,
            raw_byte_length: None,
            wire_shape: field.wire_shape,
            wire_shape_source: field.wire_shape.map(|_| source.clone()),
            constructor_writes: Vec::new(),
            unmarshal_evidence: None,
            nested_type_shape: None,
            serialize: None,
            callsite: None,
            confidence: NetworkConfidence::High,
            evidence: vec![NetworkEvidence {
                kind: NetworkEvidenceKind::MessageSource,
                source: source.clone(),
                address: None,
                detail: Some(field.name.clone()),
                confidence: NetworkConfidence::High,
            }],
        })
        .collect()
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
    network_schema: &NetworkSchema,
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

    let roots = selected_source_root_specs(&selection, network_schema);
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

fn selected_source_root_specs(
    selection: &GeneratedTypeSelectionFile,
    schema: &NetworkSchema,
) -> Vec<String> {
    let mut roots = selection.root_specs().into_iter().collect::<BTreeSet<_>>();
    let source_roots = SchemaSourceRootIndex::from_schema(schema);
    add_schema_source_roots(&mut roots, schema, &source_roots);
    roots.into_iter().collect()
}

struct SchemaSourceRootIndex {
    type_ids: BTreeSet<String>,
    names: BTreeSet<String>,
}

impl SchemaSourceRootIndex {
    fn from_schema(schema: &NetworkSchema) -> Self {
        let mut type_ids = BTreeSet::new();
        let mut names = BTreeSet::new();
        let mut duplicate_names = BTreeSet::new();
        for serialize in &schema.serialize_types {
            if !is_semantic_source_name(&serialize.name) {
                type_ids.insert(serialize.type_id.to_string());
                if !names.insert(serialize.name.clone()) {
                    duplicate_names.insert(serialize.name.clone());
                }
            }
        }
        for name in duplicate_names {
            names.remove(&name);
        }
        Self { type_ids, names }
    }

    fn contains(&self, type_id: Option<&str>, name: Option<&str>) -> bool {
        type_id.is_some_and(|type_id| self.type_ids.contains(type_id))
            || name.is_some_and(|name| self.names.contains(name.trim()))
    }
}

fn add_schema_source_roots(
    roots: &mut BTreeSet<String>,
    schema: &NetworkSchema,
    source_roots: &SchemaSourceRootIndex,
) {
    for network_type in &schema.types {
        if let Some(serialize) = network_type.serialize.as_ref() {
            add_source_root(
                roots,
                Some(serialize.type_id.to_string()),
                Some(&serialize.name),
                source_roots,
            );
        }
        for field in &network_type.fields {
            if let Some(serialize) = field.serialize.as_ref() {
                add_source_root(
                    roots,
                    Some(serialize.type_id.to_string()),
                    Some(&serialize.name),
                    source_roots,
                );
            }
            add_source_root(
                roots,
                field.source_type_id.map(|type_id| type_id.to_string()),
                field.source_type_name.as_deref(),
                source_roots,
            );
            if let Some(shape) = field.nested_type_shape.as_ref() {
                add_nested_shape_source_roots(roots, shape, source_roots);
            }
        }
    }
    for vtable in &schema.field_handler_vtables {
        add_field_handler_source_roots(roots, vtable, source_roots);
    }
}

fn add_field_handler_source_roots(
    roots: &mut BTreeSet<String>,
    vtable: &NetworkFieldHandlerVtable,
    source_roots: &SchemaSourceRootIndex,
) {
    add_source_root(
        roots,
        vtable.value_type_id.clone(),
        vtable.value_type_name.as_deref(),
        source_roots,
    );
    for candidate in &vtable.value_type_candidates {
        add_source_root(
            roots,
            candidate.type_id.map(|type_id| type_id.to_string()),
            candidate.name.as_deref(),
            source_roots,
        );
    }
    if let Some(shape) = vtable.value_type_shape.as_ref() {
        add_nested_shape_source_roots(roots, shape, source_roots);
    }
    for shape in &vtable.embedded_value_type_shapes {
        add_nested_shape_source_roots(roots, shape, source_roots);
    }
    if let Some(container) = vtable.container_shape.as_ref() {
        add_container_shape_source_roots(roots, container, source_roots);
    }
}

fn add_container_shape_source_roots(
    roots: &mut BTreeSet<String>,
    container: &NetworkReplicatedContainerShape,
    source_roots: &SchemaSourceRootIndex,
) {
    add_source_root(
        roots,
        container.value_type_id.map(|type_id| type_id.to_string()),
        container.value_type_name.as_deref(),
        source_roots,
    );
    if let Some(shape) = container.key_type_shape.as_ref() {
        add_nested_shape_source_roots(roots, shape, source_roots);
    }
    if let Some(shape) = container.value_type_shape.as_ref() {
        add_nested_shape_source_roots(roots, shape, source_roots);
    }
    for shape in &container.embedded_value_type_shapes {
        add_nested_shape_source_roots(roots, shape, source_roots);
    }
}

fn add_nested_shape_source_roots(
    roots: &mut BTreeSet<String>,
    shape: &NetworkNestedTypeShape,
    source_roots: &SchemaSourceRootIndex,
) {
    add_source_root(
        roots,
        shape.type_id.map(|type_id| type_id.to_string()),
        shape
            .type_name_full
            .as_deref()
            .or(shape.type_name.as_deref()),
        source_roots,
    );
    for member in &shape.members {
        if let Some(native_type) = member.native_type.as_deref() {
            add_source_root(roots, None, Some(native_type), source_roots);
        }
    }
}

fn add_source_root(
    roots: &mut BTreeSet<String>,
    type_id: Option<String>,
    name: Option<&str>,
    source_roots: &SchemaSourceRootIndex,
) {
    if name.is_some_and(is_semantic_source_name) {
        return;
    }
    if !source_roots.contains(type_id.as_deref(), name) {
        return;
    }
    if let Some(type_id) = type_id {
        roots.insert(type_id);
        return;
    }
    let Some(name) = name.map(str::trim).filter(|name| !name.is_empty()) else {
        return;
    };
    if is_semantic_source_name(name) || !is_probable_source_type_name(name) {
        return;
    }
    roots.insert(name.to_owned());
}

fn is_probable_source_type_name(name: &str) -> bool {
    let leaf = name.rsplit("::").next().unwrap_or(name);
    leaf.chars()
        .next()
        .is_some_and(|first| first.is_ascii_uppercase())
        && !name.contains('<')
        && !name.contains(',')
        && !name.starts_with("AZ::")
        && !name.starts_with("AZStd::")
        && !name.starts_with("std::")
}

fn is_semantic_source_name(name: &str) -> bool {
    matches!(
        name.trim().rsplit("::").next().unwrap_or(name.trim()),
        "Vec2"
            | "Vector2"
            | "Vec3"
            | "Vector3"
            | "Vec4"
            | "Vector4"
            | "Quat"
            | "Quaternion"
            | "Matrix3x3"
            | "Transform"
            | "Uuid"
            | "UID"
    )
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
