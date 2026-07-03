//! Ability tray, cooldown, and persistent ability replication.

use crate::{az_rtti, replicated_state, type_registry};

use arrayvec::ArrayVec;

use crate::Marshaler;
use crate::serialize::{ReplicatedContainer, ReplicatedFieldHandler};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Marshaler)]
pub struct AbilityU32Pair {
    pub key: u32,
    pub value: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct PersistentAbilityEntry {
    pub ability_id: u32,
    pub values: ArrayVec<AbilityU32Pair, 3>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct PersistentAbilityData {
    pub abilities: Vec<PersistentAbilityEntry>,
    pub additional_values: Vec<AbilityU32Pair>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AbilitySnapshot {
    pub persistent_ability_data: PersistentAbilityData,
    pub action_data_count: Option<ReplicatedContainer<Vec<u8>>>,
    pub action_data_ability_ids: Option<ReplicatedContainer<Vec<u32>>>,
}

#[replicated_state]
#[derive(Debug, Clone, Default)]
#[az_rtti("35877C3E-4EF8-43DD-ABE7-ABF35104F1B5")]
#[type_registry(185)]
pub struct AbilityComponentReplicatedState {
    pub persistent_ability_data: ReplicatedFieldHandler<PersistentAbilityData>,
    pub hit_data_num_hits: ReplicatedContainer<Vec<u8>>,
    pub hit_data_ability_ids: ReplicatedContainer<Vec<u32>>,
    pub action_data_count: ReplicatedContainer<Vec<u8>>,
    pub action_data_ability_ids: ReplicatedContainer<Vec<u32>>,
}

impl AbilityComponentReplicatedState {
    pub fn apply_snapshot(&mut self, snapshot: AbilitySnapshot) {
        self.persistent_ability_data
            .set_value(snapshot.persistent_ability_data);
        if let Some(values) = snapshot.action_data_count {
            self.action_data_count = values;
        }
        if let Some(values) = snapshot.action_data_ability_ids {
            self.action_data_ability_ids = values;
        }
    }
}
