use crate::serialize::ReplicatedFieldHandler;

/// Land-claim territory state.
#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("2A3BF0CE-782D-45E1-B717-E4024E2735F4")]
#[::nw_network::type_registry(62)]
pub struct LandClaimComponentReplicatedState {
    pub claim_key: ReplicatedFieldHandler<u16>,
}
