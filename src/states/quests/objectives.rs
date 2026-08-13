//! Objective task-state replication.

use arrayvec::ArrayVec;
use uuid::Uuid;

use crate::Marshaler;
use crate::serialize::ReplicatedContainer;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Marshaler)]
pub struct ObjectiveTaskKey {
    pub objective_id: u64,
    pub task_id: u8,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "wire shape keeps independent bool fields in order"
)]
pub struct ObjectiveReplicationData {
    pub objective_type: u32,
    pub objective_id: u64,
    pub objective_crc: u32,
    pub objective_uuid: Uuid,
    pub parent_objective_id: u64,
    pub objective_task_id: u16,
    pub available: bool,
    pub visible: bool,
    pub tracked: bool,
    pub complete: bool,
    pub poi_entity_id: u64,
    pub has_poi: bool,
    pub task_indices: ArrayVec<u32, 7>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Marshaler)]
pub struct ObjectiveTaskState {
    pub key: ObjectiveTaskKey,
    pub state: u32,
    pub count: u32,
    pub flags: u8,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObjectivesSnapshot {
    pub task_start_times_sequence: Option<u64>,
    pub tracked_objectives: Option<ReplicatedContainer<Vec<u64>>>,
    pub completed_objectives: Option<ReplicatedContainer<Vec<u64>>>,
    pub active_objectives: Option<ReplicatedContainer<Vec<ObjectiveReplicationData>>>,
    pub task_states: Option<ReplicatedContainer<Vec<ObjectiveTaskState>>>,
    pub objective_poi_entity_ids: Option<ReplicatedContainer<Vec<u64>>>,
    pub grace_period_end_time: Option<u64>,
    pub dynamic_poi_indices: Option<ReplicatedContainer<Vec<u16>>>,
}
pub use crate::generated::states::ObjectivesComponentReplicatedState;
