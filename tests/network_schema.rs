use nw_network::{
    NetworkFieldConfidence, NetworkTypeKind, fields_for_type_index, name_for_type_index,
    type_by_type_index, unknown_type_indices,
};

#[test]
fn generated_schema_resolves_known_state_and_message_types() {
    let raid_state = type_by_type_index(28).expect("raid state descriptor");
    assert_eq!(raid_state.name, "Javelin::RaidDataComponentReplicatedState");
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
}

#[test]
fn generated_schema_reports_unknown_type_indices_for_capture_validation() {
    assert_eq!(unknown_type_indices([28, 164, u32::MAX]), vec![u32::MAX]);
}
