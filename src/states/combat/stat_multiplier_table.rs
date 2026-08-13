//! Stat multiplier table replication.

use crate::Marshaler;
use crate::serialize::{IndexMap, ReplicatedContainer};

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct StatMultiplierValue {
    pub amount: u32,
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StatMultiplierSnapshot {
    pub multiplier_table: ReplicatedContainer<IndexMap<u8, StatMultiplierValue>>,
    pub stamina_cost_reduction_multipliers: ReplicatedContainer<IndexMap<u32, u32>>,
    pub xp_increase_multipliers: ReplicatedContainer<IndexMap<u32, u32>>,
    pub remote_multiplier_table: ReplicatedContainer<IndexMap<u8, StatMultiplierValue>>,
}
pub use crate::generated::states::StatMultiplierTableComponentReplicatedState;
