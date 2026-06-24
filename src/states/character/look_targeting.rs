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
#[az_rtti("490DB5F1-4E39-483A-9897-78FA312E45B5")]
#[type_registry(670)]
pub struct LookTargetingComponentReplicatedState {
    pub enabled: ReplicatedFieldHandler<bool>,

    pub hub: ReplicatedState,
}
