use uuid::Uuid;

use crate::hub::ReplicatedState;
use crate::serialize::{ReplicatedFieldHandler, ReplicatedMap, VlqU64};

#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ReplicatedState,
    nw_network_derive::AzRtti,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("7DD20E73-9E5C-4753-8B94-2AEF87A82D37")]
#[type_registry(2406)]
pub struct PlayerArenaReplicatedState {
    pub is_in_arena: ReplicatedFieldHandler<bool>,
    pub is_queued_for_dungeon: ReplicatedFieldHandler<bool>,
    pub dungeon_cooldown_time: ReplicatedFieldHandler<u64>,
    pub enter_solo_trial_cooldown_time: ReplicatedFieldHandler<u64>,
    pub dungeon_ranks: ReplicatedMap<VlqU64, u8>,
    pub last_dungeons_entered: ReplicatedFieldHandler<Uuid>,
    pub last_mutated_dungeon_entered: ReplicatedFieldHandler<Uuid>,
    pub num_base_dungeons_entered_since_last_refresh: ReplicatedFieldHandler<u32>,
    pub num_mutated_dungeons_entered_since_last_refresh: ReplicatedFieldHandler<u32>,
    pub num_group_trials_entered_since_last_refresh: ReplicatedFieldHandler<u32>,
    pub next_dungeon_base_max_limit_refresh_time: ReplicatedFieldHandler<u64>,
    pub next_dungeon_mutated_max_limit_refresh_time: ReplicatedFieldHandler<u64>,
    pub next_group_trial_max_limit_refresh_time: ReplicatedFieldHandler<u64>,
    pub has_mutation_unlock_award_been_granted: ReplicatedFieldHandler<bool>,
    pub single_player_instance_state: ReplicatedFieldHandler<u8>,
    pub single_player_dungeon_time: ReplicatedFieldHandler<u64>,
    pub game_mode_idx: ReplicatedFieldHandler<u8>,

    pub hub: ReplicatedState,
}
