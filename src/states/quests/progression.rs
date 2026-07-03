//! Quest progression and categorical progression replication.

use crate::{az_rtti, replicated_state, type_registry};

use crate::Marshaler;
use crate::serialize::{Change, ReplicatedContainer, VlqU64};

pub use crate::generated::states::ProgressionComponentReplicatedState;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CategoricalProgressionSnapshot {
    pub progression_ids: ReplicatedContainer<Vec<u32>>,
    pub ranks: ReplicatedContainer<Vec<u16>>,
    pub points: ReplicatedContainer<Vec<u64>>,
}

#[replicated_state]
#[derive(Debug, Clone, Default)]
#[az_rtti("9D621862-D7F9-44B0-9A64-E3ED8A154AE1")]
#[type_registry(911)]
pub struct CategoricalProgressionReplicatedState {
    pub progression_ids: ReplicatedContainer<Vec<u32>>,
    pub ranks: ReplicatedContainer<Vec<u16>>,
    pub points: ReplicatedContainer<Vec<u64>>,
}

impl CategoricalProgressionReplicatedState {
    pub fn apply_snapshot(&mut self, snapshot: CategoricalProgressionSnapshot) {
        self.progression_ids = snapshot.progression_ids;
        self.ranks = snapshot.ranks;
        self.points = snapshot.points;
    }

    #[must_use]
    pub fn indexed_delta(&self, index: usize) -> Self {
        let mut state = Self::default();
        if self.progression_ids.has_value()
            && let Some(values) = Self::project_index(&self.progression_ids, index)
        {
            state.progression_ids = values;
        }
        if self.ranks.has_value()
            && let Some(values) = Self::project_index(&self.ranks, index)
        {
            state.ranks = values;
        }
        if self.points.has_value()
            && let Some(values) = Self::project_index(&self.points, index)
        {
            state.points = values;
        }
        state
    }

    fn project_index<T>(
        source: &ReplicatedContainer<Vec<T>>,
        index: usize,
    ) -> Option<ReplicatedContainer<Vec<T>>>
    where
        T: Clone + Marshaler,
    {
        let value = source.values().get(index)?.clone();
        Some(ReplicatedContainer::delta(vec![Change::update(
            VlqU64::new(index as u64),
            value,
            source.last_modified(),
        )]))
    }
}
