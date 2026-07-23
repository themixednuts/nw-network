use std::collections::BTreeMap;

use nw_network::generated::states::{
    CapturePointReplicatedState as GeneratedCapturePointReplicatedState,
    EventTimelineComponentReplicatedState as GeneratedEventTimelineComponentReplicatedState,
    IncapacitatedReplicatedState as GeneratedIncapacitatedReplicatedState,
    LookTargetingComponentReplicatedState as GeneratedLookTargetingComponentReplicatedState,
    PlayerHousingReplicatedState as GeneratedPlayerHousingReplicatedState,
    TurretReplicatedState as GeneratedTurretReplicatedState,
};
use nw_network::network_schema::identity::RaidDataComponentReplicatedState;
use nw_network::{
    NetworkFieldConfidence, NetworkTypeCapability, NetworkTypeIdentity, NetworkWireShape,
    field_for_type_index, fields_for_type_index,
    generated::messages::{RegisterFragmentAccessMsg, UnregisterFragmentAccessMsg},
    hub::FragmentKey,
    is_replicated_state_type_index, name_for_type_index, non_replicated_state_type_indices,
    replicated_state_port_statuses, type_by_type_index, type_indices_missing_field_wire_shapes,
    unknown_type_indices, validate_state_fragment_type_indices,
};
use serde_json::Value;
use uuid::Uuid;

const NETWORK_SCHEMA_JSON: &str =
    include_str!("../crates/nw-network-types/codegen/network-schema.json");
const CAPTURE_POINT_TYPE_ID: Uuid = Uuid::from_u128(0x1c9f052f_b0db_4466_9ed1_681d34da4452);
const INCAPACITATED_TYPE_ID: Uuid = Uuid::from_u128(0x490db5f1_4e39_483a_9897_78fa312e45b5);
const EVENT_TIMELINE_TYPE_ID: Uuid = Uuid::from_u128(0x9e6ee43b_15f1_497b_9461_1f97e488aa10);
const LOOK_TARGETING_TYPE_ID: Uuid = Uuid::from_u128(0xeae9bc99_4d02_4ba5_acfa_85eb452119f2);
const PLAYER_HOUSING_TYPE_ID: Uuid = Uuid::from_u128(0x4f70c3bb_8f7d_48c2_a0b6_95431f88f356);
const TURRET_TYPE_ID: Uuid = Uuid::from_u128(0xa9f8d205_2922_41e1_b8c2_0dfaf1cc8475);

#[test]
fn generated_schema_resolves_known_state_and_message_types() {
    let raid_state = type_by_type_index(28).expect("raid state descriptor");
    assert_eq!(
        raid_state.name,
        Some("Javelin::RaidDataComponentReplicatedState")
    );
    assert!(raid_state.is_replicated_state());
    assert!(raid_state.has_registered_fields());

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
    assert!(type_indices_missing_field_wire_shapes([28]).is_empty());

    assert_eq!(
        name_for_type_index(164),
        Some("ClientActorRoutingAuthorizationTrait::ClientAddEntryMsg")
    );

    let force_migrate = type_by_type_index(67).expect("force migrate descriptor");
    assert_eq!(
        force_migrate.name,
        Some("MB::ServerContext::ForceMigrateActorMsg")
    );
    assert!(force_migrate.is_direct_message());
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
    assert!(
        RaidDataComponentReplicatedState::CAPABILITIES
            .contains(&NetworkTypeCapability::ReplicatedState)
    );
    assert!(
        RaidDataComponentReplicatedState::CAPABILITIES
            .contains(&NetworkTypeCapability::RegisteredFields)
    );
    assert_eq!(
        RaidDataComponentReplicatedState::descriptor().name,
        Some("Javelin::RaidDataComponentReplicatedState")
    );
}

#[test]
fn generated_fragment_access_messages_compile_with_resolved_fields() {
    assert_eq!(
        <RegisterFragmentAccessMsg as nw_network::TypeRegistryEntry>::TYPE_INDEX,
        397
    );
    assert_eq!(
        <UnregisterFragmentAccessMsg as nw_network::TypeRegistryEntry>::TYPE_INDEX,
        399
    );
    let register = RegisterFragmentAccessMsg {
        proxy_ref: Default::default(),
        key: FragmentKey::new(7),
    };
    let unregister = UnregisterFragmentAccessMsg {
        proxy_ref: register.proxy_ref,
        key: register.key,
    };

    assert_eq!(unregister.key, FragmentKey::new(7));
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
    let coverage = validate_state_fragment_type_indices([
        11,
        11,
        28,
        333,
        670,
        1647,
        2443,
        2768,
        3451,
        2947,
        4276,
        67,
        164,
        u32::MAX,
    ]);

    assert_eq!(coverage.unknown_type_indices, vec![u32::MAX]);
    assert_eq!(coverage.non_replicated_state_type_indices, vec![67, 164]);
    assert_eq!(
        coverage.unregistered_replicated_state_type_indices,
        vec![2947, 3451]
    );
    assert_eq!(
        coverage.registered_replicated_state_type_indices,
        vec![11, 28, 333, 670, 1647, 2443, 2768, 4276]
    );
    assert_eq!(
        coverage.field_shape_incomplete_replicated_state_type_indices,
        vec![11, 333, 1647, 2768, 2947, 3451, 4276]
    );
    assert_eq!(
        coverage.generation_ready_unregistered_replicated_state_type_indices,
        Vec::<u32>::new()
    );
    assert!(!coverage.is_fully_registered());
    assert!(!coverage.is_fully_supported());

    let registered_state = validate_state_fragment_type_indices([670]);
    assert!(registered_state.is_fully_registered());
    assert!(registered_state.has_complete_field_shapes());
    assert!(registered_state.is_fully_supported());

    let newly_generated_state = validate_state_fragment_type_indices([28]);
    assert!(newly_generated_state.is_fully_registered());
    assert!(newly_generated_state.has_complete_field_shapes());
    assert!(newly_generated_state.is_fully_supported());
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
    assert_eq!(alc_status_state.field_count, 64);
    assert_eq!(alc_status_state.missing_field_wire_shape_count, 1);
    assert!(!alc_status_state.has_complete_field_shapes());
    assert!(!alc_status_state.can_generate_state_fields());
}

#[test]
fn generated_replicated_states_are_registered_unless_denied() {
    let expected = [
        (
            <GeneratedCapturePointReplicatedState as nw_network::TypeRegistryEntry>::TYPE_INDEX,
            <GeneratedCapturePointReplicatedState as nw_network::AzRtti>::TYPE_ID,
            CAPTURE_POINT_TYPE_ID,
        ),
        (
            <GeneratedIncapacitatedReplicatedState as nw_network::TypeRegistryEntry>::TYPE_INDEX,
            <GeneratedIncapacitatedReplicatedState as nw_network::AzRtti>::TYPE_ID,
            INCAPACITATED_TYPE_ID,
        ),
        (
            <GeneratedEventTimelineComponentReplicatedState as nw_network::TypeRegistryEntry>::TYPE_INDEX,
            <GeneratedEventTimelineComponentReplicatedState as nw_network::AzRtti>::TYPE_ID,
            EVENT_TIMELINE_TYPE_ID,
        ),
        (
            <GeneratedLookTargetingComponentReplicatedState as nw_network::TypeRegistryEntry>::TYPE_INDEX,
            <GeneratedLookTargetingComponentReplicatedState as nw_network::AzRtti>::TYPE_ID,
            LOOK_TARGETING_TYPE_ID,
        ),
        (
            <GeneratedPlayerHousingReplicatedState as nw_network::TypeRegistryEntry>::TYPE_INDEX,
            <GeneratedPlayerHousingReplicatedState as nw_network::AzRtti>::TYPE_ID,
            PLAYER_HOUSING_TYPE_ID,
        ),
        (
            <GeneratedTurretReplicatedState as nw_network::TypeRegistryEntry>::TYPE_INDEX,
            <GeneratedTurretReplicatedState as nw_network::AzRtti>::TYPE_ID,
            TURRET_TYPE_ID,
        ),
    ];

    for (type_index, type_id, expected_type_id) in expected {
        assert_eq!(type_id, expected_type_id);
        let registration = nw_network::hub::fragment_registration_by_type_index(type_index)
            .expect("generated state registration");
        assert_eq!((registration.type_index)(), type_index);
        assert_eq!((registration.uuid)(), expected_type_id);
    }
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
