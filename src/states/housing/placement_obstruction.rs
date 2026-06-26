use crate::serialize::ReplicatedFieldHandler;

/// Replicated placement-completion obstruction state.
#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("EAE37835-E282-46CC-99A0-81C6BC53CEEA")]
#[::nw_network::type_registry(2187)]
pub struct PlacementObstructionComponentReplicatedState {
    pub has_completion_obstruction: ReplicatedFieldHandler<bool>,
}
