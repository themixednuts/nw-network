//! Projectile transform, lifetime, and hit-data replication.

use crate::Marshaler;

/// Value stored by [`ProjectileReplicatedState::piercing_hits`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct PiercingHitData {
    pub target_entity: u64,
    pub flag: u8,
    pub volume_index: u16,
}

/// Projectile ranged-attack replicated state.
pub use crate::generated::states::ProjectileReplicatedState;
