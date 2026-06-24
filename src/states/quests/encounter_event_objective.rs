use arrayvec::ArrayVec;

use crate::Marshaler;
use crate::hub::ReplicatedState;
use crate::serialize::ReplicatedFieldHandler;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Marshaler)]
pub struct EncounterObjectiveStatusEntry {
    pub key: u32,
    pub value: u32,
}

pub type EncounterObjectiveStatus = ArrayVec<EncounterObjectiveStatusEntry, 10>;

/// Encounter-event objective status state.
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
pub struct EncounterEventObjectiveReplicatedState {
    pub status: ReplicatedFieldHandler<EncounterObjectiveStatus>,

    pub hub: ReplicatedState,
}
