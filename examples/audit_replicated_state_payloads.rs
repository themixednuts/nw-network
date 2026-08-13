use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::env;
use std::error::Error;
use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::{Path, PathBuf};

use nw_network::generated::states::{
    ALCReplicatedState as GeneratedAlc, AbilityComponentReplicatedState as GeneratedAbility,
    ContainerComponentReplicatedState as GeneratedContainer,
    GdeMetadataReplicatedState as GeneratedGdeMetadata,
    GlobalStorageComponentReplicatedState as GeneratedGlobalStorage,
    InstancedSlayerScriptReplicatedState as GeneratedInstancedSlayerScript,
    ItemManagementComponentReplicatedState as GeneratedItemManagement,
    ObjectiveInteractorComponentReplicatedState as GeneratedObjectiveInteractor,
    PaperdollComponentReplicatedState as GeneratedPaperdoll,
    PlayerComponentReplicatedState as GeneratedPlayer,
    ReactionTrackingReplicatedState as GeneratedReactionTracking,
    RewardTrackComponentReplicatedState as GeneratedRewardTrack,
    SeasonsRewardsReplicatedState as GeneratedSeasonsRewards,
    SlayerScriptReplicatedState as GeneratedSlayerScript,
    VitalsComponentReplicatedState as GeneratedVitals,
    WarDataComponentReplicatedState as GeneratedWarData,
};
use nw_network::hub::fragment_registration_by_type_index;
use nw_network::serialize::CARRIER_ENDIAN;
use nw_network::{DynFragment, ReadBuffer, TypeIndex, Unmarshal, WriteBuffer};
use serde_json::Value;
use uuid::Uuid;

const CAPTURE_SCHEMA: &str = "newworld.replicated_state.payload.v1";
const DENIED_TYPE_INDICES: &[u32] = &[
    10, 11, 15, 185, 1755, 1927, 3183, 3362, 3829, 3935, 4913, 5437, 6234,
];

type AnyError = Box<dyn Error + Send + Sync>;

#[derive(Clone)]
struct SchemaIdentity {
    type_index: u32,
    type_id: Uuid,
    name: String,
}

#[derive(Default)]
struct TypeStats {
    name: String,
    rows: usize,
    native_failures: usize,
    active_decode_passes: usize,
    active_decode_failures: usize,
    active_encode_matches: usize,
    generated_decode_passes: usize,
    generated_decode_failures: usize,
    generated_encode_valid: usize,
    generated_encode_invalid: usize,
    generated_encode_matches: usize,
    active_failure_reasons: BTreeMap<String, usize>,
    generated_failure_reasons: BTreeMap<String, usize>,
    generated_encode_failure_reasons: BTreeMap<String, usize>,
}

struct Replay {
    encoded: Vec<u8>,
}

fn main() -> Result<(), AnyError> {
    let mut args = env::args_os().skip(1);
    let payload_path = args.next().map(PathBuf::from).ok_or_else(|| {
        invalid_data(
            "usage: cargo run --example audit_replicated_state_payloads -- \
             <replicated_state_payloads.jsonl> <typeregistry.json> [network-schema.json]",
        )
    })?;
    let registry_path = args.next().map(PathBuf::from).ok_or_else(|| {
        invalid_data("missing typeregistry.json path; the registry identity check is mandatory")
    })?;
    let schema_path = args.next().map_or_else(
        || PathBuf::from("crates/nw-network-types/codegen/network-schema.json"),
        PathBuf::from,
    );
    if args.next().is_some() {
        return Err(invalid_data("too many arguments"));
    }

    let schema_by_vtable = load_schema_identities(&schema_path)?;
    let (registry_by_index, registry_by_uuid) = load_type_registry(&registry_path)?;
    let file = File::open(&payload_path)
        .map_err(|error| path_error("open payload capture", &payload_path, error))?;

    let mut stats = BTreeMap::<u32, TypeStats>::new();
    let mut hard_failures = Vec::new();
    let mut captured_denied = BTreeSet::new();
    let mut total_rows = 0usize;

    for (line_index, line) in BufReader::new(file).lines().enumerate() {
        let line_number = line_index + 1;
        let line =
            line.map_err(|error| path_error("read payload capture", &payload_path, error))?;
        if line.trim().is_empty() {
            continue;
        }
        total_rows += 1;
        let row: Value = serde_json::from_str(&line).map_err(|error| {
            invalid_data(format!(
                "{}:{line_number}: invalid JSON: {error}",
                payload_path.display()
            ))
        })?;
        if row.get("schema").and_then(Value::as_str) != Some(CAPTURE_SCHEMA) {
            hard_failures.push(format!("line {line_number}: unexpected capture schema"));
            continue;
        }

        let Some(vtable) = row.get("instance_vtable").and_then(Value::as_str) else {
            hard_failures.push(format!("line {line_number}: missing instance_vtable"));
            continue;
        };
        let Some(identity) = schema_by_vtable.get(&normalize_address(vtable)) else {
            hard_failures.push(format!(
                "line {line_number}: instance vtable {vtable} is absent from the canonical schema"
            ));
            continue;
        };
        if let Err(error) =
            verify_registry_identity(identity, &registry_by_index, &registry_by_uuid)
        {
            hard_failures.push(format!("line {line_number}: {error}"));
            continue;
        }

        let entry = stats.entry(identity.type_index).or_default();
        entry.name.clone_from(&identity.name);
        entry.rows += 1;
        if DENIED_TYPE_INDICES.contains(&identity.type_index) {
            captured_denied.insert(identity.type_index);
        }

        if row.get("success").and_then(Value::as_bool) != Some(true) {
            entry.native_failures += 1;
            hard_failures.push(format!(
                "line {line_number}: native {} unmarshal did not succeed",
                identity.name
            ));
            continue;
        }
        let Some(payload_hex) = row.get("payload_hex").and_then(Value::as_str) else {
            hard_failures.push(format!("line {line_number}: missing payload_hex"));
            continue;
        };
        let payload = match decode_hex(payload_hex) {
            Ok(payload) => payload,
            Err(error) => {
                hard_failures.push(format!("line {line_number}: {error}"));
                continue;
            }
        };
        let expected_len = row
            .get("consumed_bytes")
            .and_then(Value::as_u64)
            .and_then(|value| usize::try_from(value).ok());
        if expected_len != Some(payload.len()) {
            hard_failures.push(format!(
                "line {line_number}: consumed_bytes {:?} disagrees with {} decoded payload bytes",
                expected_len,
                payload.len()
            ));
            continue;
        }
        match replay_active(identity.type_index, &payload) {
            Ok(replay) => {
                entry.active_decode_passes += 1;
                if replay.encoded == payload {
                    entry.active_encode_matches += 1;
                }
            }
            Err(error) => {
                let error = match diagnose_active_failure(identity.type_index, &payload) {
                    Some(diagnostic) => format!("{error}; {diagnostic}"),
                    None => error,
                };
                entry.active_decode_failures += 1;
                *entry
                    .active_failure_reasons
                    .entry(error.clone())
                    .or_default() += 1;
                hard_failures.push(format!(
                    "line {line_number}: active {} decoder failed: {error}",
                    identity.name
                ));
            }
        }

        if let Some(result) = replay_denied_generated(identity.type_index, &payload) {
            match result {
                Ok(replay) => {
                    entry.generated_decode_passes += 1;
                    match consume_active(identity.type_index, &replay.encoded) {
                        Ok(()) => entry.generated_encode_valid += 1,
                        Err(error) => {
                            entry.generated_encode_invalid += 1;
                            let reason = format!(
                                "active decoder rejected output: {error}; {}",
                                byte_difference(&payload, &replay.encoded)
                            );
                            *entry
                                .generated_encode_failure_reasons
                                .entry(reason)
                                .or_default() += 1;
                        }
                    }
                    if replay.encoded == payload {
                        entry.generated_encode_matches += 1;
                    }
                }
                Err(error) => {
                    entry.generated_decode_failures += 1;
                    *entry
                        .generated_failure_reasons
                        .entry(error.clone())
                        .or_default() += 1;
                }
            }
        }
    }

    println!(
        "audited {total_rows} native payload row(s) across {} replicated-state type(s)",
        stats.len()
    );
    for (type_index, entry) in &stats {
        let generated = if DENIED_TYPE_INDICES.contains(type_index) {
            format!(
                ", generated decode pass/fail {}/{}, encode accepted/rejected {}/{}, exact {}/{}",
                entry.generated_decode_passes,
                entry.generated_decode_failures,
                entry.generated_encode_valid,
                entry.generated_encode_invalid,
                entry.generated_encode_matches,
                entry.generated_decode_passes,
            )
        } else {
            String::new()
        };
        println!(
            "typeIndex {type_index:>4} {:<64} rows {}, native failures {}, active decode pass/fail {}/{}, exact encode {}/{}{generated}",
            entry.name,
            entry.rows,
            entry.native_failures,
            entry.active_decode_passes,
            entry.active_decode_failures,
            entry.active_encode_matches,
            entry.active_decode_passes,
        );
        for (reason, count) in &entry.active_failure_reasons {
            println!("  active failure x{count}: {reason}");
        }
        for (reason, count) in &entry.generated_failure_reasons {
            println!("  generated failure x{count}: {reason}");
        }
        for (reason, count) in &entry.generated_encode_failure_reasons {
            println!("  generated output rejection x{count}: {reason}");
        }
    }

    let missing_denied = DENIED_TYPE_INDICES
        .iter()
        .copied()
        .filter(|type_index| !captured_denied.contains(type_index))
        .collect::<Vec<_>>();
    println!(
        "generated-denylist coverage: {}/{} type(s); not observed: {:?}",
        captured_denied.len(),
        DENIED_TYPE_INDICES.len(),
        missing_denied
    );
    if !missing_denied.is_empty() {
        hard_failures.push(format!(
            "capture does not cover denied generated type indices {missing_denied:?}"
        ));
    }

    if hard_failures.is_empty() {
        return Ok(());
    }
    for failure in hard_failures.iter().take(50) {
        eprintln!("FAIL: {failure}");
    }
    if hard_failures.len() > 50 {
        eprintln!(
            "FAIL: {} additional failure(s) omitted",
            hard_failures.len() - 50
        );
    }
    Err(invalid_data(format!(
        "replicated-state payload audit found {} hard failure(s)",
        hard_failures.len()
    )))
}

fn load_schema_identities(path: &Path) -> Result<HashMap<String, SchemaIdentity>, AnyError> {
    let root: Value = serde_json::from_reader(BufReader::new(
        File::open(path).map_err(|error| path_error("open network schema", path, error))?,
    ))
    .map_err(|error| invalid_data(format!("parse {}: {error}", path.display())))?;
    let types = root
        .get("types")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_data(format!("{} has no types array", path.display())))?;
    let mut identities = HashMap::new();
    for value in types {
        let capabilities = value.get("capabilities").and_then(Value::as_array);
        if !capabilities.is_some_and(|values| {
            values
                .iter()
                .any(|value| value.as_str() == Some("replicated-state"))
        }) {
            continue;
        }
        let Some(type_index) = value
            .get("typeIndex")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
        else {
            continue;
        };
        let Some(type_id) = value
            .get("typeId")
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
        else {
            continue;
        };
        let Some(name) = value.get("name").and_then(Value::as_str) else {
            continue;
        };
        let Some(address) = value
            .get("azRtti")
            .and_then(|value| value.get("address"))
            .and_then(Value::as_str)
        else {
            continue;
        };
        let key = normalize_address(address);
        let identity = SchemaIdentity {
            type_index,
            type_id,
            name: name.to_owned(),
        };
        if let Some(previous) = identities.insert(key.clone(), identity) {
            return Err(invalid_data(format!(
                "{} maps vtable {address} to both typeIndex {} and {type_index}",
                path.display(),
                previous.type_index
            )));
        }
    }
    Ok(identities)
}

fn load_type_registry(path: &Path) -> Result<(HashMap<u32, Uuid>, HashMap<Uuid, u32>), AnyError> {
    let root: Value = serde_json::from_reader(BufReader::new(
        File::open(path).map_err(|error| path_error("open type registry", path, error))?,
    ))
    .map_err(|error| invalid_data(format!("parse {}: {error}", path.display())))?;
    let data = root
        .get("data")
        .ok_or_else(|| invalid_data(format!("{} has no data object", path.display())))?;
    let index_entries = data
        .get("typeIndex")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_data(format!("{} has no data.typeIndex array", path.display())))?;
    let mut by_index = HashMap::new();
    for value in index_entries {
        let Some(type_index) = value
            .get("typeIndex")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
        else {
            continue;
        };
        let Some(type_id) = value
            .get("uuid")
            .and_then(Value::as_str)
            .and_then(|value| Uuid::parse_str(value).ok())
        else {
            continue;
        };
        by_index.insert(type_index, type_id);
    }

    let uuid_entries = data
        .get("typesByUuid")
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_data(format!("{} has no data.typesByUuid array", path.display())))?;
    let mut by_uuid = HashMap::new();
    for value in uuid_entries {
        let Some(pair) = value.as_array().filter(|pair| pair.len() == 2) else {
            continue;
        };
        let Some(type_id) = pair[0]
            .as_str()
            .and_then(|value| Uuid::parse_str(value).ok())
        else {
            continue;
        };
        let Some(type_index) = pair[1]
            .get("typeIndex")
            .and_then(Value::as_u64)
            .and_then(|value| u32::try_from(value).ok())
        else {
            continue;
        };
        by_uuid.insert(type_id, type_index);
    }
    Ok((by_index, by_uuid))
}

fn verify_registry_identity(
    identity: &SchemaIdentity,
    by_index: &HashMap<u32, Uuid>,
    by_uuid: &HashMap<Uuid, u32>,
) -> Result<(), String> {
    match by_index.get(&identity.type_index) {
        Some(type_id) if *type_id == identity.type_id => {}
        Some(type_id) => {
            return Err(format!(
                "typeregistry typeIndex {} has UUID {type_id}, schema has {}",
                identity.type_index, identity.type_id
            ));
        }
        None => {
            return Err(format!(
                "typeregistry has no typeIndex {}",
                identity.type_index
            ));
        }
    }
    match by_uuid.get(&identity.type_id) {
        Some(type_index) if *type_index == identity.type_index => Ok(()),
        Some(type_index) => Err(format!(
            "typeregistry UUID {} maps to typeIndex {type_index}, schema has {}",
            identity.type_id, identity.type_index
        )),
        None => Err(format!(
            "typeregistry has no UUID {} for typeIndex {}",
            identity.type_id, identity.type_index
        )),
    }
}

fn replay_active(type_index: u32, payload: &[u8]) -> Result<Replay, String> {
    let registration = fragment_registration_by_type_index(TypeIndex::new(type_index))
        .ok_or_else(|| format!("no active fragment registration for typeIndex {type_index}"))?;
    let mut rb = ReadBuffer::new(CARRIER_ENDIAN, payload);
    let fragment = (registration.decode_contents)(&mut rb).map_err(|error| error.to_string())?;
    if rb.left() != 0 {
        return Err(format!(
            "left {} of {} bytes unread",
            rb.left(),
            payload.len()
        ));
    }
    let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
    fragment.marshal_contents(&mut wb);
    Ok(Replay {
        encoded: wb.as_slice().to_vec(),
    })
}

macro_rules! trace_generated_state_fields {
    ($payload:expr, $state_ty:ty, [$($field:ident),+ $(,)?]) => {{
        (|| -> Result<String, String> {
            let mut state = <$state_ty>::default();
            let mut rb = ReadBuffer::new(CARRIER_ENDIAN, $payload);
            let descriptor_mask = rb
                .read_u8()
                .map_err(|error| format!("descriptor at byte 0: {error}"))?;
            if descriptor_mask & 1 == 0 {
                return Ok(format!(
                    "field trace: group 0 absent (descriptor 0x{descriptor_mask:02x}), {} bytes remain",
                    rb.left()
                ));
            }

            let field_count = [$(stringify!($field)),+].len();
            let mut field_mask = 0u8;
            let mut field_index = 0usize;
            let mut descriptor_done = false;
            let mut decoded = Vec::new();
            $(
                if !descriptor_done {
                    if field_index % 7 == 0 {
                        let offset = rb.position();
                        field_mask = rb.read_u8().map_err(|error| {
                            format!("field-mask chunk at byte {offset}: {error}")
                        })?;
                    }
                    if (field_mask & (1 << (field_index % 7))) != 0 {
                        let start = rb.position();
                        state.$field = Unmarshal::unmarshal(&mut rb).map_err(|error| {
                            format!(
                                "field trace: {} (index {field_index}) at bytes {start}..{}: {error}",
                                stringify!($field),
                                rb.position()
                            )
                        })?;
                        decoded.push(format!(
                            "{}@{start}..{}",
                            stringify!($field),
                            rb.position()
                        ));
                    }
                    if (field_index % 7 == 6 || field_index + 1 == field_count)
                        && (field_mask & 0x80) == 0
                    {
                        descriptor_done = true;
                    }
                }
                field_index += 1;
            )+
            let _ = (&state, field_index);
            Ok(format!(
                "field trace decoded [{}], stopped at byte {}, {} bytes remain",
                decoded.join(", "),
                rb.position(),
                rb.left()
            ))
        })()
    }};
}

fn diagnose_active_failure(type_index: u32, payload: &[u8]) -> Option<String> {
    match type_index {
        1739 => Some(
            trace_generated_state_fields!(
                payload,
                GeneratedWarData,
                [war_data, war_schedule_adjustments, influence_race_data]
            )
            .unwrap_or_else(|error| error),
        ),
        2938 => Some(
            trace_generated_state_fields!(
                payload,
                GeneratedGlobalStorage,
                [
                    global_item_map,
                    overflow_item_count,
                    weight_map,
                    slot_count_map
                ]
            )
            .unwrap_or_else(|error| error),
        ),
        5485 => Some(
            trace_generated_state_fields!(
                payload,
                GeneratedSeasonsRewards,
                [
                    is_initialized,
                    season_ids,
                    season_bitmask_count,
                    season_xp_by_season,
                    redeem_bitmask,
                    escrow_bitmask,
                    foreign_escrow_bitmask,
                    first_character_connect_time
                ]
            )
            .unwrap_or_else(|error| error),
        ),
        _ => None,
    }
}

fn consume_active(type_index: u32, payload: &[u8]) -> Result<(), String> {
    let registration = fragment_registration_by_type_index(TypeIndex::new(type_index))
        .ok_or_else(|| format!("no active fragment registration for typeIndex {type_index}"))?;
    let mut rb = ReadBuffer::new(CARRIER_ENDIAN, payload);
    (registration.consume_contents)(&mut rb).map_err(|error| error.to_string())?;
    if rb.left() != 0 {
        return Err(format!(
            "left {} of {} bytes unread",
            rb.left(),
            payload.len()
        ));
    }
    Ok(())
}

fn replay_denied_generated(type_index: u32, payload: &[u8]) -> Option<Result<Replay, String>> {
    Some(match type_index {
        10 => replay_generated::<GeneratedGdeMetadata>(payload),
        11 => replay_generated::<GeneratedAlc>(payload),
        15 => replay_generated::<GeneratedVitals>(payload),
        185 => replay_generated::<GeneratedAbility>(payload),
        1755 => replay_generated::<GeneratedContainer>(payload),
        1927 => replay_generated::<GeneratedReactionTracking>(payload),
        3183 => replay_generated::<GeneratedPaperdoll>(payload),
        3362 => replay_generated::<GeneratedSlayerScript>(payload),
        3829 => replay_generated::<GeneratedObjectiveInteractor>(payload),
        3935 => replay_generated::<GeneratedPlayer>(payload),
        4913 => replay_generated::<GeneratedRewardTrack>(payload),
        5437 => replay_generated::<GeneratedItemManagement>(payload),
        6234 => replay_generated::<GeneratedInstancedSlayerScript>(payload),
        _ => return None,
    })
}

fn replay_generated<T>(payload: &[u8]) -> Result<Replay, String>
where
    T: DynFragment + Default,
{
    let mut value = T::default();
    let mut rb = ReadBuffer::new(CARRIER_ENDIAN, payload);
    value
        .unmarshal_contents(&mut rb)
        .map_err(|error| error.to_string())?;
    if rb.left() != 0 {
        return Err(format!(
            "left {} of {} bytes unread",
            rb.left(),
            payload.len()
        ));
    }
    let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
    value.marshal_contents(&mut wb);
    Ok(Replay {
        encoded: wb.as_slice().to_vec(),
    })
}

fn decode_hex(value: &str) -> Result<Vec<u8>, String> {
    if !value.len().is_multiple_of(2) {
        return Err("payload_hex has an odd number of digits".to_owned());
    }
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let high = hex_nibble(pair[0])?;
            let low = hex_nibble(pair[1])?;
            Ok((high << 4) | low)
        })
        .collect()
}

fn hex_nibble(value: u8) -> Result<u8, String> {
    match value {
        b'0'..=b'9' => Ok(value - b'0'),
        b'a'..=b'f' => Ok(value - b'a' + 10),
        b'A'..=b'F' => Ok(value - b'A' + 10),
        _ => Err(format!("payload_hex contains non-hex byte 0x{value:02x}")),
    }
}

fn normalize_address(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}

fn byte_difference(expected: &[u8], actual: &[u8]) -> String {
    let shared = expected.len().min(actual.len());
    if let Some(offset) = (0..shared).find(|offset| expected[*offset] != actual[*offset]) {
        return format!(
            "first difference at byte {offset}: native 0x{:02x}, Rust 0x{:02x}; lengths {} and {}",
            expected[offset],
            actual[offset],
            expected.len(),
            actual.len()
        );
    }
    format!(
        "shared prefix matches; lengths {} and {}",
        expected.len(),
        actual.len()
    )
}

fn invalid_data(message: impl Into<String>) -> AnyError {
    io::Error::new(io::ErrorKind::InvalidData, message.into()).into()
}

fn path_error(action: &str, path: &Path, error: io::Error) -> AnyError {
    io::Error::new(
        error.kind(),
        format!("{action} {}: {error}", path.display()),
    )
    .into()
}
