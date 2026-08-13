//! Gatherable resource controller state replication.

use crate::Marshaler;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Marshaler)]
#[repr(transparent)]
pub struct ReplicatedGatherableState(u8);

impl ReplicatedGatherableState {
    #[must_use]
    pub const fn new(value: u8) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn value(self) -> u8 {
        self.0
    }
}
pub use crate::generated::states::GatherableControllerReplicatedState;
