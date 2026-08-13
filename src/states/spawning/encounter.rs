//! Encounter status-list replication.

use crate::Marshaler;

pub const MAX_ENCOUNTER_STATUS_ENTRIES: usize = 10;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Marshaler)]
pub struct EncounterStatusEntry {
    pub key: u32,
    pub value: u32,
}
pub use crate::generated::states::EncounterComponentReplicatedState;
