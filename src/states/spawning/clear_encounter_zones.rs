//! Clear-encounter-zone state replication.

use crate::replicated_state;

use crate::serialize::ReplicatedFieldHandler;

/// Clear encounter-zone replicated state.
#[replicated_state]
#[derive(Debug, Clone, Default)]
pub struct ClearEncounterZonesReplicatedState {
    pub status: ReplicatedFieldHandler<u32>,
}
