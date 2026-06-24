use crate::hub::ReplicatedState;
use crate::serialize::ReplicatedFieldHandler;

#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ReplicatedState,
    nw_network_derive::AzRtti,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("1774C5D5-C7E0-4E52-A9A1-3816DF9E25DA")]
#[type_registry(100)]
pub struct HubIFragmentReplicatedState {
    #[replicated_state(name = "replicatedHideLevel")]
    pub replicated_hide_level: ReplicatedFieldHandler<u32>,

    pub hub: ReplicatedState,
}
