use crate::hub::ReplicatedState;
use crate::serialize::ReplicatedFieldHandler;

#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ReplicatedState,
    nw_network_derive::AzRtti,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("A80F9DAD-9E8B-4D28-A664-69FAFE4A6676")]
#[type_registry(1296)]
pub struct BossPhaseComponentReplicatedState {
    pub is_active: ReplicatedFieldHandler<bool>,
    pub current_stage_entity_id: ReplicatedFieldHandler<u64>,
    pub current_stage_start_time: ReplicatedFieldHandler<u64>,

    pub hub: ReplicatedState,
}
