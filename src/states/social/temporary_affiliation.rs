use crate::Marshaler;
use crate::serialize::{MarshalerError, ReadBuffer, ReplicatedMap, VlqU64, WriteBuffer};
use crate::types::{TemporaryAffiliationRelationship, TemporaryAffiliationType};

pub const MAX_TEMPORARY_AFFILIATION_CHANGES: usize = 0x3fff;

impl Marshaler for TemporaryAffiliationType {
    const MARSHAL_SIZE: usize = <i32 as Marshaler>::MARSHAL_SIZE;

    fn marshal(&self, wb: &mut WriteBuffer) {
        i32::from(*self).marshal(wb);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let value = i32::unmarshal(rb)?;
        Self::try_from(value).map_err(|_| MarshalerError::InvalidRange {
            value: value.try_into().unwrap_or(0),
            min: 0,
            max: 4,
        })
    }
}

impl Marshaler for TemporaryAffiliationRelationship {
    const MARSHAL_SIZE: usize = <i32 as Marshaler>::MARSHAL_SIZE;

    fn marshal(&self, wb: &mut WriteBuffer) {
        i32::from(*self).marshal(wb);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let value = i32::unmarshal(rb)?;
        Self::try_from(value).map_err(|_| MarshalerError::InvalidRange {
            value: value.try_into().unwrap_or(0),
            min: 0,
            max: 2,
        })
    }
}

#[derive(::nw_network::Marshaler, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TemporaryAffiliation {
    pub affiliation_type: TemporaryAffiliationType,
    pub relationship: TemporaryAffiliationRelationship,
}

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("E45CAB41-47AC-4AC0-8CCF-276816ACAB0A")]
#[::nw_network::type_registry(3563)]
pub struct TemporaryAffiliationReplicatedState {
    pub affiliations:
        ReplicatedMap<VlqU64, TemporaryAffiliation, MAX_TEMPORARY_AFFILIATION_CHANGES>,
}
