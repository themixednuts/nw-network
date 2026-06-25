use crate::serialize::{Marshaler, MarshalerError, ReadBuffer, WriteBuffer};

#[derive(
    Debug, Clone, Default, PartialEq, Eq, nw_network_derive::AzRtti, nw_network_derive::TypeRegistry,
)]
#[az_rtti("56C9A913-F676-4E50-B2B4-1C9F8719DF56")]
#[type_registry(5116)]
pub struct AbilityInstanceTrackingComponentReplicatedState {
    pub value: String,
}

impl Marshaler for AbilityInstanceTrackingComponentReplicatedState {
    fn marshal(&self, wb: &mut WriteBuffer) {
        self.value.marshal(wb);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        Ok(Self {
            value: String::unmarshal(rb)?,
        })
    }
}
