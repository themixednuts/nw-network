use crate::serialize::ReplicatedFieldHandler;

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("083D6E80-4081-47B7-A477-BE9E7029AAFF")]
#[::nw_network::type_registry(366)]
pub struct DetectionVolumeEventReplicatedState {
    pub last_on_enter_events_execution_time: ReplicatedFieldHandler<u64>,
    pub last_on_exit_events_execution_time: ReplicatedFieldHandler<u64>,
    pub last_on_first_enter_events_execution_time: ReplicatedFieldHandler<u64>,
    pub last_on_last_exit_events_execution_time: ReplicatedFieldHandler<u64>,
}
