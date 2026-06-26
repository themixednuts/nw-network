use crate::Marshaler;
use crate::serialize::{Change, ReplicatedFieldHandler, ReplicatedVec, VlqU64};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CategoricalProgressionSnapshot {
    pub progression_ids: ReplicatedVec<u32>,
    pub ranks: ReplicatedVec<u16>,
    pub points: ReplicatedVec<u64>,
}

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("3529CE4B-E0EF-40C5-AF11-5165DA637303")]
#[::nw_network::type_registry(899)]
pub struct ProgressionComponentReplicatedState {
    #[replicated_state(group = 1)]
    pub experience_points: ReplicatedFieldHandler<u32>,
    #[replicated_state(group = 1)]
    pub rested_exp: ReplicatedFieldHandler<u32>,
    pub level: ReplicatedFieldHandler<u32>,
    pub bonus_level: ReplicatedFieldHandler<u32>,
}

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("9D621862-D7F9-44B0-9A64-E3ED8A154AE1")]
#[::nw_network::type_registry(911)]
pub struct CategoricalProgressionReplicatedState {
    pub progression_ids: ReplicatedVec<u32>,
    pub ranks: ReplicatedVec<u16>,
    pub points: ReplicatedVec<u64>,
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

    fn project_index<T>(source: &ReplicatedVec<T>, index: usize) -> Option<ReplicatedVec<T>>
    where
        T: Clone + Marshaler,
    {
        let value = source.values().get(index)?.clone();
        Some(ReplicatedVec::delta(vec![Change::update(
            VlqU64::new(index as u64),
            value,
            source.last_modified(),
        )]))
    }
}
