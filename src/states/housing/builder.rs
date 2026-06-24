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
#[az_rtti("149758CA-AEA4-4E44-AF78-D9495915B8AF")]
#[type_registry(2270)]
pub struct BuilderComponentReplicatedState {
    pub player_builder_state: ReplicatedFieldHandler<u8>,
    #[replicated_state(group = 1)]
    pub number_of_placed_deployables: ReplicatedFieldHandler<u8>,

    pub hub: ReplicatedState,
}
