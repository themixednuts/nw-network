use std::ops::{Deref, DerefMut};

use glam::Vec2;
use indexmap::IndexMap;

use crate::Marshaler;
use crate::hub::SequenceNumber;
use crate::serialize::{ReplicatedFieldHandler, ReplicatedIndexMap};

/// Value payload of one global-map entry.
#[derive(Debug, Clone, Default, PartialEq, Marshaler)]
pub struct GlobalMapDataValue {
    pub position: Vec2,
    pub field_08: u16,
    pub field_0c: u32,
    pub field_10: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Marshaler)]
#[repr(transparent)]
pub struct GlobalMapData(ReplicatedIndexMap<u64, GlobalMapDataValue>);

impl GlobalMapData {
    #[must_use]
    pub fn new(
        sequence: impl Into<SequenceNumber>,
        values: IndexMap<u64, GlobalMapDataValue>,
    ) -> Self {
        Self(ReplicatedIndexMap::new(sequence, values))
    }
}

impl Deref for GlobalMapData {
    type Target = ReplicatedIndexMap<u64, GlobalMapDataValue>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for GlobalMapData {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Replicated global-map state.
#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("111AEBB0-4F23-4914-B732-A349CCBD82D4")]
#[::nw_network::type_registry(3780)]
pub struct GlobalMapDataManagerComponentReplicatedState {
    pub global_map_data: ReplicatedFieldHandler<GlobalMapData>,
}
