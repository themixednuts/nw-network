use crate::serialize::ReplicatedFieldHandler;

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("0EF6EEBD-CBBB-48EF-B88F-99BF2B0F8A1A")]
#[::nw_network::type_registry(5257)]
pub struct ChargeComponentReplicatedState {
    pub chrg_pcnt: ReplicatedFieldHandler<u8>,
}
