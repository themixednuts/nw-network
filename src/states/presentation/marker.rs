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
#[az_rtti("A0CA40E0-FAC9-4964-81AD-98E23E2F9CD7")]
#[type_registry(5399)]
pub struct MarkerComponentReplicatedState {
    pub enabled: ReplicatedFieldHandler<bool>,

    pub hub: ReplicatedState,
}
