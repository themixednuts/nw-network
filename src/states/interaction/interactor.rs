use crate::GdeId;
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
#[az_rtti("6F49B860-36AA-4583-B426-94CC174B2B9D")]
#[type_registry(3752)]
pub struct InteractorComponentReplicatedState {
    pub enabled: ReplicatedFieldHandler<bool>,
    pub cached_committed_interact_gdeid: ReplicatedFieldHandler<GdeId>,

    pub hub: ReplicatedState,
}
