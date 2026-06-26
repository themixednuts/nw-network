use crate::serialize::{HalfF32, ReplicatedFieldHandler};
use crate::{GdeId, Marshaler};

#[derive(Debug, Clone, Copy, Default, PartialEq, Marshaler)]
pub struct ReactionHalfVec3 {
    pub x: HalfF32,
    pub z: HalfF32,
    pub y: HalfF32,
}

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("058AD715-52F1-4DEE-8E9E-059319B6EDD3")]
#[::nw_network::type_registry(1927)]
pub struct ReactionTrackingReplicatedState {
    pub reaction_ref_count: ReplicatedFieldHandler<u8>,
    pub damaging_gdeid: ReplicatedFieldHandler<Option<GdeId>>,
    pub damage_table_id: ReplicatedFieldHandler<u32>,
    pub damage_table_row: ReplicatedFieldHandler<u16>,
    pub damaged_dir: ReplicatedFieldHandler<ReactionHalfVec3>,
    pub impact_dist: ReplicatedFieldHandler<ReactionHalfVec3>,
    pub stun_to_breakout: ReplicatedFieldHandler<f32>,
    pub reaction_flags: ReplicatedFieldHandler<u8>,
    pub attacker_level: ReplicatedFieldHandler<u8>,
}
