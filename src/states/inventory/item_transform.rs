//! Item transformation input and output replication.

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
