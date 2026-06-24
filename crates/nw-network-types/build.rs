use std::{
    collections::BTreeSet,
    env, fs,
    io::{self, Read},
    path::{Component, Path, PathBuf},
};

use anyhow::{Context, Result, bail};
use nw_resources::EmbeddedResource;
use nw_serialize_codegen::{
    CodegenContext, NetworkRustEmitter, NetworkSchema, RustCodegenPlanner, RustSourceEmitter,
    RustStandaloneProjectFile, SerializeCodegenRootMode, SerializeCodegenRootSelection,
    SerializeContextCompileInputs, SerializeContextCompiler, SerializeContextDocument,
    complete_known_missing_reflected_bodies, module_descriptor_capture, module_descriptors_root,
    module_name_from_resource_name, resolve_codegen_root_type_ids,
};
use serde::Deserialize;
use serde_json::Value;

const CODEGEN_VERSION: &str = "nw-network-v1.3";

fn main() -> Result<()> {
    let manifest_dir =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").context("CARGO_MANIFEST_DIR")?);
    let build_script = manifest_dir.join("build.rs");
    let selection_file = manifest_dir.join("codegen/selection.json");
    let network_schema_file = manifest_dir.join("codegen/network-schema.json");
    let context = CodegenContext::automatic();

    rerun_if_changed(&build_script);
    rerun_if_changed(&selection_file);
    rerun_if_changed(&network_schema_file);

    let input_hash = input_hash(&build_script, &selection_file, &network_schema_file)?;
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").context("OUT_DIR")?);
    let output_root = out_dir.join("nw_network");
    let stamp_path = output_root.join(".input-hash");
    if output_root.join("src/lib.rs").is_file()
        && fs::read_to_string(&stamp_path).is_ok_and(|stamp| stamp == input_hash)
    {
        return Ok(());
    }

    let selection = SelectionFile::from_path(&selection_file)?;
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
    let rust_unit = RustCodegenPlanner::standalone().plan_serialize_codegen_units(
        &completed.emitted,
        &completed.context,
        &context,
    );
    let project = RustSourceEmitter::emit_standalone_project(&rust_unit, &context)
        .with_context(|| "emit nw-network standalone Rust crate")?;
    let network_schema = load_network_schema(&network_schema_file)?;
    let network_output = NetworkRustEmitter::emit_descriptors(&network_schema)
        .context("emit network schema descriptor Rust")?;
    let mut files = project.files;
    files.push(RustStandaloneProjectFile {
        path: "src/network_schema.rs".to_owned(),
        source: network_output.source,
    });
    let mut report = serde_json::to_string_pretty(&network_output.report)
        .context("serialize network schema Rust generation report")?;
    report.push('\n');
    files.push(RustStandaloneProjectFile {
        path: "network-schema.rust-report.json".to_owned(),
        source: report,
    });

    write_project(&output_root, &stamp_path, &input_hash, files, &context)
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SelectionFile {
    roots: Vec<RootEntry>,
}

impl SelectionFile {
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
    selection_file: &Path,
    network_schema_file: &Path,
) -> Result<String> {
    let mut hash = blake3::Hasher::new();
    hash.update(CODEGEN_VERSION.as_bytes());
    hash_file("build.rs", build_script, &mut hash)?;
    hash_resource("serialize.json", nw_resources::SERIALIZE_JSON, &mut hash);
    hash_file("codegen/selection.json", selection_file, &mut hash)?;
    hash_file(
        "codegen/network-schema.json",
        network_schema_file,
        &mut hash,
    )?;
    for resource in nw_resources::module_descriptors() {
        hash_resource(resource.path, resource.bytes, &mut hash);
    }
    Ok(hash.finalize().to_hex().to_string())
}

fn load_network_schema(path: &Path) -> Result<NetworkSchema> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))
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

fn hash_resource(path: &str, bytes: &[u8], hash: &mut blake3::Hasher) {
    hash.update(path.as_bytes());
    hash.update(bytes);
}

fn hash_file(label: &str, path: &Path, hash: &mut blake3::Hasher) -> Result<()> {
    hash.update(label.as_bytes());
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    hash.update(&bytes);
    Ok(())
}

fn write_project(
    output_root: &Path,
    stamp_path: &Path,
    input_hash: &str,
    files: Vec<nw_serialize_codegen::RustStandaloneProjectFile>,
    context: &CodegenContext,
) -> Result<()> {
    let files = files
        .into_iter()
        .map(|file| {
            let source = generated_source(&file.path, file.source);
            Ok(ProjectFile {
                path: output_root.join(path_from_slash_relative(&file.path)?),
                source,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    fs::create_dir_all(output_root)
        .with_context(|| format!("create output tree {}", output_root.display()))?;

    let expected_paths = files
        .iter()
        .map(|file| file.path.clone())
        .chain(std::iter::once(stamp_path.to_path_buf()))
        .collect::<BTreeSet<_>>();
    let stale_files = sorted_files(output_root)?
        .into_iter()
        .filter(|path| !expected_paths.contains(path))
        .collect::<Vec<_>>();
    context.runner().try_map(&stale_files, |path| {
        fs::remove_file(path)
            .with_context(|| format!("remove stale generated file {}", path.display()))
    })?;

    let parent_dirs = files
        .iter()
        .filter_map(|file| file.path.parent())
        .map(Path::to_path_buf)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    context.runner().try_map(&parent_dirs, |dir| {
        fs::create_dir_all(dir)
            .with_context(|| format!("create generated directory {}", dir.display()))
    })?;
    context.runner().try_map(&files, |file| {
        write_file_if_changed(&file.path, file.source.as_bytes())
            .with_context(|| format!("write generated file {}", file.path.display()))
    })?;
    write_file_if_changed(stamp_path, input_hash.as_bytes())
        .with_context(|| format!("write {}", stamp_path.display()))?;
    prune_empty_dirs(output_root)
}

#[derive(Debug)]
struct ProjectFile {
    path: PathBuf,
    source: String,
}

fn generated_source(path: &str, source: String) -> String {
    if path == "src/lib.rs" {
        let without_inner_attr = if source.starts_with("#![") {
            source.split_once('\n').map_or(source.clone(), |(_, rest)| {
                rest.trim_start_matches('\n').to_owned()
            })
        } else {
            source
        };
        let mut source = without_inner_attr.replace("pub mod az;", "#[doc(hidden)]\npub mod az;");
        source.push_str("\npub mod network_schema;\n");
        return source;
    }
    source
}

fn sorted_files(path: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_files(path, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect_files(path: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(path).with_context(|| format!("read directory {}", path.display()))? {
        let entry = entry.with_context(|| format!("read directory entry in {}", path.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("read file type {}", path.display()))?;
        if file_type.is_dir() {
            collect_files(&path, files)?;
        } else if file_type.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

fn prune_empty_dirs(root: &Path) -> Result<()> {
    let mut dirs = Vec::new();
    collect_dirs(root, &mut dirs)?;
    dirs.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    for dir in dirs {
        if dir == root {
            continue;
        }
        match fs::remove_dir(&dir) {
            Ok(()) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::DirectoryNotEmpty | io::ErrorKind::NotFound
                ) => {}
            Err(source) => {
                return Err(source).with_context(|| {
                    format!("remove empty generated directory {}", dir.display())
                });
            }
        }
    }
    Ok(())
}

fn collect_dirs(path: &Path, dirs: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(path).with_context(|| format!("read directory {}", path.display()))? {
        let entry = entry.with_context(|| format!("read directory entry in {}", path.display()))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("read file type {}", path.display()))?;
        if file_type.is_dir() {
            dirs.push(path.clone());
            collect_dirs(&path, dirs)?;
        }
    }
    Ok(())
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

fn path_from_slash_relative(relative: &str) -> Result<PathBuf> {
    let mut path = PathBuf::new();
    for part in relative.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            bail!("invalid generated relative path `{relative}`");
        }
        let component = Path::new(part)
            .components()
            .next()
            .context("generated path segment")?;
        if !matches!(component, Component::Normal(_)) {
            bail!("invalid generated relative path `{relative}`");
        }
        path.push(part);
    }
    Ok(path)
}
