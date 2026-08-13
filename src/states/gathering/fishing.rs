//! Fishing state transition replication.

use crate::Marshaler;

pub const MAX_FISHING_STATE_TRANSITION_CHANGES: usize = 0x3fff;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Marshaler)]
pub struct FishingStateTransition {
    pub state: u16,
    pub value: u32,
}
pub use crate::generated::states::FishingComponentReplicatedState;
