//! Driver Layer — DTLS-encrypted UDP transport.
//!
//! `SecureConnection` is the per-connection DTLS state machine
//! (initiator / responder). On the server side `MultiPeerListener`
//! demuxes a shared UDP socket into per-peer `SecureConnection`s.

pub mod error;
#[cfg(feature = "server")]
pub mod multi_peer;
pub(crate) mod recv_ring;
pub mod secure_connection;
#[cfg(feature = "server")]
pub mod test_cert;

pub use error::DriverError;
#[cfg(feature = "server")]
pub(crate) use multi_peer::MultiPeerEvent;
#[cfg(feature = "server")]
pub use multi_peer::MultiPeerListener;
#[cfg(feature = "server")]
pub use secure_connection::SecureSocketListener;
pub use secure_connection::{Established, SecureConnection, SecureConnectionBuilder};
#[cfg(feature = "server")]
pub use test_cert::generate_self_signed_cert;
