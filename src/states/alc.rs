use arrayvec::ArrayVec;
use bevy_math::bounding::Aabb2d;
use glam::{Quat, Vec2, Vec3};
use std::fmt;

use crate::hub::{
    ClientActorHash, DynFragment, FixedReplicatedState, FixedReplicatedStateFields, Fragment,
    FragmentBase, FragmentCategory, GroupIndex, MarshalContext, SequenceNumber,
};
use crate::serialize::{
    Codec, DeltaCompressedCounterHandler, DeltaCompressedReplicatedFieldHandler,
    FloatTimerDeltaReplicatedField, HalfF32Marshaler, Marshaler, MarshalerError,
    PositionAnchorMarshaler, QuantizedRelativePosition, QuatSmallestThreeQuantized, ReadBuffer,
    ReplicatedFieldHandler, ReplicatedFieldHandlerBase, VlqU32, WriteBuffer, quantize_with_range,
    unquantize_with_range,
};

pub const ACTION_STATE_MAX_SCOPE_SIZE: usize = 16;
pub const SCOPE_INFO_TOTAL_BYTES: usize = 11 * ACTION_STATE_MAX_SCOPE_SIZE;
pub const SCOPE_TIME_TOTAL_BYTES: usize = 2 * ACTION_STATE_MAX_SCOPE_SIZE;

pub type ScopeInfoBlob = ArrayVec<u8, SCOPE_INFO_TOTAL_BYTES>;
pub type ScopeTimeBlob = ArrayVec<u8, SCOPE_TIME_TOTAL_BYTES>;
pub type ScopelessInfoBlob = ArrayVec<u8, SCOPE_INFO_TOTAL_BYTES>;
pub type ScopelessTimeBlob = ArrayVec<u8, SCOPE_TIME_TOTAL_BYTES>;
pub type SlayerScriptBlob = Vec<u8>;
pub type SlayerScriptField = Option<SlayerScriptBlob>;

const ALC_REPLICATION_GROUPS: usize = 2;
const ALC_FIELDS_PER_GROUP: usize = 54;
const ALC_CLIENT_WHITELIST_SIZE: usize = 1;
type AlcFixedState =
    FixedReplicatedState<ALC_REPLICATION_GROUPS, ALC_FIELDS_PER_GROUP, ALC_CLIENT_WHITELIST_SIZE>;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct AlcPositionAnchor {
    pub x: f32,
    pub y: f32,
    pub height: f32,
}

impl AlcPositionAnchor {
    #[must_use]
    pub const fn new(x: f32, y: f32, height: f32) -> Self {
        Self { x, y, height }
    }

    #[must_use]
    pub fn as_vec3(self) -> Vec3 {
        Vec3::new(self.x, self.y, self.height)
    }

    #[must_use]
    pub fn from_vec3(value: Vec3) -> Self {
        Self::new(value.x, value.y, value.z)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct AlcPositionAnchorMarshaler;

impl Codec<AlcPositionAnchor> for AlcPositionAnchorMarshaler {
    const MARSHAL_SIZE: usize = <PositionAnchorMarshaler as Codec<(f32, f32, f32)>>::MARSHAL_SIZE;

    fn marshal(value: &AlcPositionAnchor, wb: &mut WriteBuffer) {
        PositionAnchorMarshaler::marshal(&(value.x, value.y, value.height), wb);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<AlcPositionAnchor, MarshalerError> {
        let (x, y, height) = PositionAnchorMarshaler::unmarshal(rb)?;
        Ok(AlcPositionAnchor::new(x, y, height))
    }
}

#[derive(Debug, Clone, Default)]
pub struct AlcWorldPositionHandler {
    pub absolute_portion: ReplicatedFieldHandler<AlcPositionAnchor, AlcPositionAnchorMarshaler>,
    pub quantized_relative_portion: ReplicatedFieldHandler<QuantizedRelativePosition>,
    pub quantization: ReplicatedFieldHandler<f32>,
}

impl AlcWorldPositionHandler {
    #[must_use]
    pub fn value(&self) -> Option<Vec3> {
        let anchor = self.absolute_portion.value().copied()?.as_vec3();
        let delta = self
            .quantized_relative_portion
            .value()
            .copied()
            .unwrap_or_default();
        if delta.is_zero() {
            return Some(anchor);
        }
        let quantization = self.quantization.value().copied()?;
        let [x, y, z] = delta.quantized_values;
        Some(
            anchor
                + Vec3::new(
                    Self::unquantize_delta(x, quantization),
                    Self::unquantize_delta(y, quantization),
                    Self::unquantize_delta(z, quantization),
                ),
        )
    }

    #[must_use]
    pub fn is_field_valid(&self) -> bool {
        self.absolute_portion.is_field_valid()
    }

    pub fn set_value(&mut self, value: AlcPositionAnchor, quantization: f32) {
        let needs_anchor = !self.absolute_portion.is_field_valid()
            || self.quantization.value().copied() != Some(quantization)
            || !self.is_within_delta(value.as_vec3(), quantization);

        if needs_anchor {
            self.absolute_portion.set_value(value);
            self.quantized_relative_portion
                .set_value(QuantizedRelativePosition::default());
            self.quantization.set_value(quantization);
            return;
        }

        let anchor = self
            .absolute_portion
            .value()
            .copied()
            .map_or(Vec3::ZERO, AlcPositionAnchor::as_vec3);
        let diff = value.as_vec3() - anchor;
        self.quantized_relative_portion
            .set_value(QuantizedRelativePosition::new([
                Self::quantize_delta(diff.x, quantization),
                Self::quantize_delta(diff.y, quantization),
                Self::quantize_delta(diff.z, quantization),
            ]));
    }

    fn is_within_delta(&self, value: Vec3, quantization: f32) -> bool {
        let Some(anchor) = self
            .absolute_portion
            .value()
            .copied()
            .map(AlcPositionAnchor::as_vec3)
        else {
            return false;
        };
        let abs_diff = (anchor - value).abs();
        abs_diff.x < quantization && abs_diff.y < quantization && abs_diff.z < quantization
    }

    fn quantize_delta(value: f32, quantization: f32) -> u8 {
        quantize_with_range(value, quantization)
    }

    fn unquantize_delta(value: u8, quantization: f32) -> f32 {
        unquantize_with_range(value, quantization)
    }
}

pub type SlayerAnimationFieldHandler =
    DeltaCompressedReplicatedFieldHandler<f32, 2, HalfF32Marshaler>;

#[derive(Debug, Clone, Copy, Default)]
pub struct PackedQuaternionMarshaller;

impl Codec<Quat> for PackedQuaternionMarshaller {
    fn marshal(value: &Quat, wb: &mut WriteBuffer) {
        QuatSmallestThreeQuantized::from_xyzw(value.x, value.y, value.z, value.w).marshal(wb);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Quat, MarshalerError> {
        Ok(QuatSmallestThreeQuantized::unmarshal(rb)?.as_quat())
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct PackedNormalizedVec3Marshaller;

impl Codec<Vec3> for PackedNormalizedVec3Marshaller {
    fn marshal(value: &Vec3, wb: &mut WriteBuffer) {
        QuatSmallestThreeQuantized::from_xyzw(value.x, value.y, value.z, 0.0).marshal(wb);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Vec3, MarshalerError> {
        let packed = QuatSmallestThreeQuantized::unmarshal(rb)?;
        Ok(Vec3::new(
            packed.components[0],
            packed.components[1],
            packed.components[2],
        ))
    }
}

pub type TagStateData = [u8; 12];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridAccessibility {
    pub raw: u8,
}

impl GridAccessibility {
    pub const ACCESSIBLE: Self = Self { raw: 0 };

    #[must_use]
    pub const fn new(raw: u8) -> Self {
        Self { raw }
    }
}

impl Default for GridAccessibility {
    fn default() -> Self {
        Self::ACCESSIBLE
    }
}

impl Marshaler for GridAccessibility {
    fn marshal(&self, wb: &mut WriteBuffer) {
        self.raw.marshal(wb);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        Ok(Self::new(u8::unmarshal(rb)?))
    }
}

#[derive(Debug, Clone, PartialEq, nw_network_derive::Marshaler)]
pub struct AlcForbiddenBounds {
    pub bounds: Aabb2d,
    pub accessibility: GridAccessibility,
    pub for_exit: bool,
}

impl Default for AlcForbiddenBounds {
    fn default() -> Self {
        Self {
            bounds: Aabb2d::new(Vec2::ZERO, Vec2::ZERO),
            accessibility: GridAccessibility::ACCESSIBLE,
            for_exit: false,
        }
    }
}

impl fmt::Display for AlcForbiddenBounds {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{{{:?},{:?},{}}}",
            self.bounds, self.bounds, self.for_exit
        )
    }
}

#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::AzRtti,
    nw_network_derive::Fragment,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("01B0664B-3AB6-44A6-87E3-8C69D40E0365")]
#[type_registry(11)]
pub struct ALCReplicatedState {
    pub state_id: DeltaCompressedCounterHandler,
    pub time_anchor_microseconds: ReplicatedFieldHandler<u64>,
    pub time_offset_milliseconds: DeltaCompressedCounterHandler,
    pub flags: ReplicatedFieldHandler<u8>,
    pub more_flags: ReplicatedFieldHandler<u8>,
    pub world_pos: AlcWorldPositionHandler,
    pub look_dir: ReplicatedFieldHandler<Vec3, PackedNormalizedVec3Marshaller>,
    pub rotation: ReplicatedFieldHandler<Quat, PackedQuaternionMarshaller>,
    pub ai_angle_to_desired_facing: ReplicatedFieldHandler<f32>,
    pub weapon_accuracy_stance: ReplicatedFieldHandler<f32, HalfF32Marshaler>,
    pub weapon_accuracy_movement: ReplicatedFieldHandler<f32, HalfF32Marshaler>,
    pub slayer_script_layers: ReplicatedFieldHandler<u8>,
    pub slayer_script_flags: ReplicatedFieldHandler<VlqU32>,
    pub slayer_state_id: [ReplicatedFieldHandler<VlqU32>; 4],
    pub slayer_state_id_started: [ReplicatedFieldHandler<u16>; 4],
    pub slayer_sequence_id: [ReplicatedFieldHandler<VlqU32>; 4],
    pub slayer_sequence_time: [SlayerAnimationFieldHandler; 4],
    pub global_frag_tags: ReplicatedFieldHandler<TagStateData>,
    pub teleport_and_migration_id: ReplicatedFieldHandler<u8>,
    pub hit_character_counter: ReplicatedFieldHandler<u8>,
    pub hit_structure_counter: ReplicatedFieldHandler<u8>,
    pub hit_world_counter: ReplicatedFieldHandler<u8>,
    pub grit_broken_counter: ReplicatedFieldHandler<u8>,
    pub scope_data: ReplicatedFieldHandler<VlqU32>,
    pub segmented_stamina: ReplicatedFieldHandler<u8>,
    pub scope_info_blob: ReplicatedFieldHandler<ScopeInfoBlob>,
    pub scope_time_blobs: [FloatTimerDeltaReplicatedField; 2],
    pub scope_time_blob_extra: ReplicatedFieldHandler<ScopeTimeBlob>,
    pub scopeless_info_blob: ReplicatedFieldHandler<ScopelessInfoBlob>,
    pub scopeless_time_blob: ReplicatedFieldHandler<ScopelessTimeBlob>,
    pub combined_extra_network_data: ReplicatedFieldHandler<u8>,
    pub melee_attack_shape_cast_filter: ReplicatedFieldHandler<VlqU32>,
    pub forbidden_bounds: ReplicatedFieldHandler<Option<AlcForbiddenBounds>>,
    pub current_grid_accessibility: ReplicatedFieldHandler<GridAccessibility>,
    pub distance_to_ground: ReplicatedFieldHandler<u8>,
    pub water_depth: ReplicatedFieldHandler<u8>,
    base: AlcFixedState,
}

impl ALCReplicatedState {
    const DEFAULT_REPLICATION_GROUP_IDX: usize = 0;
    const DEFAULT_REPLICATION_FIELD_COUNT: usize = 54;
    pub const OWNING_PLAYER_GROUP_IDX: usize = 1;
    const OWNING_PLAYER_FIELD_COUNT: usize = 9;

    #[must_use]
    pub fn state_id(&self) -> Option<u16> {
        Self::delta_counter_value(&self.state_id)
    }

    pub fn set_state_id(&mut self, value: u16) {
        self.state_id.set_value(value);
    }

    #[must_use]
    pub fn time_offset_milliseconds(&self) -> Option<u16> {
        Self::delta_counter_value(&self.time_offset_milliseconds)
    }

    pub fn set_time_offset_milliseconds(&mut self, value: u16) {
        self.time_offset_milliseconds.set_value(value);
    }

    #[must_use]
    pub fn world_pos(&self) -> Option<Vec3> {
        self.world_pos.value()
    }

    pub fn set_world_pos(&mut self, value: AlcPositionAnchor, quantization: f32) {
        self.world_pos.set_value(value, quantization);
    }

    pub fn set_owning_player(&mut self, player_actor_hash: ClientActorHash) {
        let group = GroupIndex::new(Self::OWNING_PLAYER_GROUP_IDX);
        self.clear_replication_whitelist(group);
        self.add_client_to_replication_whitelist(player_actor_hash, group);
    }

    fn delta_counter_value(handler: &DeltaCompressedCounterHandler) -> Option<u16> {
        if handler.absolute_portion.value().is_none() && handler.relative_portion.value().is_none()
        {
            return None;
        }
        Some(handler.value())
    }

    fn visit_default_replication_fields<'a>(
        &'a self,
        mut visit: impl FnMut(usize, &'a dyn ReplicatedFieldHandlerBase),
    ) {
        visit(0, &self.state_id.relative_portion);
        visit(1, &self.time_offset_milliseconds.relative_portion);
        visit(2, &self.slayer_sequence_time[0].relative_portion);
        visit(3, &self.slayer_sequence_time[0].absolute_portion);
        visit(4, &self.slayer_sequence_time[1].relative_portion);
        visit(5, &self.slayer_sequence_time[1].absolute_portion);
        visit(6, &self.slayer_sequence_time[2].relative_portion);
        visit(7, &self.slayer_sequence_time[2].absolute_portion);
        visit(8, &self.slayer_sequence_time[3].relative_portion);
        visit(9, &self.slayer_sequence_time[3].absolute_portion);
        visit(10, &self.world_pos.quantized_relative_portion);
        visit(11, &self.rotation);
        visit(12, &self.look_dir);
        visit(13, &self.slayer_state_id[0]);
        visit(14, &self.slayer_state_id_started[0]);
        visit(15, &self.slayer_sequence_id[0]);
        visit(16, &self.slayer_state_id[1]);
        visit(17, &self.slayer_state_id_started[1]);
        visit(18, &self.slayer_sequence_id[1]);
        visit(19, &self.slayer_state_id[2]);
        visit(20, &self.slayer_state_id_started[2]);
        visit(21, &self.slayer_sequence_id[2]);
        visit(22, &self.slayer_state_id[3]);
        visit(23, &self.slayer_state_id_started[3]);
        visit(24, &self.slayer_sequence_id[3]);
        visit(25, &self.state_id.absolute_portion);
        visit(26, &self.time_offset_milliseconds.absolute_portion);
        visit(27, &self.world_pos.absolute_portion);
        visit(28, &self.scope_time_blobs[0].data[0]);
        visit(29, &self.scope_time_blob_extra);
        visit(30, &self.scope_time_blobs[1].data[0]);
        visit(31, &self.teleport_and_migration_id);
        visit(32, &self.scope_data);
        visit(33, &self.scope_info_blob);
        visit(34, &self.global_frag_tags);
        visit(35, &self.ai_angle_to_desired_facing);
        visit(36, &self.weapon_accuracy_stance);
        visit(37, &self.weapon_accuracy_movement);
        visit(38, &self.scope_time_blobs[0].data[1]);
        visit(39, &self.scope_time_blobs[1].data[1]);
        visit(40, &self.time_anchor_microseconds);
        visit(41, &self.flags);
        visit(42, &self.more_flags);
        visit(43, &self.combined_extra_network_data);
        visit(44, &self.segmented_stamina);
        visit(45, &self.grit_broken_counter);
        visit(46, &self.melee_attack_shape_cast_filter);
        visit(47, &self.slayer_script_layers);
        visit(48, &self.slayer_script_flags);
        visit(49, &self.scope_time_blobs[0].data[2]);
        visit(50, &self.scope_time_blobs[0].data[3]);
        visit(51, &self.scope_time_blobs[1].data[2]);
        visit(52, &self.scope_time_blobs[1].data[3]);
        visit(53, &self.world_pos.quantization);
    }

    fn try_visit_default_replication_fields_mut(
        &mut self,
        mut visit: impl FnMut(usize, &mut dyn ReplicatedFieldHandlerBase) -> Result<(), MarshalerError>,
    ) -> Result<(), MarshalerError> {
        visit(0, &mut self.state_id.relative_portion)?;
        visit(1, &mut self.time_offset_milliseconds.relative_portion)?;
        visit(2, &mut self.slayer_sequence_time[0].relative_portion)?;
        visit(3, &mut self.slayer_sequence_time[0].absolute_portion)?;
        visit(4, &mut self.slayer_sequence_time[1].relative_portion)?;
        visit(5, &mut self.slayer_sequence_time[1].absolute_portion)?;
        visit(6, &mut self.slayer_sequence_time[2].relative_portion)?;
        visit(7, &mut self.slayer_sequence_time[2].absolute_portion)?;
        visit(8, &mut self.slayer_sequence_time[3].relative_portion)?;
        visit(9, &mut self.slayer_sequence_time[3].absolute_portion)?;
        visit(10, &mut self.world_pos.quantized_relative_portion)?;
        visit(11, &mut self.rotation)?;
        visit(12, &mut self.look_dir)?;
        visit(13, &mut self.slayer_state_id[0])?;
        visit(14, &mut self.slayer_state_id_started[0])?;
        visit(15, &mut self.slayer_sequence_id[0])?;
        visit(16, &mut self.slayer_state_id[1])?;
        visit(17, &mut self.slayer_state_id_started[1])?;
        visit(18, &mut self.slayer_sequence_id[1])?;
        visit(19, &mut self.slayer_state_id[2])?;
        visit(20, &mut self.slayer_state_id_started[2])?;
        visit(21, &mut self.slayer_sequence_id[2])?;
        visit(22, &mut self.slayer_state_id[3])?;
        visit(23, &mut self.slayer_state_id_started[3])?;
        visit(24, &mut self.slayer_sequence_id[3])?;
        visit(25, &mut self.state_id.absolute_portion)?;
        visit(26, &mut self.time_offset_milliseconds.absolute_portion)?;
        visit(27, &mut self.world_pos.absolute_portion)?;
        visit(28, &mut self.scope_time_blobs[0].data[0])?;
        visit(29, &mut self.scope_time_blob_extra)?;
        visit(30, &mut self.scope_time_blobs[1].data[0])?;
        visit(31, &mut self.teleport_and_migration_id)?;
        visit(32, &mut self.scope_data)?;
        visit(33, &mut self.scope_info_blob)?;
        visit(34, &mut self.global_frag_tags)?;
        visit(35, &mut self.ai_angle_to_desired_facing)?;
        visit(36, &mut self.weapon_accuracy_stance)?;
        visit(37, &mut self.weapon_accuracy_movement)?;
        visit(38, &mut self.scope_time_blobs[0].data[1])?;
        visit(39, &mut self.scope_time_blobs[1].data[1])?;
        visit(40, &mut self.time_anchor_microseconds)?;
        visit(41, &mut self.flags)?;
        visit(42, &mut self.more_flags)?;
        visit(43, &mut self.combined_extra_network_data)?;
        visit(44, &mut self.segmented_stamina)?;
        visit(45, &mut self.grit_broken_counter)?;
        visit(46, &mut self.melee_attack_shape_cast_filter)?;
        visit(47, &mut self.slayer_script_layers)?;
        visit(48, &mut self.slayer_script_flags)?;
        visit(49, &mut self.scope_time_blobs[0].data[2])?;
        visit(50, &mut self.scope_time_blobs[0].data[3])?;
        visit(51, &mut self.scope_time_blobs[1].data[2])?;
        visit(52, &mut self.scope_time_blobs[1].data[3])?;
        visit(53, &mut self.world_pos.quantization)?;
        Ok(())
    }

    fn visit_owning_player_fields<'a>(
        &'a self,
        mut visit: impl FnMut(usize, &'a dyn ReplicatedFieldHandlerBase),
    ) {
        visit(0, &self.distance_to_ground);
        visit(1, &self.water_depth);
        visit(2, &self.scopeless_info_blob);
        visit(3, &self.scopeless_time_blob);
        visit(4, &self.hit_character_counter);
        visit(5, &self.hit_structure_counter);
        visit(6, &self.hit_world_counter);
        visit(7, &self.forbidden_bounds);
        visit(8, &self.current_grid_accessibility);
    }

    fn try_visit_owning_player_fields_mut(
        &mut self,
        mut visit: impl FnMut(usize, &mut dyn ReplicatedFieldHandlerBase) -> Result<(), MarshalerError>,
    ) -> Result<(), MarshalerError> {
        visit(0, &mut self.distance_to_ground)?;
        visit(1, &mut self.water_depth)?;
        visit(2, &mut self.scopeless_info_blob)?;
        visit(3, &mut self.scopeless_time_blob)?;
        visit(4, &mut self.hit_character_counter)?;
        visit(5, &mut self.hit_structure_counter)?;
        visit(6, &mut self.hit_world_counter)?;
        visit(7, &mut self.forbidden_bounds)?;
        visit(8, &mut self.current_grid_accessibility)?;
        Ok(())
    }

    fn collect_fixed_fields(
        &self,
        group_idx: usize,
    ) -> ArrayVec<&dyn ReplicatedFieldHandlerBase, 64> {
        let mut fields = ArrayVec::new();
        self.visit_fixed_fields(group_idx, |index, field| {
            debug_assert_eq!(index, fields.len());
            fields.push(field);
        });
        fields
    }

    fn marshal_metadata_sequences(&self, wb: &mut WriteBuffer) -> bool {
        for group_idx in 0..ALC_REPLICATION_GROUPS {
            self.visit_fixed_fields(group_idx, |_, field| {
                field.last_modified().marshal(wb);
            });
        }
        for group_idx in 0..ALC_REPLICATION_GROUPS {
            self.base
                .client_whitelist_last_modified(GroupIndex::new(group_idx))
                .marshal(wb);
        }
        true
    }

    fn unmarshal_metadata_sequences(
        &mut self,
        rb: &mut ReadBuffer,
    ) -> Result<bool, MarshalerError> {
        for group_idx in 0..ALC_REPLICATION_GROUPS {
            self.try_visit_fixed_fields_mut(group_idx, |_, field| {
                let sequence = SequenceNumber::unmarshal(rb)?;
                field.set_last_modified(sequence);
                Ok(())
            })?;
        }
        for group_idx in 0..ALC_REPLICATION_GROUPS {
            let sequence = SequenceNumber::unmarshal(rb)?;
            self.base
                .set_client_whitelist_last_modified(GroupIndex::new(group_idx), sequence);
        }
        Ok(true)
    }
}

impl DynFragment for ALCReplicatedState {
    fn base(&self) -> &FragmentBase {
        self.base.base()
    }

    fn base_mut(&mut self) -> &mut FragmentBase {
        self.base.base_mut()
    }

    fn marshal_contents(&self, wb: &mut WriteBuffer) -> bool {
        self.marshal_contents_with(&MarshalContext::default(), wb)
    }

    fn marshal_contents_with(&self, mc: &MarshalContext<'_>, wb: &mut WriteBuffer) -> bool {
        self.marshal_fixed_contents(mc, wb)
    }

    fn unmarshal_contents(&mut self, rb: &mut ReadBuffer) -> Result<bool, MarshalerError> {
        self.unmarshal_fixed_contents(rb)
    }

    fn marshal_attributes(&self, mc: &MarshalContext<'_>, wb: &mut WriteBuffer) -> bool {
        self.base
            .marshal_client_whitelist_attributes(mc.baseline_seq, wb)
    }

    fn unmarshal_attributes(&mut self, rb: &mut ReadBuffer) -> Result<bool, MarshalerError> {
        self.base.unmarshal_client_whitelist_attributes(rb)
    }

    fn marshal_field_metadata(&self, _mc: &MarshalContext<'_>, wb: &mut WriteBuffer) -> bool {
        self.marshal_metadata_sequences(wb)
    }

    fn unmarshal_field_metadata(&mut self, rb: &mut ReadBuffer) -> Result<bool, MarshalerError> {
        self.unmarshal_metadata_sequences(rb)
    }
}

impl Fragment for ALCReplicatedState {
    fn merge_and_update_sequence(
        &self,
        new_fragment: &mut dyn Fragment,
        seq: SequenceNumber,
        inherit_previous_network_data_status: bool,
    ) -> Option<Box<dyn Fragment>> {
        self.merge_fixed_and_update_sequence(
            new_fragment,
            seq,
            inherit_previous_network_data_status,
        )
    }

    fn reset_has_new_network_data(&mut self) {
        self.reset_fixed_has_new_network_data();
    }

    fn set_has_new_network_data_on_initial_state(&mut self) {
        self.set_fixed_has_new_network_data_on_initial_state();
    }

    fn is_fully_merged_state(&self) -> bool {
        self.base.is_fully_merged_state()
    }

    fn has_new_network_data(&self) -> bool {
        self.base.has_new_network_data()
    }

    fn detected_new_data_in_last_merge(&self) -> bool {
        self.base.detected_new_data_in_last_merge()
    }

    fn update_sequence(&self) -> SequenceNumber {
        self.base.sequence()
    }

    fn is_fragment_dirty(&self, baseline: SequenceNumber) -> bool {
        baseline < self.base.last_modified()
    }

    fn category(&self) -> FragmentCategory {
        FragmentCategory::PlayerCharacter
    }

    fn has_world_position(&self) -> bool {
        self.world_pos.is_field_valid()
    }

    fn world_position(&self) -> Option<Vec3> {
        self.world_pos()
    }

    fn num_filter_groups(&self) -> usize {
        ALC_REPLICATION_GROUPS
    }

    fn should_send_to_client_group(&self, target: ClientActorHash, group_idx: GroupIndex) -> bool {
        self.should_send_to_client(target, group_idx)
    }

    fn create_new_instance(&self) -> Option<Box<dyn Fragment>> {
        Some(Box::new(Self::default()))
    }
}

impl
    FixedReplicatedStateFields<
        ALC_REPLICATION_GROUPS,
        ALC_FIELDS_PER_GROUP,
        ALC_CLIENT_WHITELIST_SIZE,
    > for ALCReplicatedState
{
    fn fixed_replicated_state(&self) -> &AlcFixedState {
        &self.base
    }

    fn fixed_replicated_state_mut(&mut self) -> &mut AlcFixedState {
        &mut self.base
    }

    fn fixed_group_field_count(&self, group_idx: usize) -> Option<usize> {
        match group_idx {
            Self::DEFAULT_REPLICATION_GROUP_IDX => Some(Self::DEFAULT_REPLICATION_FIELD_COUNT),
            Self::OWNING_PLAYER_GROUP_IDX => Some(Self::OWNING_PLAYER_FIELD_COUNT),
            _ => None,
        }
    }

    fn visit_fixed_fields<'a>(
        &'a self,
        group_idx: usize,
        visit: impl FnMut(usize, &'a dyn ReplicatedFieldHandlerBase),
    ) {
        match group_idx {
            Self::DEFAULT_REPLICATION_GROUP_IDX => self.visit_default_replication_fields(visit),
            Self::OWNING_PLAYER_GROUP_IDX => self.visit_owning_player_fields(visit),
            _ => {}
        }
    }

    fn try_visit_fixed_fields_mut(
        &mut self,
        group_idx: usize,
        visit: impl FnMut(usize, &mut dyn ReplicatedFieldHandlerBase) -> Result<(), MarshalerError>,
    ) -> Result<(), MarshalerError> {
        match group_idx {
            Self::DEFAULT_REPLICATION_GROUP_IDX => {
                self.try_visit_default_replication_fields_mut(visit)
            }
            Self::OWNING_PLAYER_GROUP_IDX => self.try_visit_owning_player_fields_mut(visit),
            _ => Ok(()),
        }
    }

    fn try_visit_fixed_fields_for_merge(
        &mut self,
        old_state: &Self,
        new_state: &mut Self,
        group_idx: usize,
        mut visit: impl FnMut(
            usize,
            &mut dyn ReplicatedFieldHandlerBase,
            &dyn ReplicatedFieldHandlerBase,
            &mut dyn ReplicatedFieldHandlerBase,
        ) -> Result<(), MarshalerError>,
    ) -> Result<(), MarshalerError> {
        let old_fields = old_state.collect_fixed_fields(group_idx);
        new_state.try_visit_fixed_fields_mut(group_idx, |index, new_field| {
            let Some(old_field) = old_fields.get(index).copied() else {
                debug_assert!(false, "old state is missing fixed field {index}");
                return Ok(());
            };
            self.try_visit_fixed_fields_mut(group_idx, |merged_index, merged_field| {
                if merged_index == index {
                    visit(index, merged_field, old_field, new_field)?;
                }
                Ok(())
            })
        })?;
        Ok(())
    }
}

impl Marshaler for ALCReplicatedState {
    fn marshal(&self, wb: &mut WriteBuffer) {
        DynFragment::marshal_contents(self, wb);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let mut value = Self::default();
        DynFragment::unmarshal_contents(&mut value, rb)?;
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn alc_world_position_reports_handler_validity() {
        let mut state = ALCReplicatedState::default();
        assert!(!Fragment::has_world_position(&state));
        assert_eq!(Fragment::world_position(&state), None);

        let position = AlcPositionAnchor::new(10.0, 20.0, 30.0);
        state.set_world_pos(position, 0.25);

        assert!(Fragment::has_world_position(&state));
        assert_eq!(Fragment::world_position(&state), Some(position.as_vec3()));
    }

    #[test]
    fn forbidden_bounds_default_matches_member_defaults() {
        let bounds = AlcForbiddenBounds::default();

        assert_eq!(bounds.accessibility, GridAccessibility::ACCESSIBLE);
        assert!(!bounds.for_exit);
    }

    #[test]
    fn forbidden_bounds_display_uses_registered_debug_shape() {
        let bounds = AlcForbiddenBounds::default();

        assert_eq!(
            bounds.to_string(),
            format!("{{{:?},{:?},false}}", bounds.bounds, bounds.bounds)
        );
    }

    #[test]
    fn alc_position_anchor_uses_packed_height_range() {
        let mut min = WriteBuffer::default();
        AlcPositionAnchorMarshaler::marshal(&AlcPositionAnchor::new(1.0, 2.0, -100.0), &mut min);
        assert_eq!(
            min.as_slice().len(),
            AlcPositionAnchorMarshaler::MARSHAL_SIZE
        );
        assert_eq!(&min.as_slice()[8..], &[0x00, 0x00]);

        let mut max = WriteBuffer::default();
        AlcPositionAnchorMarshaler::marshal(&AlcPositionAnchor::new(1.0, 2.0, 2000.0), &mut max);
        assert_eq!(&max.as_slice()[8..], &[0xff, 0xff]);
    }
}
