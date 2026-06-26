use arrayvec::ArrayVec;

use crate::Marshaler;
use crate::serialize::ReplicatedFieldHandler;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Marshaler)]
pub struct EncounterObjectiveStatusEntry {
    pub key: u32,
    pub value: u32,
}

pub type EncounterObjectiveStatus = ArrayVec<EncounterObjectiveStatusEntry, 10>;

/// Encounter-event objective status state.
#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("F2C3B42E-DB86-4B2C-840F-64748FE26C73")]
pub struct EncounterEventObjectiveReplicatedState {
    pub status: ReplicatedFieldHandler<EncounterObjectiveStatus>,
}
