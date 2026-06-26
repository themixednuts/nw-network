use crate::Marshaler;
use crate::serialize::{HalfF32, MarshalerError, ReadBuffer, ReplicatedMap, VlqU64, WriteBuffer};

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

impl Marshaler for RemoteStatusEffectData {
    fn marshal(&self, wb: &mut WriteBuffer) {
        let raw = (self.value & 0x7f) | if self.high_bit_set { 0x80 } else { 0 };
        raw.marshal(wb);
    }

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
pub struct TerritoryStatusEffectData {
    pub status_effect_id: u32,
    pub end_time: u64,
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
    pub local_effects_map: ReplicatedMap<u16, StatusEffectInstanceData>,
    pub effects_map: ReplicatedMap<u16, StatusEffectInstanceData>,
    pub remote_effects_map: ReplicatedMap<u16, RemoteStatusEffectData>,
    pub lightweight_local_effects_map: ReplicatedMap<u16, LightweightStatusEffectData>,
    pub territory_status_effects: ReplicatedMap<VlqU64, TerritoryStatusEffectData>,
    pub dynamic_scaling_data: ReplicatedMap<VlqU64, DynamicScalingStatusEffectData>,
    pub active_tray_icons: ReplicatedMap<VlqU64, ActiveTrayIconData>,
    pub local_replicated_update_counts: ReplicatedMap<u32, u16>,
    pub remote_replicated_update_counts: ReplicatedMap<u32, u16>,
}

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("E36B9CC4-082E-40F0-BA4F-5C5BE9CD3C16")]
#[::nw_network::type_registry(4236)]
pub struct StatusEffectsComponentReplicatedState {
    #[replicated_state(group = 1)]
    pub local_effects_map: ReplicatedMap<u16, StatusEffectInstanceData>,
    #[replicated_state(group = 3)]
    pub effects_map: ReplicatedMap<u16, StatusEffectInstanceData>,
    #[replicated_state(group = 3)]
    pub remote_effects_map: ReplicatedMap<u16, RemoteStatusEffectData>,
    #[replicated_state(group = 1)]
    pub lightweight_local_effects_map: ReplicatedMap<u16, LightweightStatusEffectData>,
    #[replicated_state(group = 2)]
    pub territory_status_effects: ReplicatedMap<VlqU64, TerritoryStatusEffectData>,
    #[replicated_state(group = 2)]
    pub dynamic_scaling_data: ReplicatedMap<VlqU64, DynamicScalingStatusEffectData>,
    #[replicated_state(group = 1)]
    pub active_tray_icons: ReplicatedMap<VlqU64, ActiveTrayIconData>,
    #[replicated_state(group = 2)]
    pub local_replicated_update_counts: ReplicatedMap<u32, u16>,
    #[replicated_state(group = 2)]
    pub remote_replicated_update_counts: ReplicatedMap<u32, u16>,
}

impl StatusEffectsComponentReplicatedState {
    pub fn apply_snapshot(&mut self, snapshot: StatusEffectsSnapshot) {
        self.local_effects_map = snapshot.local_effects_map;
        self.effects_map = snapshot.effects_map;
        self.remote_effects_map = snapshot.remote_effects_map;
        self.lightweight_local_effects_map = snapshot.lightweight_local_effects_map;
        self.territory_status_effects = snapshot.territory_status_effects;
        self.dynamic_scaling_data = snapshot.dynamic_scaling_data;
        self.active_tray_icons = snapshot.active_tray_icons;
        self.local_replicated_update_counts = snapshot.local_replicated_update_counts;
        self.remote_replicated_update_counts = snapshot.remote_replicated_update_counts;
    }
}
