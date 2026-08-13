//! Item skin and dye replication.

use crate::Marshaler;

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
pub use crate::generated::states::ItemSkinningComponentReplicatedState;
