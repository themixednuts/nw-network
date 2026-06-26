use crate::RemoteServerGdeRef;
use crate::serialize::ReplicatedFieldHandler;

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("A7B26751-0140-48BD-9A8F-34F59EE80D80")]
#[::nw_network::type_registry(1292)]
pub struct CampingComponentReplicatedState {
    pub current_camp_ref: ReplicatedFieldHandler<RemoteServerGdeRef>,
    pub current_camp_skin_id: ReplicatedFieldHandler<u32>,
    pub is_camp_under_attack: ReplicatedFieldHandler<bool>,
    pub camp_bind_cooldown_end_time: ReplicatedFieldHandler<u64>,
}
