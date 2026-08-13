//! Quest, objective, progression, reward, and seasonal replicated state modules.

pub mod achievement;
pub mod encounter_event_objective;
pub mod game_event;
pub mod objective_interactor;
pub mod objectives;
pub mod points_accumulator;
pub mod progression;
pub mod reward_track;
pub mod seasons_rewards;

pub use crate::generated::states::PlayerTimeComponentReplicatedState;
pub use achievement::AchievementComponentReplicatedState;
pub use encounter_event_objective::{EncounterObjectiveStatus, EncounterObjectiveStatusEntry};
pub use game_event::{
    DailyBonusUsed, GameEventComponentReplicatedState, GameEventData, GameEventSnapshot,
    GameEventSubEntry,
};
pub use objective_interactor::{
    CommunityGoalParams, MissionParam, ObjectiveInteractorComponentReplicatedState,
    ObjectiveInteractorSnapshot, ObjectiveResponseParametersReplicatedState,
};
pub use objectives::{
    ObjectiveReplicationData, ObjectiveTaskKey, ObjectiveTaskState,
    ObjectivesComponentReplicatedState, ObjectivesSnapshot,
};
pub use points_accumulator::PointsAccumulatorComponentReplicatedState;
pub use progression::{
    CategoricalProgressionReplicatedState, CategoricalProgressionSnapshot,
    ProgressionComponentReplicatedState,
};
pub use reward_track::{RewardTrackComponentReplicatedState, RewardTrackSnapshot, RolledReward};
pub use seasons_rewards::{
    SeasonsRewardsReplicatedState, SeasonsRewardsSnapshot, SeasonsRewardsStatsUpdateSnapshot,
    SeasonsRewardsTaskIds, SeasonsRewardsTrackedStatReplicatedState,
};
