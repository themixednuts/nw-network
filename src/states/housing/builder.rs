use crate::serialize::ReplicatedFieldHandler;

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("149758CA-AEA4-4E44-AF78-D9495915B8AF")]
#[::nw_network::type_registry(2270)]
pub struct BuilderComponentReplicatedState {
    pub player_builder_state: ReplicatedFieldHandler<u8>,
    #[replicated_state(group = 1)]
    pub number_of_placed_deployables: ReplicatedFieldHandler<u8>,
}
