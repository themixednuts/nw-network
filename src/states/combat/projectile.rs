use crate::hub::{FragmentCategory, ReplicatedState};
use crate::serialize::{
    HalfF32Marshaler, HalfVec3Marshaler, MarshalerError, PositionAnchorMarshaler, ReadBuffer,
    ReplicatedFieldHandler, ReplicatedVec, VlqU32Marshaler, WriteBuffer,
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
#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ChunkMarshaler,
    nw_network_derive::AzRtti,
    nw_network_derive::Fragment,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("39B4C919-3A6D-46B5-92D0-3B4ACB284B1D")]
#[type_registry(16)]
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

    pub hub: ReplicatedState,
}

impl ProjectileReplicatedState {
    fn unmarshal_fields(&mut self, rb: &mut ReadBuffer) -> Result<(), MarshalerError> {
        crate::unmarshal_replicated_fields!(
            rb,
            self.position_anchor,
            self.anchor_height_delta,
            self.spawn_velocity,
            self.gravity_override,
            self.time_anchor_us,
            self.time_offset_start_accel_ms,
            self.time_offset_bounce_ms,
            self.collided_with_gde_id,
            self.collided_with_hit_volume_idx,
            self.flags,
            self.owner,
            self.ranged_attack_index,
            self.weapon_effect_crc,
            self.ammo_id_crc,
            self.damage_table_id,
            self.damage_table_row_index,
            self.piercing_hits,
            self.magnet_entity_id,
            self.magnet_root_offset,
            self.combat_settings_exp_idx,
        )
    }

    fn marshal_fields(&self, wb: &mut WriteBuffer) {
        crate::marshal_replicated_fields!(
            wb,
            self.position_anchor,
            self.anchor_height_delta,
            self.spawn_velocity,
            self.gravity_override,
            self.time_anchor_us,
            self.time_offset_start_accel_ms,
            self.time_offset_bounce_ms,
            self.collided_with_gde_id,
            self.collided_with_hit_volume_idx,
            self.flags,
            self.owner,
            self.ranged_attack_index,
            self.weapon_effect_crc,
            self.ammo_id_crc,
            self.damage_table_id,
            self.damage_table_row_index,
            self.piercing_hits,
            self.magnet_entity_id,
            self.magnet_root_offset,
            self.combat_settings_exp_idx,
        );
    }
}

crate::impl_hub_fragment!(
    ProjectileReplicatedState,
    hub = hub,
    marshal = marshal_fields,
    unmarshal = unmarshal_fields,
    category = FragmentCategory::Projectile,
    world_position = position_anchor,
);
