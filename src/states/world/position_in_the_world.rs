use glam::{Quat, Vec3};

use crate::serialize::{
    PositionAnchorMarshaler, QuatCompNorm, ReplicatedFieldHandler, Vec3CompMarshaler,
};

/// Position, rotation, and scale state for an entity in the world.
#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("79C28008-4FC5-4EFB-88A1-538F4FB7DDE1")]
#[::nw_network::type_registry(13)]
pub struct PositionInTheWorldReplicatedState {
    pub position: ReplicatedFieldHandler<(f32, f32, f32), PositionAnchorMarshaler>,
    pub rotation: ReplicatedFieldHandler<QuatCompNorm>,
    pub scale: ReplicatedFieldHandler<Vec3, Vec3CompMarshaler>,
}

impl PositionInTheWorldReplicatedState {
    #[must_use]
    pub fn with_anchor(position: (f32, f32, f32)) -> Self {
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
        self.rotation.value().copied().map(|rotation| rotation.0)
    }

    #[must_use]
    pub fn scale(&self) -> Option<Vec3> {
        self.scale.value().copied()
    }
}

#[must_use]
pub const fn position_anchor_to_bevy_translation((x, y, height): (f32, f32, f32)) -> Vec3 {
    Vec3::new(x, height, y)
}
