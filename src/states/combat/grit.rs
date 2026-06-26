use crate::serialize::{HalfF32Marshaler, ReplicatedFieldHandler};

pub type GritHalfFloatField = ReplicatedFieldHandler<f32, HalfF32Marshaler>;

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("C17BC1B3-AB97-402D-98DF-86C2A260D09E")]
#[::nw_network::type_registry(17)]
pub struct GritReplicatedState {
    pub current: GritHalfFloatField,
    pub max: GritHalfFloatField,
    pub no_hit_time_remaining: GritHalfFloatField,
    pub mult_max: GritHalfFloatField,
    pub stagger_resist_mod: GritHalfFloatField,
    pub stagger_resist: GritHalfFloatField,
    pub stagger_resist_nm: GritHalfFloatField,
    pub elsrm: GritHalfFloatField,
    pub total_stagger_damage: GritHalfFloatField,
    pub last: GritHalfFloatField,
}
