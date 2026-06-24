use crate::hub::ReplicatedState;
use crate::serialize::{Change, ReplicatedMap};

pub type ItemTransformItemDescriptor = super::item_descriptor::ReplicatedItemDescriptor;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OwnedItemEntry {
    pub paperdoll_slot: u16,
    pub item: ItemTransformItemDescriptor,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ItemTransformSnapshot {
    pub owned_items_sequence: u64,
    pub owned_items: Vec<OwnedItemEntry>,
}

#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ReplicatedState,
    nw_network_derive::AzRtti,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("A7933D94-4E0B-4711-BE2D-EA22000CCF06")]
#[type_registry(5437)]
pub struct ItemTransformComponentReplicatedState {
    #[replicated_state(group = 1)]
    pub owned_items: ReplicatedMap<u16, ItemTransformItemDescriptor>,

    pub hub: ReplicatedState,
}

impl ItemTransformComponentReplicatedState {
    pub fn apply_snapshot(&mut self, snapshot: ItemTransformSnapshot) {
        self.owned_items = ReplicatedMap::new(
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
            state.owned_items = ReplicatedMap::delta(vec![entry]);
        }
        state
    }
}
