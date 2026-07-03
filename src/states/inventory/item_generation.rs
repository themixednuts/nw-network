//! Item generator selection replication.

use crate::{az_rtti, replicated_state, type_registry};

use crate::serialize::ReplicatedFieldHandler;
use crate::types::TimePoint;

/// Item generator replicated state.
#[replicated_state]
#[derive(Debug, Clone, Default)]
#[az_rtti("164A2CCF-156D-4314-88BD-F2C253F01647")]
#[type_registry(3002)]
pub struct ItemGenerationComponentReplicatedState {
    pub active: ReplicatedFieldHandler<bool>,
    pub item_generation_time: ReplicatedFieldHandler<TimePoint>,
}
