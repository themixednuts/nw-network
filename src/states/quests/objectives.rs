//! Objective task-state replication.

use crate::{az_rtti, replicated_state, type_registry};

use arrayvec::ArrayVec;
use uuid::Uuid;

use crate::Marshaler;
use crate::serialize::{IndexMap, ReplicatedContainer, ReplicatedFieldHandler};

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

#[replicated_state]
#[derive(Debug, Clone, Default)]
#[az_rtti("036365F4-A485-439D-8A3F-C51DFF6123B4")]
#[type_registry(3857)]
pub struct ObjectivesComponentReplicatedState {
    #[replicated_state(group = 1)]
    pub task_start_times: ReplicatedContainer<IndexMap<ObjectiveTaskKey, u64>>,
    #[replicated_state(group = 1)]
    pub tracked_objectives: ReplicatedContainer<Vec<u64>>,
    #[replicated_state(group = 1)]
    pub completed_objectives: ReplicatedContainer<Vec<u64>>,
    #[replicated_state(group = 1)]
    pub active_objectives: ReplicatedContainer<Vec<ObjectiveReplicationData>>,
    #[replicated_state(group = 1)]
    pub task_states: ReplicatedContainer<Vec<ObjectiveTaskState>>,
    #[replicated_state(group = 1)]
    pub objective_poi_entity_ids: ReplicatedContainer<Vec<u64>>,
    pub grace_period_end_time: ReplicatedFieldHandler<u64>,
    #[replicated_state(group = 1)]
    pub dynamic_poi_indices: ReplicatedContainer<Vec<u16>>,
}

impl ObjectivesComponentReplicatedState {
    pub fn apply_snapshot(&mut self, snapshot: ObjectivesSnapshot) {
        if let Some(sequence) = snapshot.task_start_times_sequence {
            self.task_start_times = ReplicatedContainer::new(sequence, IndexMap::new());
        }
        if let Some(values) = snapshot.tracked_objectives {
            self.tracked_objectives = values;
        }
        if let Some(values) = snapshot.completed_objectives {
            self.completed_objectives = values;
        }
        if let Some(values) = snapshot.active_objectives {
            self.active_objectives = values;
        }
        if let Some(values) = snapshot.task_states {
            self.task_states = values;
        }
        if let Some(values) = snapshot.objective_poi_entity_ids {
            self.objective_poi_entity_ids = values;
        }
        if let Some(value) = snapshot.grace_period_end_time {
            self.grace_period_end_time.set_value(value);
        }
        if let Some(values) = snapshot.dynamic_poi_indices {
            self.dynamic_poi_indices = values;
        }
    }
}
