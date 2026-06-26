use std::collections::HashMap;

use crate::Marshaler;
use crate::serialize::{ReplicatedFieldHandler, ReplicatedMap, ReplicatedVec};

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

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("756DEAE5-A1F0-4863-BE20-B44D871C46A1")]
#[::nw_network::type_registry(982)]
pub struct LootTrackerComponentReplicatedState {
    pub loot_data_map: ReplicatedMap<LootTrackerKey, LootRollData>,
    pub loot_collectibles: ReplicatedVec<u64>,
    pub failed_roll_bonus_percent: ReplicatedFieldHandler<f32>,
    pub slayer_script_data_map: ReplicatedMap<LootTrackerKey, SlayerScriptLootData>,
    pub loot_divert_map: ReplicatedMap<u32, LootDivertMapValue>,
    pub loot_limit_data_map: ReplicatedMap<u32, LootLimitStateData>,
}

impl LootTrackerComponentReplicatedState {
    pub fn apply_snapshot(&mut self, snapshot: LootTrackerSnapshot) {
        if snapshot.loot_data_map_sequence > 0 {
            self.loot_data_map =
                ReplicatedMap::new(snapshot.loot_data_map_sequence, HashMap::new());
        }

        if snapshot.loot_divert_map_sequence > 0 || !snapshot.loot_diverts.is_empty() {
            self.loot_divert_map = ReplicatedMap::new(
                snapshot.loot_divert_map_sequence,
                snapshot
                    .loot_diverts
                    .into_iter()
                    .map(|entry| (entry.key, entry.data))
                    .collect(),
            );
        }
    }
}
