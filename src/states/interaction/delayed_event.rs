use crate::serialize::{HalfF32Marshaler, ReplicatedFieldHandler};

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("B42BA5BB-D0AA-4A0B-8974-6D0A754DCBD5")]
#[::nw_network::type_registry(3139)]
pub struct DelayedEventComponentReplicatedState {
    pub total_delay_duration: ReplicatedFieldHandler<f32, HalfF32Marshaler>,
    pub delayed_event_completed: ReplicatedFieldHandler<bool>,
}
