// Common Carrier types
// Following GridMate Carrier structures

use num_enum::{IntoPrimitive, TryFromPrimitive};

/// Sequence number type with wrapping semantics (GridMate: TrafficControl::SequenceNumber = u16)
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, derive_more::From, derive_more::Into,
)]
pub struct SequenceNumber(u16);

impl SequenceNumber {
    pub const ZERO: Self = Self(0);
    pub const MAX: Self = Self(u16::MAX);

    pub fn new(value: u16) -> Self {
        Self(value)
    }

    pub fn next(self) -> Self {
        Self(self.0.wrapping_add(1))
    }

    pub fn get(self) -> u16 {
        self.0
    }

    pub fn wrapping_add(self, other: u16) -> Self {
        Self(self.0.wrapping_add(other))
    }

    pub fn wrapping_sub(self, other: Self) -> u16 {
        other.0.wrapping_sub(self.0)
    }

    pub fn sequential_distance_to(self, other: Self) -> u16 {
        other.0.wrapping_sub(self.0)
    }
}

/// Maximum sequence number value (GridMate: SequenceNumberMax = 0xFFFF)
pub const SEQUENCE_NUMBER_MAX: SequenceNumber = SequenceNumber::MAX;

/// System channel constant (GridMate: k_systemChannel = 3)
pub const SYSTEM_CHANNEL: u8 = 3;

/// Maximum number of channels (GridMate: k_maxNumberOfChannels = 4)
/// Channels 0-3 where channel 3 is the system channel
pub const MAX_CHANNELS: usize = 4;

/// Channel ID type
/// Newtype for type safety - prevents mixing channel IDs with other u8 values
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, derive_more::From, derive_more::Into)]
pub struct ChannelId(u8);

impl ChannelId {
    pub const SYSTEM: Self = Self(SYSTEM_CHANNEL);

    pub fn new(channel: u8) -> Option<Self> {
        if channel < MAX_CHANNELS as u8 {
            Some(Self(channel))
        } else {
            None
        }
    }

    pub fn new_unchecked(channel: u8) -> Self {
        Self(channel)
    }

    pub fn get(&self) -> u8 {
        self.0
    }

    pub fn as_usize(&self) -> usize {
        self.0 as usize
    }
}

/// Number of priority queues — matches `Carrier::PRIORITY_MAX`. One queue per
/// [`super::message::DataPriority`] variant excluding the `Max` sentinel:
/// `System(0)` / `High(1)` / `Normal(2)` / `Low(3)`.
pub const PRIORITY_MAX: usize = 4;

/// Source `Carrier::ConnectionStates`.
///
/// The async Rust carrier still uses typestate markers for valid operations.
/// This enum preserves the source query surface for status/reporting code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum CarrierConnectionState {
    Connecting = 0,
    Connected = 1,
    Disconnecting = 2,
    Disconnected = 3,
}

/// Source `Carrier::ReceiveResult::States`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum ReceiveState {
    Received = 0,
    UnsufficientBufferSize = 1,
    NoMessageToReceive = 2,
}

/// Source `Carrier::ReceiveResult`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReceiveResult {
    pub state: ReceiveState,
    pub num_bytes: u32,
}

impl ReceiveResult {
    #[must_use]
    pub const fn received(num_bytes: u32) -> Self {
        Self {
            state: ReceiveState::Received,
            num_bytes,
        }
    }

    #[must_use]
    pub const fn unsufficient_buffer_size(required_bytes: u32) -> Self {
        Self {
            state: ReceiveState::UnsufficientBufferSize,
            num_bytes: required_bytes,
        }
    }

    #[must_use]
    pub const fn no_message_to_receive() -> Self {
        Self {
            state: ReceiveState::NoMessageToReceive,
            num_bytes: 0,
        }
    }
}

/// Source `Carrier::FlowInformation`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FlowInformation {
    pub num_to_send_messages: usize,
    pub num_to_receive_messages: usize,
    pub data_in_transfer: usize,
    pub congestion_window: usize,
}

/// Source `Carrier::CarrierStats`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CarrierStats {
    pub message_data_allocation_count: u64,
    pub free_message_data_block_num: u64,
    pub message_allocation_count: u64,
    pub free_message_num: u64,
    pub data_gram_allocation_count: u64,
    pub free_data_gram_num: u64,
    pub converted_reliable_messages: u64,
}

/// Disconnect reason
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum DisconnectReason {
    UserRequested = 0,
    BadConnection,
    BadPackets,
    DriverError,
    HandshakeRejected,
    HandshakeTimeout,
    WasAlreadyConnected,
    ShuttingDown,
    DebugDeleteConnection,
    VersionMismatch,
}

/// Legacy alias for backward compatibility
#[deprecated(note = "use DisconnectReason instead")]
pub type CarrierDisconnectReason = DisconnectReason;

impl AsRef<str> for DisconnectReason {
    fn as_ref(&self) -> &str {
        match self {
            Self::UserRequested => "User requested",
            Self::BadConnection => "Bad connection",
            Self::BadPackets => "Bad packets",
            Self::DriverError => "Driver error",
            Self::HandshakeRejected => "Handshake rejected",
            Self::HandshakeTimeout => "Handshake timeout",
            Self::WasAlreadyConnected => "Was already connected",
            Self::ShuttingDown => "Shutting down",
            Self::DebugDeleteConnection => "Debug delete connection",
            Self::VersionMismatch => "Version mismatch",
        }
    }
}
