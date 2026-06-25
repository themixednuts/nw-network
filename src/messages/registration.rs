//! Registration handshake message payloads.

use uuid::Uuid;

use crate::serialize::IndexMap;
use crate::types::Crc32;
use crate::{AzRtti, Marshaler, TypeRegistry};

/// Build/type-index CRC sent at the front of the registration payload.
pub type TypeIndexCrc = Crc32;

/// Ordered client-version token map.
pub type ClientVersionTokenMap = IndexMap<u32, String>;

/// Connection ticket string carried by the registration request.
pub type ConnTicket = String;

/// Entity identifier used by the registration impersonation fields.
#[derive(Debug, Clone, PartialEq, Eq, Marshaler)]
pub enum EntityId {
    String(String),
    Uuid(Uuid),
}

impl Default for EntityId {
    fn default() -> Self {
        Self::String(String::new())
    }
}

impl EntityId {
    #[must_use]
    pub const fn from_uuid(uuid: Uuid) -> Self {
        if uuid.is_nil() {
            Self::String(String::new())
        } else {
            Self::Uuid(uuid)
        }
    }

    #[must_use]
    pub fn as_uuid(&self) -> Option<Uuid> {
        match self {
            Self::String(value) if value.is_empty() => Some(Uuid::nil()),
            Self::String(value) => Uuid::parse_str(value).ok(),
            Self::Uuid(value) => Some(*value),
        }
    }
}

/// Client-to-server registration handshake message.
#[derive(Debug, Clone, PartialEq, Eq, Marshaler, AzRtti, TypeRegistry)]
#[az_rtti("0B826B33-89F5-49E0-B8CB-FE4433427778")]
#[type_registry(19)]
pub struct RegistrationRequestV3Msg {
    /// Retail client build/type-index CRC.
    pub type_index_crc: TypeIndexCrc,
    /// Client build tokens in launch order.
    pub client_version: ClientVersionTokenMap,
    /// Optional connection ticket.
    pub conn_ticket: ConnTicket,
    /// Amazon login/session token.
    pub login_token: LoginToken,
    /// Platform auth ticket payload.
    pub auth_token: AuthToken,
    /// Optional impersonation ids.
    pub impersonate_info: ImpersonatedValues,
    /// Capability negotiation flag.
    pub use_capabilities: bool,
}

/// Amazon login token nested in [`RegistrationRequestV3Msg`].
#[derive(Debug, Clone, PartialEq, Eq, Marshaler)]
pub struct LoginToken {
    pub signature: String,
    pub rep_address: String,
    pub world_id: String,
    pub character_id: String,
    pub persona_id: String,
    pub steam_app_id: u32,
    pub steam_user_id: String,
    pub channel_id: String,
    pub ticket_id: String,
    pub location_group_id: String,
    pub location_id: String,
    pub generate_time: u32,
    pub issue_time: u32,
    pub account_age: u32,
    pub client_capabilities: String,
    pub token_version: u32,
    pub is_trial_owner: bool,
    pub host_hash: String,
    pub is_permanent_app_owner: Option<bool>,
    /// Reserved string field observed empty.
    pub reserved_string_0: String,
    /// Reserved string field observed empty.
    pub reserved_string_1: String,
    pub jwt_claims: String,
}

/// Platform auth token nested in [`RegistrationRequestV3Msg`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct AuthToken {
    pub steam_auth_product_id: u32,
    pub valid: bool,
    pub steam_app_id: u32,
    pub steam_id: String,
    pub steam_auth_ticket: String,
    /// Reserved string field observed empty.
    pub reserved_string_0: String,
    /// Reserved integer field observed zero.
    pub reserved_u32: u32,
    /// Reserved string field observed empty.
    pub reserved_string_1: String,
    /// Reserved string field observed empty.
    pub reserved_string_2: String,
    /// Reserved string field observed empty.
    pub reserved_string_3: String,
    /// Reserved string field observed empty.
    pub reserved_string_4: String,
    /// Reserved string field observed empty.
    pub reserved_string_5: String,
}

/// Optional impersonation ids nested in [`RegistrationRequestV3Msg`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct ImpersonatedValues {
    pub impersonate_id: EntityId,
    pub override_id: EntityId,
}

impl Default for RegistrationRequestV3Msg {
    fn default() -> Self {
        Self {
            type_index_crc: TypeIndexCrc::new(3),
            client_version: ClientVersionTokenMap::new(),
            conn_ticket: String::new(),
            login_token: LoginToken::default(),
            auth_token: AuthToken::default(),
            impersonate_info: ImpersonatedValues::default(),
            use_capabilities: false,
        }
    }
}

impl Default for LoginToken {
    fn default() -> Self {
        Self {
            signature: String::new(),
            rep_address: String::new(),
            world_id: String::new(),
            character_id: String::new(),
            persona_id: String::new(),
            steam_app_id: 0,
            steam_user_id: String::new(),
            channel_id: String::new(),
            ticket_id: String::new(),
            location_group_id: String::new(),
            location_id: String::new(),
            generate_time: 0,
            issue_time: 0,
            account_age: 0,
            client_capabilities: String::new(),
            token_version: 0,
            is_trial_owner: false,
            host_hash: String::new(),
            is_permanent_app_owner: Some(false),
            reserved_string_0: String::new(),
            reserved_string_1: String::new(),
            jwt_claims: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serialize::{CARRIER_ENDIAN, ReadBuffer, WriteBuffer};

    #[test]
    fn registration_request_v3_roundtrips() {
        let mut client_version = ClientVersionTokenMap::new();
        client_version.insert(4, "6031".to_string());
        client_version.insert(3, "400".to_string());
        client_version.insert(2, "1".to_string());
        client_version.insert(1, "Javelin".to_string());

        let impersonated = Uuid::from_u128(0x00112233445566778899aabbccddeeff);
        let msg = RegistrationRequestV3Msg {
            type_index_crc: TypeIndexCrc::new(0x970c_0a5d),
            client_version,
            conn_ticket: "ticket".to_string(),
            login_token: LoginToken {
                signature: "sig".to_string(),
                rep_address: "127.0.0.1:49948".to_string(),
                world_id: "world".to_string(),
                character_id: "character".to_string(),
                persona_id: "persona".to_string(),
                steam_app_id: 1_063_730,
                steam_user_id: "******42".to_string(),
                channel_id: "STEAM_APP_ID.1063730".to_string(),
                ticket_id: "entry-ticket".to_string(),
                location_group_id: "DEFAULT".to_string(),
                location_id: "000".to_string(),
                generate_time: 1_717_000_001,
                issue_time: 1_717_000_002,
                account_age: 99,
                client_capabilities: String::new(),
                token_version: 10,
                is_trial_owner: false,
                host_hash: "host".to_string(),
                is_permanent_app_owner: Some(true),
                reserved_string_0: String::new(),
                reserved_string_1: String::new(),
                jwt_claims: "{}".to_string(),
            },
            auth_token: AuthToken {
                steam_auth_product_id: 1,
                valid: true,
                steam_app_id: 1_063_730,
                steam_id: "76561198000000000".to_string(),
                steam_auth_ticket: "steam|abcdef".to_string(),
                ..Default::default()
            },
            impersonate_info: ImpersonatedValues {
                impersonate_id: EntityId::Uuid(impersonated),
                override_id: EntityId::String(String::new()),
            },
            use_capabilities: false,
        };

        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        msg.marshal(&mut wb);
        let bytes = wb.into_vec();

        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bytes);
        let decoded = RegistrationRequestV3Msg::unmarshal(&mut rb).expect("registration request");

        assert_eq!(decoded, msg);
        assert!(rb.remaining().is_empty());
    }
}
