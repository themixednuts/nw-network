use crate::serialize::ReplicatedFieldHandler;

/// Spectator camera availability state.
#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("56985FA2-5902-4C74-A879-C8C0F30AD569")]
#[::nw_network::type_registry(6817)]
pub struct PvPSpectatorCamControllerReplicatedState {
    #[replicated_state(group = 1)]
    pub enabled: ReplicatedFieldHandler<bool>,
    #[replicated_state(group = 1)]
    pub triggered_via_interactable: ReplicatedFieldHandler<bool>,
}
