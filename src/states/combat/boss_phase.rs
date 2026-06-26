use crate::serialize::ReplicatedFieldHandler;

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("A80F9DAD-9E8B-4D28-A664-69FAFE4A6676")]
#[::nw_network::type_registry(1296)]
pub struct BossPhaseComponentReplicatedState {
    pub is_active: ReplicatedFieldHandler<bool>,
    pub current_stage_entity_id: ReplicatedFieldHandler<u64>,
    pub current_stage_start_time: ReplicatedFieldHandler<u64>,
}
