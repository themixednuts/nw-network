use crate::hub::ReplicatedState;
use crate::serialize::ReplicatedFieldHandler;

#[derive(Debug, Clone, Default, nw_network_derive::ReplicatedState)]
pub struct TriggerAreaEntityEventTimingsReplicatedState {
    pub last_on_enter_events_execution_time: ReplicatedFieldHandler<u64>,
    pub last_on_exit_events_execution_time: ReplicatedFieldHandler<u64>,
    pub last_on_first_enter_events_execution_time: ReplicatedFieldHandler<u64>,
    pub last_on_last_exit_events_execution_time: ReplicatedFieldHandler<u64>,

    pub hub: ReplicatedState,
}
