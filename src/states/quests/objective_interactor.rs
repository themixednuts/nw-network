use arrayvec::ArrayVec;
use uuid::Uuid;

use crate::Marshaler;
use crate::serialize::{ReplicatedFieldHandler, ReplicatedVec};

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "wire shape keeps independent bool fields in order"
)]
pub struct ObjectiveResponseParametersReplicatedState {
    pub objective_uuid: Uuid,
    pub response_time: u64,
    pub response_id: u16,
    pub is_selected: bool,
    pub is_complete: bool,
    pub is_repeatable: bool,
    pub has_target: bool,
    pub target_id: u64,
    pub has_response_values: bool,
    pub response_values: ArrayVec<u32, 7>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct MissionParam {
    pub mission_id: u64,
    pub response_crc: u32,
    pub response: ObjectiveResponseParametersReplicatedState,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct CommunityGoalParams {
    pub goal_id: u16,
    pub goal_crc: u32,
    pub active_objective_ids: Vec<u32>,
    pub completed_objective_ids: Vec<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObjectiveInteractorSnapshot {
    pub objective_provider_id: Uuid,
    pub expiration_time: u64,
    pub mission_params_sequence: u64,
    pub mission_params: Vec<MissionParam>,
    pub community_goals: Vec<CommunityGoalParams>,
}

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("2205CFE8-2403-42C7-81F7-5DEDB58E9ECE")]
#[::nw_network::type_registry(3829)]
pub struct ObjectiveInteractorComponentReplicatedState {
    pub objective_provider_id: ReplicatedFieldHandler<Uuid>,
    pub expiration_time: ReplicatedFieldHandler<u64>,
    pub mission_params: ReplicatedVec<MissionParam>,
    pub community_goals: ReplicatedFieldHandler<Vec<CommunityGoalParams>>,
}

impl ObjectiveInteractorComponentReplicatedState {
    pub fn apply_snapshot(&mut self, snapshot: ObjectiveInteractorSnapshot) {
        self.objective_provider_id
            .set_value(snapshot.objective_provider_id);
        self.expiration_time.set_value(snapshot.expiration_time);
        self.mission_params =
            ReplicatedVec::new(snapshot.mission_params_sequence, snapshot.mission_params);
        self.community_goals.set_value(snapshot.community_goals);
    }
}
