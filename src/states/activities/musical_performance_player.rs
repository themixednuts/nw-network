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
#[az_rtti("02C9E1DC-F013-4B1F-BFAA-6314278984D5")]
#[type_registry(4878)]
pub struct MusicalPerformancePlayerComponentReplicatedState {
    pub performance_id: ReplicatedFieldHandler<u64>,
    #[replicated_state(group = 1)]
    pub lockout_duration: ReplicatedFieldHandler<u16>,
    #[replicated_state(group = 1)]
    pub performance_state: ReplicatedFieldHandler<u8>,
    #[replicated_state(group = 1)]
    pub observing_performance_id: ReplicatedFieldHandler<u64>,

    pub hub: ReplicatedState,
}
