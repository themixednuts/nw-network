use crate::hub::ReplicatedState;
use crate::serialize::{HalfF32Marshaler, ReplicatedFieldHandler};

#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ReplicatedState,
    nw_network_derive::AzRtti,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("B42BA5BB-D0AA-4A0B-8974-6D0A754DCBD5")]
#[type_registry(3139)]
pub struct DelayedEventComponentReplicatedState {
    pub total_delay_duration: ReplicatedFieldHandler<f32, HalfF32Marshaler>,
    pub delayed_event_completed: ReplicatedFieldHandler<bool>,

    pub hub: ReplicatedState,
}
