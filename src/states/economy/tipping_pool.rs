//! Tipping-pool point totals and payout timing replication.

use crate::Marshaler;
use crate::serialize::ReplicatedContainer;

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct TippingPoolPointEntry {
    pub pool_id: u32,
    pub point_id: u32,
    pub count: u16,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TippingPoolSnapshot {
    pub pool_ids: ReplicatedContainer<Vec<u32>, 50>,
    pub pool_counts: ReplicatedContainer<Vec<u16>, 50>,
    pub pool_categories: ReplicatedContainer<Vec<u8>, 50>,
    pub point_entries: ReplicatedContainer<Vec<TippingPoolPointEntry>, 1000>,
}
pub use crate::generated::states::TippingPoolComponentReplicatedState;
