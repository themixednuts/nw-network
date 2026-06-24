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
#[az_rtti("E24E0E75-02AB-4973-8F20-74DFB0436444")]
#[type_registry(610)]
pub struct TwitchStreamReplicatedState {
    pub is_live: ReplicatedFieldHandler<bool>,
    pub num_viewers: ReplicatedFieldHandler<u32>,
    pub channel_name: ReplicatedFieldHandler<String>,

    pub hub: ReplicatedState,
}
