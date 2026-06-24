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
#[az_rtti("0EF6EEBD-CBBB-48EF-B88F-99BF2B0F8A1A")]
#[type_registry(5257)]
pub struct ChargeComponentReplicatedState {
    pub chrg_pcnt: ReplicatedFieldHandler<u8>,

    pub hub: ReplicatedState,
}
