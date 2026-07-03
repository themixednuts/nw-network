//! Boss phase index replication.

use crate::{az_rtti, replicated_state, type_registry};

use crate::serialize::ReplicatedFieldHandler;

#[replicated_state]
#[derive(Debug, Clone, Default)]
#[az_rtti("A80F9DAD-9E8B-4D28-A664-69FAFE4A6676")]
#[type_registry(1296)]
pub struct BossPhaseComponentReplicatedState {
    pub is_active: ReplicatedFieldHandler<bool>,
    pub current_stage_entity_id: ReplicatedFieldHandler<u64>,
    pub current_stage_start_time: ReplicatedFieldHandler<u64>,
}
