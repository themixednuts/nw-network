use crate::serialize::ReplicatedFieldHandler;

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("E24E0E75-02AB-4973-8F20-74DFB0436444")]
#[::nw_network::type_registry(610)]
pub struct TwitchStreamReplicatedState {
    pub is_live: ReplicatedFieldHandler<bool>,
    pub num_viewers: ReplicatedFieldHandler<u32>,
    pub channel_name: ReplicatedFieldHandler<String>,
}
