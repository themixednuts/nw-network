use crate::hub::ReplicatedState;
use crate::serialize::ReplicatedFieldHandler;

/// Replicated placement-completion obstruction state.
#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ReplicatedState,
    nw_network_derive::AzRtti,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("EAE37835-E282-46CC-99A0-81C6BC53CEEA")]
#[type_registry(2187)]
pub struct PlacementObstructionComponentReplicatedState {
    pub has_completion_obstruction: ReplicatedFieldHandler<bool>,

    pub hub: ReplicatedState,
}
