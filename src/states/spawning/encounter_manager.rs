//! Encounter manager activation replication.

use crate::{az_rtti, replicated_state, type_registry};

use crate::serialize::ReplicatedFieldHandler;

#[replicated_state]
#[derive(Debug, Clone, Default)]
#[az_rtti("CB2BE398-151A-42BB-ACDF-1C5CF871BE84")]
#[type_registry(6786)]
pub struct EncounterManagerComponentReplicatedState {
    pub status: ReplicatedFieldHandler<i32>,
}
