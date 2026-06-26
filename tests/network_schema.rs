use std::collections::BTreeMap;

use nw_network::generated::states::RaidDataComponentReplicatedState as GeneratedRaidDataComponentReplicatedState;
use nw_network::network_schema::identity::RaidDataComponentReplicatedState;
use nw_network::{
    NetworkFieldConfidence, NetworkTypeIdentity, NetworkTypeKind, NetworkWireShape,
    field_for_type_index, fields_for_type_index,
    generated::messages::{
        RegisterFragmentAccessMsg, ReplicateClientFragmentUpdateMsg, UnregisterFragmentAccessMsg,
    },
    hub::{BaselineableFragment, FragmentKey},
    is_replicated_state_type_index, name_for_type_index, non_replicated_state_type_indices,
    replicated_state_port_statuses, type_by_type_index, type_indices_missing_field_wire_shapes,
    unknown_type_indices, validate_state_fragment_type_indices,
};
use serde_json::Value;
use uuid::Uuid;

const NETWORK_SCHEMA_JSON: &str =
    include_str!("../crates/nw-network-types/codegen/network-schema.json");
const RAID_DATA_TYPE_ID: Uuid = Uuid::from_u128(0xa85df621_dce0_409f_8d39_a447ea0807ff);

#[test]
fn generated_schema_resolves_known_state_and_message_types() {
    let raid_state = type_by_type_index(28).expect("raid state descriptor");
    assert_eq!(
        raid_state.name,
        Some("Javelin::RaidDataComponentReplicatedState")
    );
    assert_eq!(raid_state.kind, NetworkTypeKind::ReplicatedState);
    assert!(raid_state.is_field_registered);

    let fields = fields_for_type_index(28).expect("raid state fields");
    assert!(fields.iter().any(|field| {
        field.index == 0
            && field.name == "raidId"
            && field.group == Some(0)
            && field.wire_shape == Some(NetworkWireShape::U64)
            && field.confidence == NetworkFieldConfidence::High
    }));
    let raid_id = field_for_type_index(28, 0).expect("raidId field descriptor");
    assert_eq!(raid_id.name, "raidId");
    assert!(raid_id.has_wire_shape());
    assert_eq!(raid_state.missing_field_wire_shape_count(), 0);
    assert!(raid_state.has_complete_field_wire_shapes());
    assert_eq!(
        type_indices_missing_field_wire_shapes([28]),
        Vec::<u32>::new()
    );

    assert_eq!(
        name_for_type_index(164),
        Some("ClientActorRoutingAuthorizationTrait::ClientAddEntryMsg")
    );

    let force_migrate = type_by_type_index(67).expect("force migrate descriptor");
    assert_eq!(
        force_migrate.name,
        Some("MB::ServerContext::ForceMigrateActorMsg")
    );
    assert_eq!(force_migrate.kind, NetworkTypeKind::Message);
    assert_eq!(
        name_for_type_index(67),
        Some("MB::ServerContext::ForceMigrateActorMsg")
    );
}

#[test]
fn generated_identity_marker_resolves_descriptor_metadata() {
    assert_eq!(RaidDataComponentReplicatedState::TYPE_INDEX, 28);
    assert_eq!(
        RaidDataComponentReplicatedState::NAME,
        "Javelin::RaidDataComponentReplicatedState"
    );
    assert_eq!(
        RaidDataComponentReplicatedState::KIND,
        NetworkTypeKind::ReplicatedState
    );
    assert_eq!(
        RaidDataComponentReplicatedState::descriptor().name,
        Some("Javelin::RaidDataComponentReplicatedState")
    );
}

#[test]
fn generated_fragment_messages_compile_with_resolved_fields() {
    assert_eq!(
        <RegisterFragmentAccessMsg as nw_network::TypeRegistryEntry>::TYPE_INDEX,
        397
    );
    assert_eq!(
        <UnregisterFragmentAccessMsg as nw_network::TypeRegistryEntry>::TYPE_INDEX,
        399
    );
    assert_eq!(
        <ReplicateClientFragmentUpdateMsg as nw_network::TypeRegistryEntry>::TYPE_INDEX,
        422
    );

    let register = RegisterFragmentAccessMsg {
        proxy_ref: Default::default(),
        key: FragmentKey::new(7),
    };
    let unregister = UnregisterFragmentAccessMsg {
        proxy_ref: register.proxy_ref,
        key: register.key,
    };
    let update = ReplicateClientFragmentUpdateMsg {
        target_ref: unregister.proxy_ref,
        key: unregister.key,
        fragment: BaselineableFragment::default(),
    };

    assert_eq!(update.key, FragmentKey::new(7));
    assert!(update.fragment.body.is_empty());
}

#[test]
fn generated_schema_reports_unknown_type_indices_for_capture_validation() {
    assert_eq!(
        unknown_type_indices([28, 67, 164, u32::MAX]),
        vec![u32::MAX]
    );
    assert!(is_replicated_state_type_index(28));
    assert!(!is_replicated_state_type_index(67));
    assert_eq!(
        non_replicated_state_type_indices([28, 67, 164, u32::MAX]),
        vec![67, 164]
    );
}

#[test]
fn state_fragment_type_coverage_distinguishes_schema_and_decoder_gaps() {
    let coverage = validate_state_fragment_type_indices([11, 11, 28, 67, 164, u32::MAX]);

    assert_eq!(coverage.unknown_type_indices, vec![u32::MAX]);
    assert_eq!(coverage.non_replicated_state_type_indices, vec![67, 164]);
    assert_eq!(
        coverage.unregistered_replicated_state_type_indices,
        Vec::<u32>::new()
    );
    assert_eq!(
        coverage.registered_replicated_state_type_indices,
        vec![11, 28]
    );
    assert_eq!(
        coverage.field_shape_incomplete_replicated_state_type_indices,
        vec![11]
    );
    assert_eq!(
        coverage.generation_ready_unregistered_replicated_state_type_indices,
        Vec::<u32>::new()
    );
    assert!(!coverage.is_fully_registered());
    assert!(!coverage.is_fully_supported());

    let registered_state = validate_state_fragment_type_indices([11]);
    assert!(registered_state.is_fully_registered());
    assert!(!registered_state.has_complete_field_shapes());
    assert!(!registered_state.is_fully_supported());

    let generated_state = validate_state_fragment_type_indices([28]);
    assert!(generated_state.is_fully_registered());
    assert!(generated_state.has_complete_field_shapes());
    assert!(generated_state.is_fully_supported());
}

#[test]
fn replicated_state_port_statuses_compare_schema_and_registered_ports() {
    let statuses = replicated_state_port_statuses();

    let raid_state = statuses
        .iter()
        .find(|status| status.type_index == 28)
        .expect("raid state status");
    assert_eq!(
        raid_state.name,
        Some("Javelin::RaidDataComponentReplicatedState")
    );
    assert!(raid_state.is_registered);
    assert_eq!(raid_state.field_count, 5);
    assert_eq!(raid_state.missing_field_wire_shape_count, 0);
    assert!(raid_state.has_complete_field_shapes());
    assert!(!raid_state.can_generate_state_fields());

    let alc_status_state = statuses
        .iter()
        .find(|status| status.type_index == 11)
        .expect("alc status state status");
    assert!(alc_status_state.is_registered);
    assert_eq!(alc_status_state.field_count, 0);
    assert!(!alc_status_state.has_complete_field_shapes());
    assert!(!alc_status_state.can_generate_state_fields());
}

#[test]
fn generated_replicated_state_is_registered_from_allowlist() {
    assert_eq!(
        <GeneratedRaidDataComponentReplicatedState as nw_network::TypeRegistryEntry>::TYPE_INDEX,
        28
    );
    assert_eq!(
        <GeneratedRaidDataComponentReplicatedState as nw_network::AzRtti>::TYPE_ID,
        RAID_DATA_TYPE_ID
    );

    let registration =
        nw_network::hub::fragment_registration_by_type_index(28).expect("generated raid state");
    assert_eq!((registration.type_index)(), 28);
    assert_eq!((registration.uuid)(), RAID_DATA_TYPE_ID);
}

#[test]
fn fragment_type_index_registrations_are_unique() {
    let mut counts = BTreeMap::<u32, usize>::new();
    for registration in inventory::iter::<nw_network::FragmentRegistration> {
        *counts.entry((registration.type_index)()).or_default() += 1;
    }

    let duplicates = counts
        .into_iter()
        .filter_map(|(type_index, count)| (count > 1).then_some((type_index, count)))
        .collect::<Vec<_>>();

    assert!(
        duplicates.is_empty(),
        "duplicate fragment registrations: {duplicates:?}"
    );
}

#[test]
fn checked_in_schema_carries_confidence_ranked_serialize_evidence() {
    let schema: Value = serde_json::from_str(NETWORK_SCHEMA_JSON).expect("network schema JSON");
    assert_eq!(schema["summary"]["serializeTypeCount"], 12);
    assert_eq!(schema["summary"]["serializeDependencyCount"], 6);

    let null_type = type_by_schema_name(&schema, "NullType").expect("NullType schema entry");
    assert!(null_type["serialize"].is_null());

    let query_shape =
        type_by_schema_name(&schema, "QueryShapePoint").expect("QueryShapePoint schema entry");
    assert_eq!(query_shape["serialize"]["name"], "QueryShapePoint");
    let serialize_evidence = query_shape["evidence"]
        .as_array()
        .expect("evidence array")
        .iter()
        .find(|evidence| evidence["kind"] == "serialize-context")
        .expect("serialize evidence");
    assert_eq!(serialize_evidence["source"], "serializeContext:name");
    assert_eq!(serialize_evidence["confidence"], "inferred");
}

fn type_by_schema_name<'a>(schema: &'a Value, name: &str) -> Option<&'a Value> {
    schema["types"]
        .as_array()?
        .iter()
        .find(|network_type| network_type["name"] == name)
}
