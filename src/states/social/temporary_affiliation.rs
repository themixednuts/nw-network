//! Temporary player affiliation replication.
use crate::serialize::marshaler::{Marshal, Unmarshal};

use crate::Marshaler;
use crate::serialize::{MarshalerError, ReadBuffer, WriteBuffer};
use crate::types::{TemporaryAffiliationRelationship, TemporaryAffiliationType};

pub const MAX_TEMPORARY_AFFILIATION_CHANGES: usize = 0x3fff;

impl Marshal for TemporaryAffiliationType {
    const MARSHAL_SIZE: usize = <i32 as Marshal>::MARSHAL_SIZE;

    fn marshal(&self, wb: &mut WriteBuffer) {
        i32::from(*self).marshal(wb);
    }
}

impl Unmarshal for TemporaryAffiliationType {
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let value = i32::unmarshal(rb)?;
        Self::try_from(value).map_err(|_| MarshalerError::InvalidRange {
            value: value.try_into().unwrap_or(0),
            min: 0,
            max: 4,
        })
    }
}

impl Marshal for TemporaryAffiliationRelationship {
    const MARSHAL_SIZE: usize = <i32 as Marshal>::MARSHAL_SIZE;

    fn marshal(&self, wb: &mut WriteBuffer) {
        i32::from(*self).marshal(wb);
    }
}

impl Unmarshal for TemporaryAffiliationRelationship {
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let value = i32::unmarshal(rb)?;
        Self::try_from(value).map_err(|_| MarshalerError::InvalidRange {
            value: value.try_into().unwrap_or(0),
            min: 0,
            max: 2,
        })
    }
}

#[derive(Marshaler, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TemporaryAffiliation {
    pub affiliation_type: TemporaryAffiliationType,
    pub relationship: TemporaryAffiliationRelationship,
}
pub use crate::generated::states::TemporaryAffiliationReplicatedState;
