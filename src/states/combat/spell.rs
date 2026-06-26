use crate::serialize::{PositionAnchorMarshaler, QuatCompNorm, ReplicatedFieldHandler};
use crate::types::{RemoteServerGdeRef, TimePoint};

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("601F05A6-4BAC-4300-B926-2840A5F2EF95")]
#[::nw_network::type_registry(2912)]
pub struct SpellComponentReplicatedState {
    pub spell_data_id: ReplicatedFieldHandler<u32>,
    pub start_position: ReplicatedFieldHandler<(f32, f32, f32), PositionAnchorMarshaler>,
    pub start_rotation: ReplicatedFieldHandler<QuatCompNorm>,
    pub child_position: ReplicatedFieldHandler<(f32, f32, f32), PositionAnchorMarshaler>,
    pub child_rotation: ReplicatedFieldHandler<QuatCompNorm>,
    pub previous_attachment: ReplicatedFieldHandler<RemoteServerGdeRef>,
    pub attachment: ReplicatedFieldHandler<RemoteServerGdeRef>,
    pub caster_id: ReplicatedFieldHandler<RemoteServerGdeRef>,
    pub spawn_time: ReplicatedFieldHandler<TimePoint>,
    pub spawn_count: ReplicatedFieldHandler<u32>,
    pub aoe_radius_scaling: ReplicatedFieldHandler<f32>,
}
