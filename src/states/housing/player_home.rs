//! Player home-point replication.

use glam::Vec3;
use uuid::Uuid;

use crate::Marshaler;
use crate::serialize::ReplicatedContainer;

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct HomePointPersistentRef {
    pub gde_id: Uuid,
    pub home_point_unique_id_value: u64,
    pub gde_id_hash: u64,
}

/// Home-point entry stored by player-home replicated state.
#[derive(Debug, Clone, Default, PartialEq, Marshaler)]
pub struct HomePointReplicatedState {
    pub persistent_ref: HomePointPersistentRef,
    pub name: String,
    pub position: Vec3,
    pub cooldown_duration_ns: u64,
    pub cooldown_end_ns: u64,
    pub respawn_type: u32,
    pub is_overloaded: bool,
    pub is_hidden_from_respawn: u8,
    pub home_point_unique_id: String,
    pub respawn_modifier: u32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct PlayerHomeSnapshot {
    pub home_point_list: ReplicatedContainer<Vec<HomePointReplicatedState>>,
    pub home_point_id: Option<String>,
}
pub use crate::generated::states::PlayerHomeComponentReplicatedState;
