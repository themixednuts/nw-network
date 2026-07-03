// DatagramData - Group of MessageData
// Following GridMate Carrier.cpp lines 113-128

use super::message::MessageData;
use super::types::{PRIORITY_MAX, SequenceNumber};
use crate::serialize::{ReadBuffer, error::MarshalerError};

/// Carrier datagram header.
/// They added a compression handshake that GridMate originally had as TODO
/// Format:
/// - Byte 0: 0x80 (uncompressed) or 0x81 (compressed) - check with & 0x01
/// - Byte 1: 0x01 if has compressor
/// - Bytes 2-3: u16 sequence number (big-endian)
///
/// Note: GridMate's compressor returns the uncompressed size, meaning it's stored
/// in the compressed payload itself (part of LZ4 block format), NOT in datagram header
#[derive(Debug, Clone, Copy)]
pub struct DatagramHeader {
    pub is_compressed: bool,
    pub has_compressor: bool,
    pub sequence_number: SequenceNumber,
}

impl DatagramHeader {
    /// Wire size of the header (always 4 bytes).
    pub const SIZE: usize = 4;

    /// Byte-0 marker for an uncompressed datagram.
    pub const UNCOMPRESSED: u8 = 0x80;
    /// Byte-0 marker for an LZ4-compressed datagram.
    pub const COMPRESSED: u8 = 0x81;
    /// Byte-1 marker indicating the sender advertises compressor support.
    pub const HAS_COMPRESSOR: u8 = 0x01;

    /// Stream-read the carrier datagram header from `rb`. Advances
    /// `rb` by exactly [`DatagramHeader::SIZE`] bytes on success;
    /// `rb.remaining()` after the call is the datagram payload.
    pub fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        // Byte 0: 0x80 = not compressed, 0x81 = compressed
        let first_byte = rb.read_u8()?;
        if first_byte != 0x80 && first_byte != 0x81 {
            return Err(MarshalerError::InvalidDiscriminant { value: first_byte });
        }
        let is_compressed = (first_byte & 0x01) != 0;

        // Byte 1: 0x01 if has compressor
        let has_compressor = rb.read_u8()? == 0x01;

        // Bytes 2-3: u16 sequence number (big-endian / network order)
        let sequence_number = SequenceNumber::from(rb.read_u16()?);

        Ok(Self {
            is_compressed,
            has_compressor,
            sequence_number,
        })
    }
}

/// Flow control data for datagram (GridMate: TrafficControl::DataGramControlData)
#[derive(Debug, Clone)]
pub struct DataGramControlData {
    /// Datagram sequence number
    pub sequence_number: SequenceNumber,

    /// Total size of datagram
    pub size: u16,

    /// Effective size (payload without system messages)
    pub effective_size: u16,

    /// Timestamp when sent
    pub sent_time: std::time::Instant,
}

impl DataGramControlData {
    pub fn new(sequence_number: SequenceNumber) -> Self {
        Self {
            sequence_number,
            size: 0,
            effective_size: 0,
            sent_time: std::time::Instant::now(),
        }
    }
}

/// Carrier datagram (GridMate: DatagramData struct)
/// A group of MessageData
pub struct DatagramData {
    /// Flow control data
    pub flow_control: DataGramControlData,

    /// Size of data in toResend list (not including headers)
    pub resend_data_size: u16,

    /// Lists of reliable messages that were part of datagram (by priority)
    /// May need to resend them
    pub to_resend: [Vec<MessageData>; PRIORITY_MAX],

    /// ACK callbacks for this datagram
    pub ack_callbacks: Vec<Box<dyn FnOnce() + Send>>,
}

impl DatagramData {
    /// Create new datagram (GridMate pattern)
    pub fn new(sequence_number: SequenceNumber) -> Self {
        Self {
            flow_control: DataGramControlData::new(sequence_number),
            resend_data_size: 0,
            // One resend queue per priority — sized by `PRIORITY_MAX`
            // so renumbering `DataPriority` doesn't quietly truncate.
            to_resend: std::array::from_fn(|_| Vec::new()),
            ack_callbacks: Vec::new(),
        }
    }
}

/// Datagram history list constants (GridMate: DataGramHistoryList)
pub mod history {
    use super::SequenceNumber;

    /// Maximum number of ACKs before removing from history
    pub const MAX_NUM_ACKS: u8 = 3;

    /// Maximum number of bytes in datagram history
    pub const MAX_NUM_BYTES: u8 = 64;

    /// Datagram history ID size (512 = 64 * 8 bits)
    pub const HISTORY_SIZE: usize = (MAX_NUM_BYTES as usize) * 8;

    /// History element (GridMate: DataGramHistoryList::Element)
    #[derive(Debug, Clone, Copy)]
    pub struct HistoryElement {
        pub sequence_number: SequenceNumber,
        /// -1 if slot not used, otherwise count of ACKs sent
        pub num_acks_sent: i32,
    }

    impl Default for HistoryElement {
        fn default() -> Self {
            Self {
                sequence_number: SequenceNumber::ZERO,
                num_acks_sent: -1,
            }
        }
    }
}
