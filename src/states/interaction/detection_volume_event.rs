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
#[az_rtti("083D6E80-4081-47B7-A477-BE9E7029AAFF")]
#[type_registry(366)]
pub struct DetectionVolumeEventReplicatedState {
    pub last_on_enter_events_execution_time: ReplicatedFieldHandler<u64>,
    pub last_on_exit_events_execution_time: ReplicatedFieldHandler<u64>,
    pub last_on_first_enter_events_execution_time: ReplicatedFieldHandler<u64>,
    pub last_on_last_exit_events_execution_time: ReplicatedFieldHandler<u64>,

    pub hub: ReplicatedState,
}
