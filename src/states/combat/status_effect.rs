//! Status-effect instance and tray-icon replication.
use crate::serialize::marshaler::{Marshal, Unmarshal};

use crate::serialize::{
    HalfF32, IndexMap, MarshalerError, ReadBuffer, ReplicatedContainer, VlqU64, WriteBuffer,
};
use crate::{Crc32, Marshaler, WallClockTimePoint};

#[derive(Debug, Clone, Copy, Default, PartialEq, Marshaler)]
pub struct StatusEffectInstanceData {
    pub stack_key: u64,
    pub duration: HalfF32,
    pub stack_count: u8,
    pub flags: u8,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RemoteStatusEffectData {
    pub high_bit_set: bool,
    pub value: u8,
}

impl Marshal for RemoteStatusEffectData {
    fn marshal(&self, wb: &mut WriteBuffer) {
        let raw = (self.value & 0x7f) | if self.high_bit_set { 0x80 } else { 0 };
        raw.marshal(wb);
    }
}

impl Unmarshal for RemoteStatusEffectData {
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let raw = u8::unmarshal(rb)?;
        Ok(Self {
            high_bit_set: (raw & 0x80) != 0,
            value: raw & 0x7f,
        })
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Marshaler)]
pub struct LightweightStatusEffectData {
    pub stack_key: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Marshaler)]
pub struct DynamicScalingStatusEffectData {
    pub scaling_id: u32,
    pub source_id: u32,
    pub scale: f32,
    pub source_entity_key: u64,
    pub target_entity_key: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Marshaler)]
pub struct TerritoryStatusEffect {
    pub effect_id: Crc32,
    pub end_time_stamp: WallClockTimePoint,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Marshaler)]
pub struct ActiveTrayIconData {
    pub icon_id: u32,
    pub source_key: u64,
    pub priority: u8,
    pub duration_scale: f32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct StatusEffectsSnapshot {
    pub local_effects_map: ReplicatedContainer<IndexMap<u16, StatusEffectInstanceData>>,
    pub effects_map: ReplicatedContainer<IndexMap<u16, StatusEffectInstanceData>>,
    pub remote_effects_map: ReplicatedContainer<IndexMap<u16, RemoteStatusEffectData>>,
    pub lightweight_local_effects_map:
        ReplicatedContainer<IndexMap<u16, LightweightStatusEffectData>>,
    pub territory_status_effects: ReplicatedContainer<Vec<TerritoryStatusEffect>>,
    pub dynamic_scaling_data: ReplicatedContainer<IndexMap<VlqU64, DynamicScalingStatusEffectData>>,
    pub active_tray_icons: ReplicatedContainer<IndexMap<VlqU64, ActiveTrayIconData>>,
    pub local_replicated_update_counts: ReplicatedContainer<IndexMap<u32, u16>>,
    pub remote_replicated_update_counts: ReplicatedContainer<IndexMap<u32, u16>>,
}
pub use crate::generated::states::StatusEffectsComponentReplicatedState;
