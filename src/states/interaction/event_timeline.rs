use crate::serialize::ReplicatedFieldHandler;

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("9E6EE43B-15F1-497B-9461-1F97E488AA10")]
#[::nw_network::type_registry(363)]
pub struct EventTimelineComponentReplicatedState {
    pub timeline_index: ReplicatedFieldHandler<u16>,
    pub timeline_status: ReplicatedFieldHandler<u8>,
}
