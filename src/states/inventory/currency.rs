use crate::serialize::ReplicatedFieldHandler;

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("C3F8F5F5-5D8E-45A0-890C-36B9B0183C8E")]
#[::nw_network::type_registry(3786)]
pub struct CurrencyComponentReplicatedState {
    pub currency: ReplicatedFieldHandler<u64>,
}
