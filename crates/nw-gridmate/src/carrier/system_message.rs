//! System message IDs.
//!
//! Direct port of Lumberyard's `GridMate::SystemMessageId` enum. The
//! carrier protocol reserves these for connection control and acks;
//! they ride on [`super::SYSTEM_CHANNEL`] and never surface to
//! application code.

pub const SM_CONNECT_REQUEST: u8 = 1;
pub const SM_CONNECT_ACK: u8 = 2;
pub const SM_DISCONNECT: u8 = 3;
pub const SM_CLOCK_SYNC: u8 = 4;
/// Marker — carrier-thread message ids are `> SM_CT_FIRST`.
pub const SM_CT_FIRST: u8 = 5;
pub const SM_CT_ACKS: u8 = 6;
pub const SM_CT_CONN_CONTROL: u8 = 7;
pub const SM_CT_BANDWIDTH: u8 = 8;
