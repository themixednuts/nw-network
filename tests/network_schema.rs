use nw_network::network_schema::identity::RaidDataComponentReplicatedState;
use nw_network::{
    NetworkFieldConfidence, NetworkTypeIdentity, NetworkTypeKind, fields_for_type_index,
    is_replicated_state_type_index, name_for_type_index, non_replicated_state_type_indices,
    type_by_type_index, unknown_type_indices,
};
use serde_json::Value;

const NETWORK_SCHEMA_JSON: &str =
    include_str!("../crates/nw-network-types/codegen/network-schema.json");

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
            && field.confidence == NetworkFieldConfidence::High
    }));

    assert_eq!(
        name_for_type_index(164),
        Some("ClientActorRoutingAuthorizationTrait::ClientAddEntryMsg")
    );

    let unnamed_type = type_by_type_index(67).expect("unnamed descriptor");
    assert_eq!(unnamed_type.name, None);
    assert_eq!(name_for_type_index(67), None);
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
