use crate::hub::ReplicatedState;
use crate::serialize::ReplicatedFieldHandler;
use crate::types::TimePoint;

/// Item generator replicated state.
#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ReplicatedState,
    nw_network_derive::AzRtti,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("164A2CCF-156D-4314-88BD-F2C253F01647")]
#[type_registry(3002)]
pub struct ItemGenerationComponentReplicatedState {
    pub active: ReplicatedFieldHandler<bool>,
    pub item_generation_time: ReplicatedFieldHandler<TimePoint>,

    pub hub: ReplicatedState,
}
