//! Managed item storage replication for inventory-backed systems.

use crate::{az_rtti, replicated_state, type_registry};

use crate::Marshaler;
use crate::serialize::{Change, IndexMap, ReplicatedContainer};

use super::item_transform::{ItemTransformItemDescriptor, ItemTransformSnapshot};

type ItemManagementItemDescriptor = ItemTransformItemDescriptor;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Marshaler)]
pub struct ItemManagementStorageKey {
    pub bytes: [u8; 16],
}

impl ItemManagementStorageKey {
    #[must_use]
    pub fn from_slice(bytes: &[u8]) -> Option<Self> {
        let bytes: [u8; 16] = bytes.try_into().ok()?;
        Some(Self { bytes })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct ItemStorageItems {
    pub items: Vec<ItemManagementItemDescriptor>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ItemManagementSnapshot {
    pub weight_map: ReplicatedContainer<IndexMap<ItemManagementStorageKey, u32>>,
    pub slot_count_map: ReplicatedContainer<IndexMap<ItemManagementStorageKey, u32>>,
}

#[replicated_state]
#[derive(Debug, Clone, Default)]
#[az_rtti("A7933D94-4E0B-4711-BE2D-EA22000CCF06")]
#[type_registry(5437)]
pub struct ItemManagementComponentReplicatedState {
    #[replicated_state(group = 1)]
    pub owned_items: ReplicatedContainer<IndexMap<u16, ItemTransformItemDescriptor>>,
}

impl ItemManagementComponentReplicatedState {
    pub fn apply_item_transform_snapshot(&mut self, snapshot: ItemTransformSnapshot) {
        self.owned_items = ReplicatedContainer::new(
            snapshot.owned_items_sequence,
            snapshot
                .owned_items
                .into_iter()
                .map(|entry| (entry.paperdoll_slot, entry.item))
                .collect(),
        );
    }

    #[must_use]
    pub fn owned_item_delta(&self, preferred_slot: u16) -> Self {
        let mut state = Self::default();
        let items = &self.owned_items;
        if !items.has_value() {
            return state;
        }

        let entry = items
            .values()
            .iter()
            .find(|(slot, _)| **slot == preferred_slot)
            .or_else(|| items.values().iter().next())
            .map(|(slot, item)| Change::update(*slot, item.clone(), items.last_modified()));

        if let Some(entry) = entry {
            state.owned_items = ReplicatedContainer::delta(vec![entry]);
        }
        state
    }
}
