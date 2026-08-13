//! Encounter-event objective status replication.

use arrayvec::ArrayVec;

use crate::Marshaler;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Marshaler)]
pub struct EncounterObjectiveStatusEntry {
    pub key: u32,
    pub value: u32,
}

pub type EncounterObjectiveStatus = ArrayVec<EncounterObjectiveStatusEntry, 10>;
