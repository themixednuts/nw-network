use crate::hub::ReplicatedState;
use crate::serialize::ReplicatedFieldHandler;

/// Clear encounter-zone replicated state.
#[derive(Debug, Clone, Default, nw_network_derive::ReplicatedState)]
pub struct ClearEncounterZonesReplicatedState {
    pub status: ReplicatedFieldHandler<u32>,

    pub hub: ReplicatedState,
}
