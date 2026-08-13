//! Transmog item and station interaction replication.

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
pub use crate::generated::states::TransmogComponentReplicatedState;
