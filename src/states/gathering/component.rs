use crate::serialize::ReplicatedFieldHandler;
use crate::{GatheringStatus, GdeId};

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("0091234C-7DDA-45FC-99EE-D44859C02A7F")]
#[::nw_network::type_registry(81)]
pub struct GatheringComponentReplicatedState {
    pub status: ReplicatedFieldHandler<GatheringStatus>,
    pub amt_remaining: ReplicatedFieldHandler<f32>,
    pub gather_efficiency: ReplicatedFieldHandler<f32>,
    pub num_gathering: ReplicatedFieldHandler<u32>,
    pub gather_ref_count: ReplicatedFieldHandler<u32>,
    pub water_gather_ref_count: ReplicatedFieldHandler<u32>,
    pub is_gathering: ReplicatedFieldHandler<bool>,
    pub gatherable_gdeid: ReplicatedFieldHandler<GdeId>,
    pub sync_gather_ref_count: ReplicatedFieldHandler<u32>,
}
