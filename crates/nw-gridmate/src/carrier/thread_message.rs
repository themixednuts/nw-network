// Thread messages for communication between main thread and carrier thread
// Following GridMate Carrier.cpp ThreadMessage class

use super::message::MessageData;
use super::types::DisconnectReason;

/// Commands sent from the application's owning task into the
/// per-connection carrier driver task (GridMate: CarrierCommand).
/// Internal to gridmate — consumers go through `CarrierImpl::send` /
/// `disconnect` / etc.
pub(crate) enum CarrierCommand {
    /// Send a message on a specific channel and priority
    SendMessage {
        message: MessageData,
        priority: usize,
    },

    /// Request graceful disconnect
    Disconnect(DisconnectReason),
}

/// Carrier-level events emitted from the per-connection carrier driver
/// task back to the application's owning task (GridMate:
/// `CarrierEvent`; despite the name, no actual OS thread is
/// involved — both ends run on `IoTaskPool`).
///
/// Internal to gridmate — application code consumes the higher-level
/// [`crate::session_service::Event`] surface instead, which both
/// `SessionServiceHandle` (client) and `ServerListenerHandle` (server)
/// emit by translating this enum + adding session/peer context.
#[derive(Debug, Clone)]
pub(crate) enum CarrierEvent {
    /// Connection established successfully
    Connected { version: u32 },

    /// Connection lost or disconnected
    Disconnected { reason: DisconnectReason },

    /// Message received on a channel
    MessageReceived { channel: u8, data: bytes::Bytes },

    /// Error occurred in carrier thread
    Error { description: String },
}

// Spawned tasks use `IoTaskPool` directly today (see the
// `CarrierDriver` and per-session forwarder spawn sites); a
// `Spawner` trait is pending to decouple from Bevy.
