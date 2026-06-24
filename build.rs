use std::{
    env, fs,
    io::{self, Read},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use nw_serialize_codegen::{NETWORK_RUST_EMITTER_VERSION, NetworkRustEmitter, NetworkSchema};
use serde::Deserialize;

const CODEGEN_VERSION: &str = "nw-network-generated-states-v1";

fn main() -> Result<()> {
    let manifest_dir =
        PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").context("CARGO_MANIFEST_DIR")?);
    let build_script = manifest_dir.join("build.rs");
    let selection_file = manifest_dir.join("codegen/generated-states.json");
    let network_schema_file =
        manifest_dir.join("crates/nw-network-types/codegen/network-schema.json");

    rerun_if_changed(&build_script);
    rerun_if_changed(&selection_file);
    rerun_if_changed(&network_schema_file);

    let input_hash = input_hash(&build_script, &selection_file, &network_schema_file)?;
    let out_dir = PathBuf::from(env::var_os("OUT_DIR").context("OUT_DIR")?);
    let output_root = out_dir.join("nw_network");
    let stamp_path = output_root.join(".generated-states-input-hash");
    let source_path = output_root.join("generated_states.rs");
    let report_path = output_root.join("generated-states.rust-report.json");

    if source_path.is_file()
        && report_path.is_file()
        && fs::read_to_string(&stamp_path).is_ok_and(|stamp| stamp == input_hash)
    {
        return Ok(());
    }

    let selection = StateSelectionFile::from_path(&selection_file)?;
    let network_schema = load_network_schema(&network_schema_file)?;
    let output =
        NetworkRustEmitter::emit_replicated_states(&network_schema, selection.type_indices)
            .context("emit generated replicated states")?;

    fs::create_dir_all(&output_root)
        .with_context(|| format!("create {}", output_root.display()))?;
    write_file_if_changed(&source_path, output.source.as_bytes())
        .with_context(|| format!("write {}", source_path.display()))?;
    let mut report =
        serde_json::to_string_pretty(&output.report).context("serialize generated state report")?;
    report.push('\n');
    write_file_if_changed(&report_path, report.as_bytes())
        .with_context(|| format!("write {}", report_path.display()))?;
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
    hash.update(NETWORK_RUST_EMITTER_VERSION.as_bytes());
    hash_file("build.rs", build_script, &mut hash)?;
    hash_file("codegen/generated-states.json", selection_file, &mut hash)?;
    hash_file(
        "crates/nw-network-types/codegen/network-schema.json",
        network_schema_file,
        &mut hash,
    )?;
    Ok(hash.finalize().to_hex().to_string())
}

fn load_network_schema(path: &Path) -> Result<NetworkSchema> {
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("parse {}", path.display()))
}

fn hash_file(label: &str, path: &Path, hash: &mut blake3::Hasher) -> Result<()> {
    hash.update(label.as_bytes());
    let bytes = fs::read(path).with_context(|| format!("read {}", path.display()))?;
    hash.update(&bytes);
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
