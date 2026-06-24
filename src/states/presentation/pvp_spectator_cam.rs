use crate::hub::ReplicatedState;
use crate::serialize::ReplicatedFieldHandler;

/// Spectator camera availability state.
#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ReplicatedState,
    nw_network_derive::AzRtti,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("56985FA2-5902-4C74-A879-C8C0F30AD569")]
#[type_registry(6817)]
pub struct PvPSpectatorCamControllerReplicatedState {
    #[replicated_state(group = 1)]
    pub enabled: ReplicatedFieldHandler<bool>,
    #[replicated_state(group = 1)]
    pub triggered_via_interactable: ReplicatedFieldHandler<bool>,

    pub hub: ReplicatedState,
}
