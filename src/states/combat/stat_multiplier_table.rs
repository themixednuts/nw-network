use std::collections::HashMap;

use crate::Marshaler;
use crate::hub::ReplicatedState;
use crate::serialize::{Change, MarshalerError, MaskChain, ReadBuffer, ReplicatedMap, WriteBuffer};

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

#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ChunkMarshaler,
    nw_network_derive::AzRtti,
    nw_network_derive::Fragment,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("DDD46896-8DAD-4EE5-836E-341A72D44403")]
#[type_registry(1525)]
pub struct StatMultiplierTableComponentReplicatedState {
    pub multiplier_table: ReplicatedMap<u8, StatMultiplierValue>,
    pub stamina_cost_reduction_multipliers: ReplicatedMap<u32, u32>,
    pub xp_increase_multipliers: ReplicatedMap<u32, u32>,
    pub remote_multiplier_table: ReplicatedMap<u8, StatMultiplierValue>,

    pub hub: ReplicatedState,
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

    fn unmarshal_fields(&mut self, rb: &mut ReadBuffer) -> Result<(), MarshalerError> {
        let descriptor_mask = rb.read_u8()?;
        if (descriptor_mask & 0x01) != 0 {
            MaskChain::skip(rb)?;
        }
        if (descriptor_mask & 0x02) != 0 {
            let masks = MaskChain::unmarshal(rb)?;
            if masks.is_field_set(0) {
                self.multiplier_table = ReplicatedMap::unmarshal(rb)?;
            }
            if masks.is_field_set(1) {
                self.stamina_cost_reduction_multipliers = ReplicatedMap::unmarshal(rb)?;
            }
            if masks.is_field_set(2) {
                self.xp_increase_multipliers = ReplicatedMap::unmarshal(rb)?;
            }
        }
        if (descriptor_mask & 0x04) != 0 {
            let masks = MaskChain::unmarshal(rb)?;
            if masks.is_field_set(0) {
                self.remote_multiplier_table = ReplicatedMap::unmarshal(rb)?;
            }
        }
        Ok(())
    }

    fn marshal_fields(&self, wb: &mut WriteBuffer) {
        let group1_dirty = [
            self.multiplier_table.is_dirty(),
            self.stamina_cost_reduction_multipliers.is_dirty(),
            self.xp_increase_multipliers.is_dirty(),
        ];
        let group2_dirty = [self.remote_multiplier_table.is_dirty()];
        let mut descriptor_mask = 0u8;
        if group1_dirty.iter().any(|dirty| *dirty) {
            descriptor_mask |= 0x02;
        }
        if group2_dirty.iter().any(|dirty| *dirty) {
            descriptor_mask |= 0x04;
        }
        wb.write_u8(descriptor_mask);

        if (descriptor_mask & 0x02) != 0 {
            MaskChain::from_dirty_fields(&group1_dirty).marshal(wb);
            if self.multiplier_table.is_dirty() {
                self.multiplier_table.marshal(wb);
            }
            if self.stamina_cost_reduction_multipliers.is_dirty() {
                self.stamina_cost_reduction_multipliers.marshal(wb);
            }
            if self.xp_increase_multipliers.is_dirty() {
                self.xp_increase_multipliers.marshal(wb);
            }
        }
        if (descriptor_mask & 0x04) != 0 {
            MaskChain::from_dirty_fields(&group2_dirty).marshal(wb);
            if self.remote_multiplier_table.is_dirty() {
                self.remote_multiplier_table.marshal(wb);
            }
        }
    }
}

crate::impl_hub_fragment!(
    StatMultiplierTableComponentReplicatedState,
    hub = hub,
    marshal = marshal_fields,
    unmarshal = unmarshal_fields,
);
