use crate::{
    az::{asset::AssetId, crc::Crc32, uuid::Uuid as AzUuid},
    types::{GDEID, RemoteServerContextRef, RemoteServerGDERef},
};

impl Crc32 {
    #[must_use]
    pub const fn from_ascii_case_insensitive(bytes: &[u8]) -> Self {
        Self::from_bytes_lower(bytes)
    }
}

impl AssetId {
    #[must_use]
    pub fn from_uuid(guid: ::uuid::Uuid, sub_id: u32) -> Self {
        Self::new(guid.into(), sub_id)
    }
}

impl From<::uuid::Uuid> for AssetId {
    fn from(guid: ::uuid::Uuid) -> Self {
        Self::from_uuid(guid, 0)
    }
}

impl From<(::uuid::Uuid, u32)> for AssetId {
    fn from((guid, sub_id): (::uuid::Uuid, u32)) -> Self {
        Self::from_uuid(guid, sub_id)
    }
}

impl GDEID {
    pub const INVALID: Self = Self {
        id: u32::MAX as u64,
    };

    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self { id: value }
    }

    #[must_use]
    pub const fn value(self) -> u64 {
        self.id
    }

    #[must_use]
    pub const fn is_invalid(self) -> bool {
        self.id == Self::INVALID.id
    }

    #[must_use]
    pub const fn is_valid(self) -> bool {
        !self.is_invalid()
    }
}

impl From<u64> for GDEID {
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}

impl From<GDEID> for u64 {
    fn from(value: GDEID) -> Self {
        value.value()
    }
}

impl RemoteServerContextRef {
    #[must_use]
    pub const fn new(actor_id: AzUuid) -> Self {
        Self { actor_id }
    }

    #[must_use]
    pub fn from_uuid(actor_id: ::uuid::Uuid) -> Self {
        Self::new(actor_id.into())
    }

    #[must_use]
    pub const fn has_actor_id(self) -> bool {
        !self.actor_id.is_nil()
    }
}

impl RemoteServerGDERef {
    #[must_use]
    pub const fn new(remote_server_context: RemoteServerContextRef, target_id: GDEID) -> Self {
        Self {
            remote_server_context,
            target_id,
        }
    }

    #[must_use]
    pub const fn has_target(self) -> bool {
        self.target_id.is_valid()
    }
}
