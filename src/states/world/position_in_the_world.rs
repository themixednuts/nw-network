//! Transform replication for an entity's position in the world.

use crate::{az_rtti, replicated_state, type_registry};

use glam::{Quat, Vec3};

use crate::serialize::{
    NonUniformScaleCompMarshaler, PackedPositionMarshaller, QuatCompNorm, ReplicatedFieldHandler,
};

/// Position, rotation, and scale state for an entity in the world.
#[replicated_state]
#[derive(Debug, Clone, Default)]
#[az_rtti("79C28008-4FC5-4EFB-88A1-538F4FB7DDE1")]
#[type_registry(13)]
pub struct PositionInTheWorldReplicatedState {
    pub position: ReplicatedFieldHandler<Vec3, PackedPositionMarshaller<0xc2c8_0000, 0x44fa_0000>>,
    pub rotation: ReplicatedFieldHandler<QuatCompNorm>,
    pub scale: ReplicatedFieldHandler<Vec3, NonUniformScaleCompMarshaler>,
}

impl PositionInTheWorldReplicatedState {
    #[must_use]
    pub fn with_anchor(position: Vec3) -> Self {
        let mut state = Self::default();
        state.position.set_value(position);
        state
    }

    #[must_use]
    pub fn translation(&self) -> Option<Vec3> {
        self.position
            .value()
            .copied()
            .map(position_anchor_to_bevy_translation)
    }

    #[must_use]
    pub fn rotation(&self) -> Option<Quat> {
        self.rotation.value().copied().map(Into::into)
    }

    #[must_use]
    pub fn scale(&self) -> Option<Vec3> {
        self.scale.value().copied()
    }
}

#[must_use]
pub const fn position_anchor_to_bevy_translation(position: Vec3) -> Vec3 {
    Vec3::new(position.x, position.z, position.y)
}
