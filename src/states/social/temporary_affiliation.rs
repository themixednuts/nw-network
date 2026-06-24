use crate::Marshaler;
use crate::hub::ReplicatedState;
use crate::serialize::{MarshalerError, ReadBuffer, ReplicatedMap, VlqU64, WriteBuffer};

pub const MAX_TEMPORARY_AFFILIATION_CHANGES: usize = 0x3fff;

/// Generated network enum.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(i32)]
pub enum TemporaryAffiliationType {
    #[default]
    Self_ = 0,
    SelfIgnoreActionsToNeutrals = 1,
    SelfIgnoreActionsFromNeutrals = 2,
    SelfIgnoreAllActionsBetweenNeutrals = 3,
    Enemy = 4,
}

impl TemporaryAffiliationType {
    #[must_use]
    pub const fn from_value(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::Self_),
            1 => Some(Self::SelfIgnoreActionsToNeutrals),
            2 => Some(Self::SelfIgnoreActionsFromNeutrals),
            3 => Some(Self::SelfIgnoreAllActionsBetweenNeutrals),
            4 => Some(Self::Enemy),
            _ => None,
        }
    }

    #[must_use]
    pub const fn value(self) -> i32 {
        self as i32
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Self_ => "Self",
            Self::SelfIgnoreActionsToNeutrals => "Self_IgnoreActionsToNeutrals",
            Self::SelfIgnoreActionsFromNeutrals => "Self_IgnoreActionsFromNeutrals",
            Self::SelfIgnoreAllActionsBetweenNeutrals => "Self_IgnoreAllActionsBetweenNeutrals",
            Self::Enemy => "Enemy",
        }
    }
}

impl From<TemporaryAffiliationType> for i32 {
    fn from(value: TemporaryAffiliationType) -> Self {
        value.value()
    }
}

impl Marshaler for TemporaryAffiliationType {
    const MARSHAL_SIZE: usize = <i32 as Marshaler>::MARSHAL_SIZE;

    fn marshal(&self, wb: &mut WriteBuffer) {
        self.value().marshal(wb);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let value = i32::unmarshal(rb)?;
        Self::from_value(value).ok_or(MarshalerError::InvalidRange {
            value: value.try_into().unwrap_or(0),
            min: 0,
            max: 4,
        })
    }
}

/// Generated network enum.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(i32)]
pub enum TemporaryAffiliationRelationship {
    #[default]
    Neutral = 0,
    Friendly = 1,
    Enemy = 2,
}

impl TemporaryAffiliationRelationship {
    #[must_use]
    pub const fn from_value(value: i32) -> Option<Self> {
        match value {
            0 => Some(Self::Neutral),
            1 => Some(Self::Friendly),
            2 => Some(Self::Enemy),
            _ => None,
        }
    }

    #[must_use]
    pub const fn value(self) -> i32 {
        self as i32
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Neutral => "Neutral",
            Self::Friendly => "Friendly",
            Self::Enemy => "Enemy",
        }
    }
}

impl From<TemporaryAffiliationRelationship> for i32 {
    fn from(value: TemporaryAffiliationRelationship) -> Self {
        value.value()
    }
}

impl Marshaler for TemporaryAffiliationRelationship {
    const MARSHAL_SIZE: usize = <i32 as Marshaler>::MARSHAL_SIZE;

    fn marshal(&self, wb: &mut WriteBuffer) {
        self.value().marshal(wb);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let value = i32::unmarshal(rb)?;
        Self::from_value(value).ok_or(MarshalerError::InvalidRange {
            value: value.try_into().unwrap_or(0),
            min: 0,
            max: 2,
        })
    }
}

/// Generated network value shape.
#[derive(nw_network_derive::Marshaler, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TemporaryAffiliation {
    pub affiliation_type: TemporaryAffiliationType,
    pub relationship: TemporaryAffiliationRelationship,
}

#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ReplicatedState,
    nw_network_derive::AzRtti,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("E45CAB41-47AC-4AC0-8CCF-276816ACAB0A")]
#[type_registry(3563)]
pub struct TemporaryAffiliationReplicatedState {
    pub affiliations:
        ReplicatedMap<VlqU64, TemporaryAffiliation, MAX_TEMPORARY_AFFILIATION_CHANGES>,

    pub hub: ReplicatedState,
}
