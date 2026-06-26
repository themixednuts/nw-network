use crate::serialize::{ReplicatedFieldHandler, ReplicatedVec};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TransmogSnapshot {
    pub captured_armor_sequence: u64,
    pub captured_weapon_sequence: u64,
    pub owned_armor_sequence: u64,
    pub owned_weapon_sequence: u64,
    pub captured_armor: Vec<u64>,
    pub captured_weapon: Vec<u64>,
    pub owned_armor: Vec<u64>,
    pub owned_weapon: Vec<u64>,
    pub inventory_services_ready: bool,
}

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("DEE6E179-3D80-4160-9BAD-CC5DA6B60C46")]
#[::nw_network::type_registry(5691)]
pub struct TransmogComponentReplicatedState {
    pub captured_armor_appearances: ReplicatedVec<u64>,
    pub captured_weapon_appearances: ReplicatedVec<u64>,
    pub owned_armor_appearances: ReplicatedVec<u64>,
    pub owned_weapon_appearances: ReplicatedVec<u64>,
    pub inventory_services_ready: ReplicatedFieldHandler<bool>,
}

impl TransmogComponentReplicatedState {
    pub fn apply_snapshot(&mut self, snapshot: TransmogSnapshot) {
        self.captured_armor_appearances =
            ReplicatedVec::new(snapshot.captured_armor_sequence, snapshot.captured_armor);
        self.captured_weapon_appearances =
            ReplicatedVec::new(snapshot.captured_weapon_sequence, snapshot.captured_weapon);
        self.owned_armor_appearances =
            ReplicatedVec::new(snapshot.owned_armor_sequence, snapshot.owned_armor);
        self.owned_weapon_appearances =
            ReplicatedVec::new(snapshot.owned_weapon_sequence, snapshot.owned_weapon);
        self.inventory_services_ready
            .set_value(snapshot.inventory_services_ready);
    }
}
