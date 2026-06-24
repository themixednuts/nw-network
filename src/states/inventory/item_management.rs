use crate::Marshaler;
use crate::hub::ReplicatedState;
use crate::serialize::{ReplicatedFieldHandler, ReplicatedMap};

type ItemManagementItemDescriptor = super::item_transform::ItemTransformItemDescriptor;

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
    pub weight_map: ReplicatedMap<ItemManagementStorageKey, u32>,
    pub slot_count_map: ReplicatedMap<ItemManagementStorageKey, u32>,
}

#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ReplicatedState,
    nw_network_derive::AzRtti,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("37F7555D-8476-410A-9753-850945075374")]
#[type_registry(2938)]
pub struct ItemManagementComponentReplicatedState {
    pub global_item_map: ReplicatedMap<ItemManagementStorageKey, ItemStorageItems>,
    pub overflow_item_count: ReplicatedFieldHandler<u32>,
    pub weight_map: ReplicatedMap<ItemManagementStorageKey, u32>,
    pub slot_count_map: ReplicatedMap<ItemManagementStorageKey, u32>,

    pub hub: ReplicatedState,
}

impl ItemManagementComponentReplicatedState {
    pub fn apply_snapshot(&mut self, snapshot: ItemManagementSnapshot) {
        self.weight_map = snapshot.weight_map;
        self.slot_count_map = snapshot.slot_count_map;
    }
}
