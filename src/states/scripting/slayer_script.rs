use std::collections::HashMap;

use glam::Vec3;

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
#[az_rtti("B4DB39E2-5054-4604-9855-9A4DC75BDDE4")]
#[type_registry(3362)]
pub struct SlayerScriptReplicatedState {
    pub cur_script_state_id: ReplicatedFieldHandler<u8>,
    pub cur_script_id: ReplicatedFieldHandler<u32>,
    pub spawned_entity_ids_by_spawner_id: ReplicatedMap<u32, u64>,

    pub hub: ReplicatedState,
}

#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ReplicatedState,
    nw_network_derive::AzRtti,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("B5E124FB-D4D1-4479-9A0B-3623BEF6EF0B")]
#[type_registry(6234)]
pub struct InstancedSlayerScriptReplicatedState {
    pub cur_script_state_id: ReplicatedFieldHandler<u8>,
    pub cur_script_id: ReplicatedFieldHandler<u32>,
    pub spawned_entity_ids_by_spawner_id: ReplicatedMap<u32, u64>,
    pub synced_timers: ReplicatedMap<u32, VlqU64>,

    #[replicated_state(group = 1)]
    pub script_tag_id: ReplicatedFieldHandler<u32>,
    #[replicated_state(group = 1)]
    pub script_location: ReplicatedFieldHandler<Vec3>,
    #[replicated_state(group = 1)]
    pub active_task_id: ReplicatedFieldHandler<u64>,

    pub hub: ReplicatedState,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InstancedSlayerScriptSnapshot {
    pub script_tag_id: u32,
    pub spawned_entity_ids_sequence: u64,
    pub spawned_entity_ids_by_spawner_id: HashMap<u32, u64>,
}

impl InstancedSlayerScriptReplicatedState {
    pub fn apply_snapshot(&mut self, snapshot: InstancedSlayerScriptSnapshot) {
        self.script_tag_id.set_value(snapshot.script_tag_id);
        self.spawned_entity_ids_by_spawner_id = ReplicatedMap::new(
            snapshot.spawned_entity_ids_sequence,
            snapshot.spawned_entity_ids_by_spawner_id,
        );
    }
}
