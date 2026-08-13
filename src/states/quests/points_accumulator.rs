//! Points accumulator replicated state.

pub use crate::generated::states::PointsAccumulatorComponentReplicatedState;
use crate::source::TimePoint;

impl PointsAccumulatorComponentReplicatedState {
    pub fn set_points(&mut self, current: f32, max: u32, zeroed_at: u64) {
        self.num_points_0.set_value(current);
        self.max_num_points_0.set_value(max);
        self.time_when_points_zeroed_0.set_value(TimePoint {
            nanoseconds_since_server_start: zeroed_at,
        });
    }
}
