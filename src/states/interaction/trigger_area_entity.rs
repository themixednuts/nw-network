use crate::serialize::ReplicatedFieldHandler;

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
pub struct TriggerAreaEntityEventTimingsReplicatedState {
    pub last_on_enter_events_execution_time: ReplicatedFieldHandler<u64>,
    pub last_on_exit_events_execution_time: ReplicatedFieldHandler<u64>,
    pub last_on_first_enter_events_execution_time: ReplicatedFieldHandler<u64>,
    pub last_on_last_exit_events_execution_time: ReplicatedFieldHandler<u64>,
}
