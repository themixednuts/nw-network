pub use nw_network_types::{
    az::{asset::AssetId, crc::Crc32},
    types::{
        AfflictionData, CharacterAttributeType, GDEID as GdeId, GameModeParticipantStatus,
        GatheringStatus, GeneralCooldownType, GridSides, PaperdollSlotAlias, RecipeCooldownData,
        RemoteServerContextRef, RemoteServerFacetRefGameModeParticipantComponentServerFacet,
        RemoteServerGDERef as RemoteServerGdeRef, RemoteTypelessServerFacetRef,
        ReplicationCategory, TemporaryAffiliationRelationship, TemporaryAffiliationType, TimePoint,
        WallClockTimePoint,
    },
};
use uuid::Uuid;

/// Runtime type identity: stable UUID plus a human-readable type name.
pub trait AzRtti {
    const TYPE_ID: Uuid;
    const TYPE_NAME: &'static str;
}

/// Compact type-index entry used by state-bundle fragment headers.
pub trait TypeRegistryEntry {
    const TYPE_INDEX: u32;
}

/// Request id shape used by actor-scoped payloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ActorRequestId {
    pub request_id: u64,
    pub target_local_id: u64,
}

impl ActorRequestId {
    pub const INVALID_TARGET_LOCAL_ID: u64 = u32::MAX as u64;

    #[must_use]
    pub const fn new(request_id: u64, target_local_id: u64) -> Self {
        Self {
            request_id,
            target_local_id,
        }
    }
}

impl Default for ActorRequestId {
    fn default() -> Self {
        Self {
            request_id: 0,
            target_local_id: Self::INVALID_TARGET_LOCAL_ID,
        }
    }
}

/// Opaque entity identifier carried as a `u64`.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EntityId(u64);

impl EntityId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

impl From<u64> for EntityId {
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}

impl From<EntityId> for u64 {
    fn from(value: EntityId) -> Self {
        value.value()
    }
}

/// Opaque component identifier carried as a `u64`.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ComponentId(u64);

impl ComponentId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn value(self) -> u64 {
        self.0
    }
}

impl From<u64> for ComponentId {
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}

impl From<ComponentId> for u64 {
    fn from(value: ComponentId) -> Self {
        value.value()
    }
}

/// Game-data reference carried as one UUID.
#[repr(transparent)]
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, ::nw_network::Marshaler,
)]
pub struct GdeRef(Uuid);

impl GdeRef {
    #[must_use]
    pub const fn new(value: Uuid) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn value(self) -> Uuid {
        self.0
    }
}

impl From<Uuid> for GdeRef {
    fn from(value: Uuid) -> Self {
        Self::new(value)
    }
}

impl From<GdeRef> for Uuid {
    fn from(value: GdeRef) -> Self {
        value.value()
    }
}

#[cfg(test)]
mod crc32_tests {
    use super::Crc32;

    #[test]
    fn computes_standard_crc32_check_value() {
        assert_eq!(Crc32::from_bytes(b"123456789").value(), 0xcbf4_3926);
    }

    #[test]
    fn ascii_case_insensitive_crc_matches_lowercase_input() {
        assert_eq!(
            Crc32::from_ascii_case_insensitive(b"PlayerBackstory"),
            Crc32::from_bytes(b"playerbackstory")
        );
    }
}

/// Entity reference carried either as a string name or a UUID.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EntityRef {
    String(String),
    Uuid { uuid: Uuid, format_flags: u8 },
}

impl EntityRef {
    #[must_use]
    pub fn string(value: impl Into<String>) -> Self {
        Self::String(value.into())
    }

    #[must_use]
    pub const fn uuid(uuid: Uuid, format_flags: u8) -> Self {
        Self::Uuid { uuid, format_flags }
    }

    #[must_use]
    pub const fn is_string(&self) -> bool {
        matches!(self, Self::String(_))
    }

    #[must_use]
    pub const fn is_uuid(&self) -> bool {
        matches!(self, Self::Uuid { .. })
    }

    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value.as_str()),
            Self::Uuid { .. } => None,
        }
    }

    #[must_use]
    pub const fn as_uuid(&self) -> Option<Uuid> {
        match self {
            Self::String(_) => None,
            Self::Uuid { uuid, .. } => Some(*uuid),
        }
    }
}

impl Default for EntityRef {
    fn default() -> Self {
        Self::String(String::new())
    }
}
