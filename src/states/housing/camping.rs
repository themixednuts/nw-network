//! Camping placement and owner replication.

use crate::{az_rtti, replicated_state, type_registry};

use crate::RemoteServerGdeRef;
use crate::serialize::ReplicatedFieldHandler;

#[replicated_state]
#[derive(Debug, Clone, Default)]
#[az_rtti("A7B26751-0140-48BD-9A8F-34F59EE80D80")]
#[type_registry(1292)]
pub struct CampingComponentReplicatedState {
    pub current_camp_ref: ReplicatedFieldHandler<RemoteServerGdeRef>,
    pub current_camp_skin_id: ReplicatedFieldHandler<u32>,
    pub is_camp_under_attack: ReplicatedFieldHandler<bool>,
    pub camp_bind_cooldown_end_time: ReplicatedFieldHandler<u64>,
}
