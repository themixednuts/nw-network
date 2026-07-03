//! GridMate-compatible transport, Hub message framing, and Bevy integration.
//!
//! This crate owns the runtime networking surface. It depends on `nw-network`
//! for generated payload structs, serializer primitives, schema lookup, and
//! shared ID/value types.

#[cfg(feature = "client")]
pub mod bevy;
pub mod capture;
pub mod carrier;
pub mod driver;
pub mod error;
pub mod message;
#[cfg(feature = "server")]
pub mod server;
pub mod session;
#[cfg(any(feature = "client", feature = "server"))]
pub mod session_service;
pub mod spawn;
pub mod state;

pub use nw_network::{
    AzRtti, Marshaler, MarshalerError, ReadBuffer, TypeRegistryEntry, WriteBuffer, generated,
    network_schema, serialize, types,
};

#[cfg(feature = "client")]
pub use bevy::{
    BevyIoTaskSpawner, GridMatePlugin, GridMateSessionService, NetEvent, SessionConnectionRequest,
};
pub use carrier::{CarrierDesc, CarrierProtocolProfile, DataReliability};
pub use error::{GridMateError, Result};
pub use message::{
    Actor, ClientToServer, CorrelatedMetadata, EgressPath, IMESSAGE_BASE_UUID, IngressPath,
    Message as NetworkMessage, MessageEnvelope, MessageEnvelopeView, MessageMetadata, MessagePath,
    MessageTypeId, Receivable, Sendable, ServerToClient, SizedEnvelopeMetadata,
};
#[cfg(feature = "server")]
pub use server::{OutboundTyped, ServerListenerHandle};
pub use session::{CarrierChannel, GridSession, SessionID};
#[cfg(any(feature = "client", feature = "server"))]
pub use session_service::{Event, OutboundSink, Outgoing, SessionId, SessionServiceHandle};
pub use spawn::{BoxedFuture, ShutdownSignal, Spawner, set_spawner, spawn_detached};
