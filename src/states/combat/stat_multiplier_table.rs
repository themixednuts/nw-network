use std::collections::HashMap;

use crate::Marshaler;
use crate::serialize::{Change, ReplicatedMap};

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct StatMultiplierValue {
    pub amount: u32,
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StatMultiplierSnapshot {
    pub multiplier_table: ReplicatedMap<u8, StatMultiplierValue>,
    pub stamina_cost_reduction_multipliers: ReplicatedMap<u32, u32>,
    pub xp_increase_multipliers: ReplicatedMap<u32, u32>,
    pub remote_multiplier_table: ReplicatedMap<u8, StatMultiplierValue>,
}

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("DDD46896-8DAD-4EE5-836E-341A72D44403")]
#[::nw_network::type_registry(1525)]
pub struct StatMultiplierTableComponentReplicatedState {
    #[replicated_state(group = 1)]
    pub multiplier_table: ReplicatedMap<u8, StatMultiplierValue>,
    #[replicated_state(group = 1)]
    pub stamina_cost_reduction_multipliers: ReplicatedMap<u32, u32>,
    #[replicated_state(group = 1)]
    pub xp_increase_multipliers: ReplicatedMap<u32, u32>,
    #[replicated_state(group = 2)]
    pub remote_multiplier_table: ReplicatedMap<u8, StatMultiplierValue>,
}

impl StatMultiplierTableComponentReplicatedState {
    pub fn apply_snapshot(&mut self, snapshot: StatMultiplierSnapshot) {
        self.multiplier_table = snapshot.multiplier_table;
        self.stamina_cost_reduction_multipliers = snapshot.stamina_cost_reduction_multipliers;
        self.xp_increase_multipliers = snapshot.xp_increase_multipliers;
        self.remote_multiplier_table = snapshot.remote_multiplier_table;
    }

    #[must_use]
    pub fn multiplier_delta(&self, preferred_key: u8) -> Self {
        let mut state = Self::default();
        if self.multiplier_table.has_value() {
            let entry = self
                .multiplier_table
                .values()
                .iter()
                .find(|(key, _)| **key == preferred_key)
                .or_else(|| self.multiplier_table.values().iter().next())
                .map(|(key, value)| {
                    Change::update(*key, value.clone(), self.multiplier_table.last_modified())
                });
            if let Some(entry) = entry {
                state.multiplier_table = ReplicatedMap::delta(vec![entry]);
            }
        }

        if self.remote_multiplier_table.has_value() {
            state.remote_multiplier_table = self.remote_multiplier_table.clone();
        } else if let Some((key, value, sequence)) = state
            .multiplier_table
            .current_value_changes()
            .find(|(_, _, sequence)| sequence.is_valid())
        {
            let mut values = HashMap::new();
            values.insert(*key, value.clone());
            state.remote_multiplier_table = ReplicatedMap::new(sequence, values);
        }
        state
    }
}
