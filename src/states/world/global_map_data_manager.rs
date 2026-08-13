//! Global map entry replication.

use std::ops::{Deref, DerefMut};

use glam::Vec2;
use indexmap::IndexMap;

use crate::Marshaler;
use crate::hub::SequenceNumber;
use crate::serialize::ReplicatedContainer;

/// Global-map entry payload.
#[derive(Debug, Clone, Default, PartialEq, Marshaler)]
pub struct GlobalMapData {
    pub position: Vec2,
    pub field_08: u16,
    pub field_0c: u32,
    pub field_10: bool,
}

/// Replicated global-map entries keyed by global map ID.
#[derive(Debug, Clone, Default, PartialEq, Marshaler)]
#[repr(transparent)]
pub struct GlobalMapDataMap(ReplicatedContainer<IndexMap<u64, GlobalMapData>>);

impl GlobalMapDataMap {
    #[must_use]
    pub fn new(sequence: impl Into<SequenceNumber>, values: IndexMap<u64, GlobalMapData>) -> Self {
        Self(ReplicatedContainer::new(sequence, values))
    }
}

impl Deref for GlobalMapDataMap {
    type Target = ReplicatedContainer<IndexMap<u64, GlobalMapData>>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for GlobalMapDataMap {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Replicated global-map state.
pub use crate::generated::states::GlobalMapDataManagerComponentReplicatedState;
