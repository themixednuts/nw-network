//! MTU + chunk-size arithmetic.
//!
//! Direct port of Lumberyard's `GridMate::CarrierDriver` MTU
//! calculation. The carrier splits oversized messages into chunks
//! whose size is constrained by the UDP MTU minus transport
//! overhead (UDP/IP + DTLS record + DTLS cipher + carrier message
//! header).
//!
//! Compatible GridMate deployments commonly use [`MAX_UDP_PACKET_SIZE = 1200`](MAX_UDP_PACKET_SIZE)
//! rather than Lumberyard's default 1400 — verified by Wireshark
//! capture chunk sizes.

use super::datagram::DatagramHeader;

/// Base UDP MTU.
pub const MAX_UDP_PACKET_SIZE: usize = 1200;

/// UDP + IP overhead (GridMate: `GetPacketOverheadSize = 8 + 20 = 28`).
pub const UDP_IP_OVERHEAD: usize = 28;

/// DTLS record header size (OpenSSL: `DTLS1_RT_HEADER_LENGTH = 13`).
pub const DTLS_HEADER_SIZE: usize = 13;

/// DTLS cipher overhead for AES-GCM (GridMate uses 30).
pub const DTLS_CIPHER_OVERHEAD: usize = 30;

/// Maximum datagram size after transport overhead.
///
/// GridMate: `m_maxDataGramSizeBytes = driver->GetMaxSendSize()`.
pub const MAX_DATAGRAM_SIZE: usize =
    MAX_UDP_PACKET_SIZE - UDP_IP_OVERHEAD - DTLS_HEADER_SIZE - DTLS_CIPHER_OVERHEAD;

/// Message header sizes (GridMate: `GetMaxMessageHeaderSize`).
pub mod header_sizes {
    pub const FLAGS: usize = 1; // u8
    pub const DATA_SIZE: usize = 2; // u16
    pub const CHANNEL_INFO: usize = 1; // u8
    pub const SPLIT_PACKET_INFO: usize = 2; // SequenceNumber (u16)
    pub const SEQUENCE_NUMBER: usize = 2; // SequenceNumber (u16)
    pub const SEQUENCE_RELIABLE_NUMBER: usize = 2; // SequenceNumber (u16)
}

/// Maximum message header size (all optional fields present).
///
/// GridMate: `GetMaxMessageHeaderSize()`.
pub const MAX_MESSAGE_HEADER_SIZE: usize = header_sizes::FLAGS
    + header_sizes::DATA_SIZE
    + header_sizes::CHANNEL_INFO
    + header_sizes::SPLIT_PACKET_INFO
    + header_sizes::SEQUENCE_NUMBER
    + header_sizes::SEQUENCE_RELIABLE_NUMBER;

/// Maximum message data size per chunk.
///
/// GridMate: `m_maxMsgDataSizeBytes = m_maxDataGramSizeBytes -
/// GetDataGramHeaderSize() - GetMaxMessageHeaderSize()`.
pub const MAX_MESSAGE_DATA_SIZE: usize =
    MAX_DATAGRAM_SIZE - DatagramHeader::SIZE - MAX_MESSAGE_HEADER_SIZE;

/// Half of sequence number space (used for chunk limit).
///
/// GridMate: `SequenceNumberHalfSpan = 32768 (0x8000)`.
pub const SEQUENCE_NUMBER_HALF_SPAN: usize = 32768;

/// Maximum number of chunks (GridMate: `maxNumChunks = SequenceNumberHalfSpan - 1`).
pub const MAX_NUM_CHUNKS: usize = SEQUENCE_NUMBER_HALF_SPAN - 1;

/// Calculate number of chunks needed for a message.
///
/// GridMate: `numChunks = 1 + ((dataSize - 1) / m_maxMsgDataSizeBytes)`.
#[inline]
pub const fn chunks_needed(data_size: usize) -> usize {
    if data_size <= MAX_MESSAGE_DATA_SIZE {
        1
    } else {
        1 + ((data_size - 1) / MAX_MESSAGE_DATA_SIZE)
    }
}
