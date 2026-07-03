//! Player arena, dungeon, and trial participation state.
//!
//! This is a single-group state: every field is registered in group 0 in
//! declaration order. It combines queue/presence flags, per-dungeon rankings,
//! entry counters, refresh timers, and single-player instance state so clients
//! can render activity availability without additional state fragments.

use crate::{az_rtti, replicated_state, type_registry};

use uuid::Uuid;

use crate::serialize::{IndexMap, ReplicatedContainer, ReplicatedFieldHandler, VlqU64};

/// Replicated arena and dungeon participation state.
#[replicated_state]
#[derive(Debug, Clone, Default)]
#[az_rtti("7DD20E73-9E5C-4753-8B94-2AEF87A82D37")]
#[type_registry(2406)]
pub struct PlayerArenaReplicatedState {
    /// Whether the player is currently in an arena.
    pub is_in_arena: ReplicatedFieldHandler<bool>,
    /// Whether the player is queued for a dungeon.
    pub is_queued_for_dungeon: ReplicatedFieldHandler<bool>,
    /// Dungeon-entry cooldown end time.
    pub dungeon_cooldown_time: ReplicatedFieldHandler<u64>,
    /// Solo-trial entry cooldown end time.
    pub enter_solo_trial_cooldown_time: ReplicatedFieldHandler<u64>,
    /// Per-dungeon rank values keyed by dungeon id.
    pub dungeon_ranks: ReplicatedContainer<IndexMap<VlqU64, u8>>,
    /// Last base dungeon entered.
    pub last_dungeons_entered: ReplicatedFieldHandler<Uuid>,
    /// Last mutated dungeon entered.
    pub last_mutated_dungeon_entered: ReplicatedFieldHandler<Uuid>,
    /// Base dungeon entries since the last limit refresh.
    pub num_base_dungeons_entered_since_last_refresh: ReplicatedFieldHandler<u32>,
    /// Mutated dungeon entries since the last limit refresh.
    pub num_mutated_dungeons_entered_since_last_refresh: ReplicatedFieldHandler<u32>,
    /// Group trial entries since the last limit refresh.
    pub num_group_trials_entered_since_last_refresh: ReplicatedFieldHandler<u32>,
    /// Next base-dungeon entry-limit refresh time.
    pub next_dungeon_base_max_limit_refresh_time: ReplicatedFieldHandler<u64>,
    /// Next mutated-dungeon entry-limit refresh time.
    pub next_dungeon_mutated_max_limit_refresh_time: ReplicatedFieldHandler<u64>,
    /// Next group-trial entry-limit refresh time.
    pub next_group_trial_max_limit_refresh_time: ReplicatedFieldHandler<u64>,
    /// Whether the mutation unlock award has already been granted.
    pub has_mutation_unlock_award_been_granted: ReplicatedFieldHandler<bool>,
    /// Compact single-player instance state.
    pub single_player_instance_state: ReplicatedFieldHandler<u8>,
    /// Single-player dungeon timer value.
    pub single_player_dungeon_time: ReplicatedFieldHandler<u64>,
    /// Current game-mode index.
    pub game_mode_idx: ReplicatedFieldHandler<u8>,
}
