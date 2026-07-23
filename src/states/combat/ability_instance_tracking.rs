//! Ability instance tracking payloads.
use crate::serialize::marshaler::{Marshal, Unmarshal};

use crate::{az_rtti, type_registry};

use crate::serialize::{MarshalerError, ReadBuffer, WriteBuffer};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[az_rtti("56C9A913-F676-4E50-B2B4-1C9F8719DF56")]
#[type_registry(5116)]
pub struct AbilityInstanceTrackingComponentReplicatedState {
    pub value: String,
}

impl Marshal for AbilityInstanceTrackingComponentReplicatedState {
    fn marshal(&self, wb: &mut WriteBuffer) {
        self.value.marshal(wb);
    }
}

impl Unmarshal for AbilityInstanceTrackingComponentReplicatedState {
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        Ok(Self {
            value: String::unmarshal(rb)?,
        })
    }
}
