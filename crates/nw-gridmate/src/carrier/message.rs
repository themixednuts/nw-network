// MessageData - Smallest unit of data transport in Carrier
// Following GridMate Carrier.cpp lines 68-111

use super::types::{MAX_CHANNELS, SequenceNumber};
use crate::serialize::ReadBuffer;
use bytes::Bytes;
use num_enum::{IntoPrimitive, TryFromPrimitive};
#[cfg(debug_assertions)]
use smallvec::{SmallVec, smallvec};
use tracing::trace;

/// Message flags (GridMate: MessageData::MessageFlags)
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum MessageFlags {
    /// Reliable delivery
    Reliable = 1 << 0,
    /// Multiple chunks to form full message
    Chunks = 1 << 2,
    /// Sequential ID
    SequentialId = 1 << 3,
    /// Sequential reliable ID
    SequentialRelId = 1 << 4,
    /// Data channel
    DataChannel = 1 << 5,
    /// Connection state (trying to connect)
    Connecting = 1 << 7,
}

/// Data reliability (GridMate: Carrier::DataReliability)
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum DataReliability {
    /// Unreliable (send once, no retry)
    Unreliable = 0,
    /// Reliable (guaranteed delivery)
    Reliable = 1,
}

/// Data delivery priority — Rust port of Lumberyard's
/// `Carrier::DataPriority`.
///
/// Values double as queue indices, so the discriminants are kept in
/// strict descending-priority order matching the C++ enum:
///
/// ```text
///   System(0) > High(1) > Normal(2) > Low(3)
/// ```
///
/// `Max = 4` is the array-sizing sentinel — see [`super::types::PRIORITY_MAX`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum DataPriority {
    /// `PRIORITY_SYSTEM` — highest priority, reserved for internal
    /// carrier messages (connect / ACK / system-channel traffic).
    System = 0,
    /// `PRIORITY_HIGH` — high-priority application messages.
    High = 1,
    /// `PRIORITY_NORMAL` — default for ordinary application messages.
    Normal = 2,
    /// `PRIORITY_LOW` — bandwidth-permitting; sent only when nothing
    /// higher is pending.
    Low = 3,
    /// `PRIORITY_MAX` — array-sizing sentinel; not a valid priority value.
    Max = 4,
}

/// Location of one carrier frame inside the decoded carrier payload.
#[cfg(debug_assertions)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MessageWireSpan {
    /// Carrier datagram sequence that carried this frame, once the
    /// frame has been decoded from a datagram.
    pub datagram_sequence: Option<SequenceNumber>,
    /// Start of the carrier frame header.
    pub frame_start: usize,
    /// End of the carrier frame, after its payload bytes.
    pub frame_end: usize,
    /// Start of this frame's `MessageData::data` payload.
    pub payload_start: usize,
    /// End of this frame's `MessageData::data` payload.
    pub payload_end: usize,
}

#[cfg(debug_assertions)]
pub type MessageWireSpans = SmallVec<[MessageWireSpan; 1]>;

/// Carrier message (GridMate: MessageData struct)
/// Smallest unit of data we transport in the carrier
pub struct MessageData {
    /// Reliability mode
    pub reliability: DataReliability,

    /// Channel for sending [0..k_maxNumberOfChannels)
    pub channel: u8,

    /// Number of chunks/messages needed to assemble full message [1..N]
    pub num_chunks: SequenceNumber,

    /// Message sequence number
    pub sequence_number: SequenceNumber,

    /// Reliable sequence number (valid if reliability is RELIABLE)
    pub send_reliable_seq_num: SequenceNumber,

    /// Actual message payload
    pub data: Bytes,

    /// Carrier payload spans that contributed to this message. A
    /// single unchunked message has one span; a reassembled reliable
    /// message carries one span per chunk.
    #[cfg(debug_assertions)]
    pub wire_spans: MessageWireSpans,

    /// True if message generated while connecting, false otherwise
    pub is_connecting: bool,

    /// ACK callback to execute after receiving ACK
    pub ack_callback: Option<Box<dyn FnOnce() + Send>>,
}

impl MessageData {
    /// Parse a carrier message header from a buffer, advancing the
    /// cursor through `[flags][data_size]([channel])([num_chunks])([seq])([rel_seq])[payload]`
    /// and returning the populated `MessageData`. The caller threads
    /// `prev_msg_seq_num` / `prev_reliable_msg_seq_num` /
    /// `current_channel` so sequential-id messages can infer the
    /// implicit next value.
    ///
    /// Returns `None` on truncated input or invalid flag bits.
    pub fn read_header(
        buffer: &mut ReadBuffer,
        prev_msg_seq_num: &mut [SequenceNumber; MAX_CHANNELS],
        prev_reliable_msg_seq_num: &mut [SequenceNumber; MAX_CHANNELS],
        current_channel: &mut u8,
    ) -> Option<MessageData> {
        #[cfg(debug_assertions)]
        let frame_start = buffer.position();
        let flags = buffer.read_u8().ok()?;

        const MF_UNUSED_FLAGS: u8 = (1 << 1) | (1 << 6);
        if (flags & MF_UNUSED_FLAGS) != 0 {
            trace!("Packet corrupted or stream misaligned");
            return None;
        }

        let data_size = buffer.read_u16().ok()?;

        let reliability = if (flags & MessageFlags::Reliable as u8) != 0 {
            DataReliability::Reliable
        } else {
            DataReliability::Unreliable
        };

        if (flags & MessageFlags::DataChannel as u8) != 0 {
            *current_channel = buffer.read_u8().ok()?;
        }

        if *current_channel >= MAX_CHANNELS as u8 {
            return None;
        }

        let num_chunks = if (flags & MessageFlags::Chunks as u8) != 0 {
            SequenceNumber::from(buffer.read_u16().ok()?)
        } else {
            SequenceNumber::from(1)
        };

        let sequence_number = if (flags & MessageFlags::SequentialId as u8) != 0 {
            prev_msg_seq_num[*current_channel as usize] =
                prev_msg_seq_num[*current_channel as usize].next();
            prev_msg_seq_num[*current_channel as usize]
        } else {
            let seq = SequenceNumber::from(buffer.read_u16().ok()?);
            prev_msg_seq_num[*current_channel as usize] = seq;
            seq
        };

        let send_reliable_seq_num = if (flags & MessageFlags::SequentialRelId as u8) != 0 {
            if (flags & MessageFlags::Reliable as u8) != 0 {
                prev_reliable_msg_seq_num[*current_channel as usize] =
                    prev_reliable_msg_seq_num[*current_channel as usize].next();
            }
            prev_reliable_msg_seq_num[*current_channel as usize]
        } else {
            let seq = SequenceNumber::from(buffer.read_u16().ok()?);
            prev_reliable_msg_seq_num[*current_channel as usize] = seq;
            seq
        };

        let is_connecting = (flags & MessageFlags::Connecting as u8) != 0;

        #[cfg(debug_assertions)]
        let payload_start = buffer.position();
        let payload = buffer.read_bytes(data_size as usize).ok()?;
        #[cfg(debug_assertions)]
        let payload_end = buffer.position();
        #[cfg(debug_assertions)]
        let frame_end = payload_end;

        Some(MessageData {
            reliability,
            channel: *current_channel,
            num_chunks,
            sequence_number,
            send_reliable_seq_num,
            data: Bytes::copy_from_slice(payload),
            #[cfg(debug_assertions)]
            wire_spans: smallvec![MessageWireSpan {
                datagram_sequence: None,
                frame_start,
                frame_end,
                payload_start,
                payload_end,
            }],
            is_connecting,
            ack_callback: None,
        })
    }

    /// Create new message (GridMate pattern)
    pub fn new() -> Self {
        use super::types::{SEQUENCE_NUMBER_MAX, SequenceNumber};
        Self {
            reliability: DataReliability::Unreliable,
            channel: 0,
            num_chunks: SequenceNumber::from(1),
            sequence_number: SequenceNumber::ZERO,
            send_reliable_seq_num: SEQUENCE_NUMBER_MAX, // GridMate: 0xFFFF for unreliable
            data: Bytes::new(),
            #[cfg(debug_assertions)]
            wire_spans: MessageWireSpans::new(),
            is_connecting: false,
            ack_callback: None,
        }
    }

    /// Create unreliable message on a channel
    pub fn unreliable(channel: u8, data: Bytes) -> Self {
        Self {
            channel,
            reliability: DataReliability::Unreliable,
            data,
            ..Self::new()
        }
    }

    /// Create reliable message on a channel
    pub fn reliable(channel: u8, data: Bytes) -> Self {
        Self {
            channel,
            reliability: DataReliability::Reliable,
            data,
            ..Self::new()
        }
    }

    /// Get message size
    pub fn data_size(&self) -> u16 {
        self.data.len() as u16
    }
}

impl Default for MessageData {
    fn default() -> Self {
        Self::new()
    }
}
