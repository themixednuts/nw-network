use glam::Vec3;

use crate::serialize::{
    HalfF32Marshaler, HalfVec3Marshaler, PositionAnchorMarshaler, ReplicatedFieldHandler,
    ReplicatedVec, VlqU32Marshaler,
};
use crate::{GdeId, Marshaler};

/// Value stored by [`ProjectileReplicatedState::piercing_hits`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct PiercingHitData {
    pub target_entity: u64,
    pub flag: u8,
    pub volume_index: u16,
}

/// Projectile ranged-attack replicated state.
#[::nw_network::replicated_state(
    category = "projectile",
    world_position = "world_position_anchor()"
)]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("39B4C919-3A6D-46B5-92D0-3B4ACB284B1D")]
#[::nw_network::type_registry(16)]
pub struct ProjectileReplicatedState {
    pub position_anchor: ReplicatedFieldHandler<(f32, f32, f32), PositionAnchorMarshaler>,
    pub anchor_height_delta: ReplicatedFieldHandler<f32, HalfF32Marshaler>,
    pub spawn_velocity: ReplicatedFieldHandler<(f32, f32, f32), HalfVec3Marshaler>,
    pub gravity_override: ReplicatedFieldHandler<f32, HalfF32Marshaler>,
    pub time_anchor_us: ReplicatedFieldHandler<u64>,
    pub time_offset_start_accel_ms: ReplicatedFieldHandler<u16>,
    pub time_offset_bounce_ms: ReplicatedFieldHandler<u16>,
    pub collided_with_gde_id: ReplicatedFieldHandler<GdeId>,
    pub collided_with_hit_volume_idx: ReplicatedFieldHandler<u8>,
    pub flags: ReplicatedFieldHandler<u8>,
    pub owner: ReplicatedFieldHandler<u64>,
    pub ranged_attack_index: ReplicatedFieldHandler<u32, VlqU32Marshaler>,
    pub weapon_effect_crc: ReplicatedFieldHandler<u32>,
    pub ammo_id_crc: ReplicatedFieldHandler<u32, VlqU32Marshaler>,
    pub damage_table_id: ReplicatedFieldHandler<u32>,
    pub damage_table_row_index: ReplicatedFieldHandler<u16>,
    pub piercing_hits: ReplicatedVec<PiercingHitData>,
    pub magnet_entity_id: ReplicatedFieldHandler<u64>,
    pub magnet_root_offset: ReplicatedFieldHandler<(f32, f32, f32), HalfVec3Marshaler>,
    pub combat_settings_exp_idx: ReplicatedFieldHandler<u8>,
}

impl ProjectileReplicatedState {
    fn world_position_anchor(&self) -> Option<Vec3> {
        self.position_anchor
            .value()
            .copied()
            .map(|(x, y, height)| Vec3::new(x, height, y))
    }
}
