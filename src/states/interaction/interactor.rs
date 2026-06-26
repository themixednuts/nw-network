use crate::GdeId;
use crate::serialize::ReplicatedFieldHandler;

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("6F49B860-36AA-4583-B426-94CC174B2B9D")]
#[::nw_network::type_registry(3752)]
pub struct InteractorComponentReplicatedState {
    pub enabled: ReplicatedFieldHandler<bool>,
    pub cached_committed_interact_gdeid: ReplicatedFieldHandler<GdeId>,
}
