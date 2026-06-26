use crate::serialize::ReplicatedFieldHandler;

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("02C9E1DC-F013-4B1F-BFAA-6314278984D5")]
#[::nw_network::type_registry(4878)]
pub struct MusicalPerformancePlayerComponentReplicatedState {
    pub performance_id: ReplicatedFieldHandler<u64>,
    #[replicated_state(group = 1)]
    pub lockout_duration: ReplicatedFieldHandler<u16>,
    #[replicated_state(group = 1)]
    pub performance_state: ReplicatedFieldHandler<u8>,
    #[replicated_state(group = 1)]
    pub observing_performance_id: ReplicatedFieldHandler<u64>,
}
