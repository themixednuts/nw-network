//! Loot roll, diversion, and per-entity loot-limit replication.

use crate::Marshaler;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Marshaler)]
pub struct LootTrackerKey(pub [u8; 16]);

#[derive(Debug, Clone, Default, PartialEq, Marshaler)]
pub struct LootRollData {
    pub loot_table_id: u32,
    pub roll_id: u32,
    pub source_time: u64,
    pub expiration_time: u64,
    pub active: bool,
    pub weights: [f32; 3],
    pub item_id: u32,
    pub tier: u8,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct SlayerScriptLootData {
    pub state: u8,
    pub timestamp: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct LootDivertMapValue {
    pub divert_type: u8,
    pub target_id: u64,
    pub quantity: u16,
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct LootLimitStateData {
    pub window_start: u64,
    pub window_end: u64,
    pub limit: u16,
    pub state: u8,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LootDivertEntry {
    pub key: u32,
    pub data: LootDivertMapValue,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LootTrackerSnapshot {
    pub loot_data_map_sequence: u64,
    pub loot_divert_map_sequence: u64,
    pub loot_diverts: Vec<LootDivertEntry>,
}
pub use crate::generated::states::LootTrackerComponentReplicatedState;
