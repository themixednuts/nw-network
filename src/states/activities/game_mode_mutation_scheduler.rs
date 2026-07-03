//! Game-mode mutation scheduling state replicated to clients.

use crate::{az_rtti, replicated_state, type_registry};

use crate::Marshaler;
use crate::serialize::{IndexMap, ReplicatedContainer, ReplicatedFieldHandler};

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct GameModeMutationSet {
    pub curse_mutation_id: u32,
    pub promotion_mutation_id: u32,
    pub elemental_mutation_id: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GameModeMutationSnapshot {
    pub sets_sequence: u64,
    pub sets: IndexMap<u32, GameModeMutationSet>,
    pub cadence_start_time: u64,
}

#[replicated_state]
#[derive(Debug, Clone, Default)]
#[az_rtti("D0D4C4D1-5489-4AE8-941D-4DB70E2A8AB9")]
#[type_registry(2201)]
pub struct GameModeMutationSchedulerReplicatedState {
    pub game_mode_mutation_sets: ReplicatedContainer<IndexMap<u32, GameModeMutationSet>, 0x4000>,
    pub mutation_cadence_start_time: ReplicatedFieldHandler<u64>,
}

impl GameModeMutationSchedulerReplicatedState {
    pub fn apply_snapshot(&mut self, snapshot: GameModeMutationSnapshot) {
        self.game_mode_mutation_sets =
            ReplicatedContainer::new(snapshot.sets_sequence, snapshot.sets);
        self.mutation_cadence_start_time
            .set_value(snapshot.cadence_start_time);
    }
}
