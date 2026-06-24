use std::collections::HashMap;

use crate::Marshaler;
use crate::hub::ReplicatedState;
use crate::serialize::{HalfF32, ReplicatedFieldHandler, ReplicatedMap};

#[derive(Debug, Clone, Default, PartialEq, Marshaler)]
pub struct VitalsStateData {
    pub state: u8,
    pub source_id: u64,
    pub target_id: u64,
    pub flags: u8,
}

#[derive(Debug, Clone, Default, PartialEq, Marshaler)]
pub struct HotAfflictionData {
    pub magnitude: HalfF32,
    pub duration: HalfF32,
    pub source_id: u64,
    pub expiration_time: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Marshaler)]
pub struct ColdAfflictionData {
    pub magnitude: HalfF32,
    pub stacks: u8,
    pub active: bool,
}

#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ReplicatedState,
    nw_network_derive::AzRtti,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("0E721C70-2CDB-4E85-BAE4-D545FDC6D25B")]
#[type_registry(15)]
pub struct VitalsComponentReplicatedState {
    pub health_amount: ReplicatedFieldHandler<f32>,
    pub stamina_amount: ReplicatedFieldHandler<f32>,
    #[replicated_state(group = 1)]
    pub mana_amount: ReplicatedFieldHandler<f32>,
    #[replicated_state(group = 1)]
    pub replicated_afflictions_hot_data: ReplicatedMap<u8, HotAfflictionData>,
    #[replicated_state(group = 1)]
    pub replicated_afflictions_cold_data: ReplicatedMap<u8, ColdAfflictionData>,
    pub vitals_data: ReplicatedFieldHandler<VitalsStateData>,
    pub health_change_flags: ReplicatedFieldHandler<u8>,
    pub health_max: ReplicatedFieldHandler<f32>,
    pub health_tick_rate: ReplicatedFieldHandler<HalfF32>,
    pub stamina_max: ReplicatedFieldHandler<f32>,
    pub stamina_tick_rate: ReplicatedFieldHandler<HalfF32>,
    #[replicated_state(group = 1)]
    pub mana_max: ReplicatedFieldHandler<f32>,
    #[replicated_state(group = 1)]
    pub mana_tick_rate: ReplicatedFieldHandler<HalfF32>,
    pub vitals_id: ReplicatedFieldHandler<u32>,
    pub vitals_category_id: ReplicatedFieldHandler<u32>,
    pub vitals_level: ReplicatedFieldHandler<u32>,
    pub invulnerability: ReplicatedFieldHandler<u8>,
    pub display_immune_when_invulnerable: ReplicatedFieldHandler<u8>,
    pub max_health: ReplicatedFieldHandler<u16>,

    pub hub: ReplicatedState,
}

impl VitalsComponentReplicatedState {
    pub fn set_affliction_sequences(&mut self, hot_sequence: u64, cold_sequence: u64) {
        self.replicated_afflictions_hot_data = ReplicatedMap::new(hot_sequence, HashMap::new());
        self.replicated_afflictions_cold_data = ReplicatedMap::new(cold_sequence, HashMap::new());
    }
}
