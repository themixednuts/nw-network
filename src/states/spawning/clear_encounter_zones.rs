use crate::serialize::ReplicatedFieldHandler;

/// Clear encounter-zone replicated state.
#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
pub struct ClearEncounterZonesReplicatedState {
    pub status: ReplicatedFieldHandler<u32>,
}
