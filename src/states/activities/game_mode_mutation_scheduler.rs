//! Game-mode mutation scheduling state replicated to clients.

use crate::Marshaler;
use crate::serialize::IndexMap;

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct GameModeMutationSet {
    pub curse_mutation_id: u32,
    pub promotion_mutation_id: u32,
    pub elemental_mutation_id: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GameModeMutationSnapshot {
    pub sets_sequence: u64,
    pub sets: IndexMap<u32, GameModeMutationSet>,
    pub cadence_start_time: u64,
}
pub use crate::generated::states::GameModeMutationSchedulerReplicatedState;
