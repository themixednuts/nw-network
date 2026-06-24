use uuid::Uuid;

use crate::Marshaler;
use crate::hub::ReplicatedState;
use crate::serialize::{ReplicatedFieldHandler, ReplicatedMap, ReplicatedVec};

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Marshaler)]
pub struct GameModeInstanceId {
    pub game_mode_id: Uuid,
    pub field_10: u64,
    pub field_18: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct ActiveGameModeData {
    pub field_00: u32,
    pub field_04: u32,
    pub field_08: u64,
    pub field_10: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct QueuedGameModeData {
    pub field_00: u32,
    pub field_08: u64,
    pub field_10: u32,
    pub field_14: u32,
    pub field_18: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct GameModeMutationContext {
    pub field_00: u32,
    pub field_04: u32,
    pub field_08: u32,
    pub field_0c: u8,
}

#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ReplicatedState,
    nw_network_derive::AzRtti,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("4C6684A9-6988-4A05-94BD-118CE991A7D9")]
#[type_registry(3312)]
pub struct GameModeParticipantReplicatedState {
    pub active_game_modes: ReplicatedMap<GameModeInstanceId, ActiveGameModeData>,
    pub flags: ReplicatedFieldHandler<[u8; 6]>,
    pub queuing_for_game_modes: ReplicatedVec<QueuedGameModeData>,
    pub queue_eligible_times_for_game_modes: ReplicatedMap<u32, u64>,
    pub game_mode_mutation_context: ReplicatedFieldHandler<GameModeMutationContext>,
    #[replicated_state(group = 1)]
    pub matchmaking_service_activity: ReplicatedFieldHandler<u8>,
    #[replicated_state(group = 1)]
    pub matchmaking_service_status: ReplicatedFieldHandler<u32>,
    #[replicated_state(group = 1)]
    pub matchmaking_service_match_id: ReplicatedFieldHandler<String>,
    #[replicated_state(group = 1)]
    pub matchmaking_service_desired_players: ReplicatedFieldHandler<u8>,
    #[replicated_state(group = 1)]
    pub matchmaking_service_accepted_players: ReplicatedFieldHandler<u8>,
    #[replicated_state(group = 1)]
    pub is_replicating_group_activity: ReplicatedFieldHandler<bool>,
    pub last_team_index: ReplicatedFieldHandler<u8>,
    #[replicated_state(group = 2)]
    pub group_activity_eligibility: ReplicatedFieldHandler<[u8; 16]>,

    pub hub: ReplicatedState,
}
