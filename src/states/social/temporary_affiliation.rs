//! Temporary player affiliation replication.
use crate::serialize::marshaler::{Marshal, Unmarshal};

use crate::{az_rtti, replicated_state, type_registry};

use crate::Marshaler;
use crate::serialize::{
    IndexMap, MarshalerError, ReadBuffer, ReplicatedContainer, VlqU64, WriteBuffer,
};
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

#[replicated_state]
#[derive(Debug, Clone, Default)]
#[az_rtti("E45CAB41-47AC-4AC0-8CCF-276816ACAB0A")]
#[type_registry(3563)]
pub struct TemporaryAffiliationReplicatedState {
    pub affiliations: ReplicatedContainer<
        IndexMap<VlqU64, TemporaryAffiliation>,
        MAX_TEMPORARY_AFFILIATION_CHANGES,
    >,
}
