use crate::hub::ReplicatedState;
use crate::serialize::ReplicatedFieldHandler;

/// Selected variation replicated state.
#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ReplicatedState,
    nw_network_derive::AzRtti,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("1D7FAC20-19D1-419A-95EB-91253113BC8B")]
#[type_registry(14)]
pub struct VariationComponentReplicatedState {
    pub variation_index: ReplicatedFieldHandler<u8>,
    pub variation_index_16_bit: ReplicatedFieldHandler<u16>,

    pub hub: ReplicatedState,
}
