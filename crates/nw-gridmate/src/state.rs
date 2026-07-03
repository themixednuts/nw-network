//! Shared state markers for type-level state machines
//!
//! These marker types are used by both Session and Carrier to represent
//! common connection states. Module-specific states (Idle, InSession, Leaving
//! for Session; Disconnected, Disconnecting for Carrier) are defined in their
//! respective modules.

/// Connecting state - connection/handshake in progress
/// Shared between Session and Carrier as they represent the same conceptual state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Connecting;

/// Connected state - connection established and ready
/// Shared between Session and Carrier as they represent the same conceptual state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Connected;
