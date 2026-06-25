//! Concrete network message payloads.

pub mod registration;

pub use registration::{
    AuthToken, ClientVersionTokenMap, ConnTicket, EntityId, ImpersonatedValues, LoginToken,
    RegistrationRequestV3Msg, TypeIndexCrc,
};
