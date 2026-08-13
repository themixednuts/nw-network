//! Transform replication for an entity's position in the world.

use glam::Vec3;

/// Position, rotation, and scale state for an entity in the world.
pub use crate::generated::states::PositionInTheWorldReplicatedState;
#[must_use]
pub const fn position_anchor_to_bevy_translation(position: Vec3) -> Vec3 {
    Vec3::new(position.x, position.z, position.y)
}
