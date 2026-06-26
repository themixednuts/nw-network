use crate::Marshaler;
use crate::serialize::ReplicatedVec;

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct TippingPoolPointEntry {
    pub pool_id: u32,
    pub point_id: u32,
    pub count: u16,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TippingPoolSnapshot {
    pub pool_ids: ReplicatedVec<u32, 50>,
    pub pool_counts: ReplicatedVec<u16, 50>,
    pub pool_categories: ReplicatedVec<u8, 50>,
    pub point_entries: ReplicatedVec<TippingPoolPointEntry, 1000>,
}

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("F7B56641-F8C3-41A9-83B2-13AC4F9843F9")]
#[::nw_network::type_registry(3681)]
pub struct TippingPoolComponentReplicatedState {
    pub pool_ids: ReplicatedVec<u32, 50>,
    pub pool_counts: ReplicatedVec<u16, 50>,
    pub pool_categories: ReplicatedVec<u8, 50>,
    pub point_entries: ReplicatedVec<TippingPoolPointEntry, 1000>,
}

impl TippingPoolComponentReplicatedState {
    #[must_use]
    pub fn empty_baseline(sequence: u64) -> Self {
        Self {
            pool_ids: ReplicatedVec::new(sequence, Vec::new()),
            pool_counts: ReplicatedVec::new(sequence, Vec::new()),
            pool_categories: ReplicatedVec::new(sequence, Vec::new()),
            point_entries: ReplicatedVec::new(sequence, Vec::new()),
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
