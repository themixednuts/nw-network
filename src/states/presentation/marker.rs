use crate::serialize::ReplicatedFieldHandler;

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("A0CA40E0-FAC9-4964-81AD-98E23E2F9CD7")]
#[::nw_network::type_registry(5399)]
pub struct MarkerComponentReplicatedState {
    pub enabled: ReplicatedFieldHandler<bool>,
}
