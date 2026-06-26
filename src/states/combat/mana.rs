use crate::serialize::ReplicatedFieldHandler;

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("3729094D-0FBA-4DCF-B191-7E9E53AA9B85")]
#[::nw_network::type_registry(1652)]
pub struct ManaComponentReplicatedState {
    pub cur: ReplicatedFieldHandler<f32>,
    pub max: ReplicatedFieldHandler<f32>,
    pub regen_delay: ReplicatedFieldHandler<f32>,
    pub regen_rate: ReplicatedFieldHandler<f32>,
}
