//! Item skin and dye replication.

use crate::{az_rtti, replicated_state, type_registry};

use crate::Marshaler;
use crate::serialize::{IndexMap, ReplicatedContainer};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Marshaler)]
pub struct ItemSkinDyeData {
    pub primary: u8,
    pub secondary: u8,
    pub accent: u8,
    pub tint: u8,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct SkinDyeEntry {
    pub skin_id: u32,
    pub dye: ItemSkinDyeData,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ItemSkinningSnapshot {
    pub enabled_skins_sequence: u64,
    pub skin_dye_data_sequence: u64,
    pub enabled_skin_ids: Vec<u64>,
    pub skin_dyes: Vec<SkinDyeEntry>,
}

#[replicated_state]
#[derive(Debug, Clone, Default)]
#[az_rtti("E1EBCA63-1759-477C-BBE7-61B8A471F5BB")]
#[type_registry(3765)]
pub struct ItemSkinningComponentReplicatedState {
    #[replicated_state(group = 1)]
    pub enabled_item_skins: ReplicatedContainer<Vec<u64>>,
    #[replicated_state(group = 1)]
    pub skin_dye_data: ReplicatedContainer<IndexMap<u32, ItemSkinDyeData>>,
}

impl ItemSkinningComponentReplicatedState {
    pub fn apply_snapshot(&mut self, snapshot: ItemSkinningSnapshot) {
        self.enabled_item_skins =
            ReplicatedContainer::new(snapshot.enabled_skins_sequence, snapshot.enabled_skin_ids);
        self.skin_dye_data = ReplicatedContainer::new(
            snapshot.skin_dye_data_sequence,
            snapshot
                .skin_dyes
                .into_iter()
                .map(|entry| (entry.skin_id, entry.dye))
                .collect(),
        );
    }
}
