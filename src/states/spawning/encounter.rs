use arrayvec::ArrayVec;

use crate::serialize::ReplicatedFieldHandler;

pub const MAX_ENCOUNTER_STATUS_ENTRIES: usize = 10;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ::nw_network::Marshaler)]
pub struct EncounterStatusEntry {
    pub key: u32,
    pub value: u32,
}

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("F2C3B42E-DB86-4B2C-840F-64748FE26C73")]
#[::nw_network::type_registry(2133)]
pub struct EncounterComponentReplicatedState {
    pub status:
        ReplicatedFieldHandler<ArrayVec<EncounterStatusEntry, MAX_ENCOUNTER_STATUS_ENTRIES>>,
}
