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
#[az_rtti("C3F8F5F5-5D8E-45A0-890C-36B9B0183C8E")]
#[type_registry(3786)]
pub struct CurrencyComponentReplicatedState {
    pub currency: ReplicatedFieldHandler<u64>,

    pub hub: ReplicatedState,
}
