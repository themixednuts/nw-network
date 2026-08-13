//! Quest progression and categorical progression replication.

use crate::serialize::ReplicatedContainer;

pub use crate::generated::states::ProgressionComponentReplicatedState;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CategoricalProgressionSnapshot {
    pub progression_ids: ReplicatedContainer<Vec<u32>>,
    pub ranks: ReplicatedContainer<Vec<u16>>,
    pub points: ReplicatedContainer<Vec<u64>>,
}
pub use crate::generated::states::CategoricalProgressionReplicatedState;
