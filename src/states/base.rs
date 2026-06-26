use crate::serialize::ReplicatedFieldHandler;

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("1774C5D5-C7E0-4E52-A9A1-3816DF9E25DA")]
#[::nw_network::type_registry(100)]
pub struct HubIFragmentReplicatedState {
    #[replicated_state(name = "replicatedHideLevel")]
    pub replicated_hide_level: ReplicatedFieldHandler<u32>,
}
