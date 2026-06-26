use std::collections::HashMap;

use arrayvec::ArrayVec;
use uuid::Uuid;

use crate::Marshaler;
use crate::serialize::{ReplicatedMap, ReplicatedVec};

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
    pub tracked_objectives: Option<ReplicatedVec<u64>>,
    pub completed_objectives: Option<ReplicatedVec<u64>>,
    pub active_objectives: Option<ReplicatedVec<ObjectiveReplicationData>>,
    pub task_states: Option<ReplicatedVec<ObjectiveTaskState>>,
    pub objective_poi_entity_ids: Option<ReplicatedVec<u64>>,
    pub dynamic_poi_indices: Option<ReplicatedVec<u16>>,
}

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("036365F4-A485-439D-8A3F-C51DFF6123B4")]
#[::nw_network::type_registry(3857)]
pub struct ObjectivesComponentReplicatedState {
    #[replicated_state(group = 1)]
    pub task_start_times: ReplicatedMap<ObjectiveTaskKey, u64>,
    #[replicated_state(group = 1)]
    pub tracked_objectives: ReplicatedVec<u64>,
    #[replicated_state(group = 1)]
    pub completed_objectives: ReplicatedVec<u64>,
    #[replicated_state(group = 1)]
    pub active_objectives: ReplicatedVec<ObjectiveReplicationData>,
    #[replicated_state(group = 1)]
    pub task_states: ReplicatedVec<ObjectiveTaskState>,
    #[replicated_state(group = 1)]
    pub objective_poi_entity_ids: ReplicatedVec<u64>,
    #[replicated_state(group = 1)]
    pub dynamic_poi_indices: ReplicatedVec<u16>,
}

impl ObjectivesComponentReplicatedState {
    pub fn apply_snapshot(&mut self, snapshot: ObjectivesSnapshot) {
        if let Some(sequence) = snapshot.task_start_times_sequence {
            self.task_start_times = ReplicatedMap::new(sequence, HashMap::new());
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
        if let Some(values) = snapshot.dynamic_poi_indices {
            self.dynamic_poi_indices = values;
        }
    }
}
