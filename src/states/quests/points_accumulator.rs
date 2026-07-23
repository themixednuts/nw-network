//! Points accumulator replicated state.

use crate::serialize::ReplicatedFieldHandler;
use crate::{az_rtti, replicated_state, type_registry};

#[replicated_state]
#[derive(Debug, Clone, Default)]
#[az_rtti("63AD3B5A-3E2E-4923-ACCD-1DA221431EE0")]
#[type_registry(6951)]
pub struct PointsAccumulatorComponentReplicatedState {
    pub num_points0: ReplicatedFieldHandler<u32>,
    pub max_num_points0: ReplicatedFieldHandler<u32>,
    pub time_when_points_zeroed0: ReplicatedFieldHandler<u64>,
}

impl PointsAccumulatorComponentReplicatedState {
    pub fn set_points(&mut self, current: u32, max: u32, zeroed_at: u64) {
        self.num_points0.set_value(current);
        self.max_num_points0.set_value(max);
        self.time_when_points_zeroed0.set_value(zeroed_at);
    }
}
