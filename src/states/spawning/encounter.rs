use arrayvec::ArrayVec;

use crate::hub::ReplicatedState;
use crate::serialize::ReplicatedFieldHandler;

pub const MAX_ENCOUNTER_STATUS_ENTRIES: usize = 10;

/// Generated network value shape.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, nw_network_derive::Marshaler)]
pub struct EncounterStatusEntry {
    pub key: u32,
    pub value: u32,
}

#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ReplicatedState,
    nw_network_derive::AzRtti,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("F2C3B42E-DB86-4B2C-840F-64748FE26C73")]
#[type_registry(2133)]
pub struct EncounterComponentReplicatedState {
    pub status:
        ReplicatedFieldHandler<ArrayVec<EncounterStatusEntry, MAX_ENCOUNTER_STATUS_ENTRIES>>,

    pub hub: ReplicatedState,
}
