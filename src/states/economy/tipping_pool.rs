//! Tipping-pool point totals and payout timing replication.

use crate::{az_rtti, replicated_state, type_registry};

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

#[replicated_state]
#[derive(Debug, Clone, Default)]
#[az_rtti("F7B56641-F8C3-41A9-83B2-13AC4F9843F9")]
#[type_registry(3681)]
pub struct TippingPoolComponentReplicatedState {
    pub pool_ids: ReplicatedContainer<Vec<u32>, 50>,
    pub pool_counts: ReplicatedContainer<Vec<u16>, 50>,
    pub pool_categories: ReplicatedContainer<Vec<u8>, 50>,
    pub point_entries: ReplicatedContainer<Vec<TippingPoolPointEntry>, 1000>,
}

impl TippingPoolComponentReplicatedState {
    #[must_use]
    pub fn empty_baseline(sequence: u64) -> Self {
        Self {
            pool_ids: ReplicatedContainer::new(sequence, Vec::new()),
            pool_counts: ReplicatedContainer::new(sequence, Vec::new()),
            pool_categories: ReplicatedContainer::new(sequence, Vec::new()),
            point_entries: ReplicatedContainer::new(sequence, Vec::new()),
            ..Default::default()
        }
    }

    pub fn apply_snapshot(&mut self, snapshot: TippingPoolSnapshot) {
        self.pool_ids = snapshot.pool_ids;
        self.pool_counts = snapshot.pool_counts;
        self.pool_categories = snapshot.pool_categories;
        self.point_entries = snapshot.point_entries;
    }
}
