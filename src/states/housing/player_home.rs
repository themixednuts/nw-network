use glam::Vec3;
use uuid::Uuid;

use crate::Marshaler;
use crate::serialize::{ReplicatedFieldHandler, ReplicatedVec};

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
    pub home_point_list: ReplicatedVec<HomePointReplicatedState>,
    pub home_point_id: Option<String>,
}

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("305FCFBB-3FD0-49BB-841B-14EF372C6469")]
#[::nw_network::type_registry(3652)]
pub struct PlayerHomeComponentReplicatedState {
    pub home_point_list: ReplicatedFieldHandler<ReplicatedVec<HomePointReplicatedState>>,
    pub home_point_id: ReplicatedFieldHandler<String>,
}

impl PlayerHomeComponentReplicatedState {
    #[must_use]
    pub fn empty_home_points(sequence: u64) -> Self {
        let mut state = Self::default();
        state
            .home_point_list
            .set_value(ReplicatedVec::new(sequence, Vec::new()));
        state
    }

    pub fn apply_snapshot(&mut self, snapshot: PlayerHomeSnapshot) {
        self.home_point_list.set_value(snapshot.home_point_list);
        if let Some(home_point_id) = snapshot.home_point_id {
            self.home_point_id.set_value(home_point_id);
        }
    }
}
