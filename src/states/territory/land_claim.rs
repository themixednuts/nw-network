use crate::hub::ReplicatedState;
use crate::serialize::{MarshalerError, ReadBuffer, ReplicatedFieldHandler, WriteBuffer};

/// Land-claim territory state.
#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ChunkMarshaler,
    nw_network_derive::AzRtti,
    nw_network_derive::Fragment,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("2A3BF0CE-782D-45E1-B717-E4024E2735F4")]
#[type_registry(62)]
pub struct LandClaimComponentReplicatedState {
    pub claim_key: ReplicatedFieldHandler<u16>,
    pub hub: ReplicatedState,
}

impl LandClaimComponentReplicatedState {
    fn unmarshal_fields(&mut self, rb: &mut ReadBuffer) -> Result<(), MarshalerError> {
        crate::unmarshal_replicated_fields!(rb, self.claim_key)
    }

    fn marshal_fields(&self, wb: &mut WriteBuffer) {
        crate::marshal_replicated_fields!(wb, self.claim_key);
    }
}

crate::impl_hub_fragment!(
    LandClaimComponentReplicatedState,
    hub = hub,
    marshal = marshal_fields,
    unmarshal = unmarshal_fields,
);
