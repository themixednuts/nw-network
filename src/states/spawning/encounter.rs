//! Encounter status-list replication.

use crate::{Marshaler, az_rtti, replicated_state, type_registry};

use arrayvec::ArrayVec;

use crate::serialize::ReplicatedFieldHandler;

pub const MAX_ENCOUNTER_STATUS_ENTRIES: usize = 10;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Marshaler)]
pub struct EncounterStatusEntry {
    pub key: u32,
    pub value: u32,
}

#[replicated_state]
#[derive(Debug, Clone, Default)]
#[az_rtti("F2C3B42E-DB86-4B2C-840F-64748FE26C73")]
#[type_registry(2133)]
pub struct EncounterComponentReplicatedState {
    pub status:
        ReplicatedFieldHandler<ArrayVec<EncounterStatusEntry, MAX_ENCOUNTER_STATUS_ENTRIES>>,
}
