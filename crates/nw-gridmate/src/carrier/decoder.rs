//! Stateless decode of one inbound UDP datagram into its carrier frames.
//!
//! This is the **first half** of GridMate's receive pipeline (the
//! "datagram → frames" step in `Carrier.cpp::ProcessIncomingDataGram`).
//! It runs without per-peer state: every input is the raw datagram
//! bytes, every output is a [`DecodedDatagram`] carrying the header
//! plus the [`MessageData`] frames the payload contained. Lifecycle —
//! duplicate suppression, ACK history, retransmit timers — lives one
//! layer up in [`super::connection_state::ConnectionState`]; chunk
//! reassembly and reliable-ordering live in
//! [`super::reassembler::Reassembler`].
//!
//! Why factor it: the production carrier (`process_incoming_datagram`)
//! used to inline the whole pipeline against `&mut self`. Capture
//! tooling (`cap`, `extract_types`, future replay) had to either
//! reimplement the steps or build a half-fake `ConnectionState`. By
//! exposing the pure decode step as a free function with a single
//! consolidated error type, every consumer shares the same parser
//! semantics and the same error vocabulary.

use bytes::Bytes;

use super::datagram::DatagramHeader;
use super::message::MessageData;
use super::types::{MAX_CHANNELS, SequenceNumber};
use crate::serialize::buffer::{CARRIER_ENDIAN, ReadBuffer};
use crate::serialize::error::MarshalerError;
use thiserror::Error;

/// Result of decoding one UDP datagram.
///
/// Owns the (decompressed) payload via `Bytes` so the `frames` slices
/// remain valid for the caller's lifetime without copying. `frames`
/// contains every frame parsed from the payload — system and
/// application alike, no filtering. Caller decides how to route them
/// (typically: system frames handled inline by the peer state machine;
/// app frames pushed into a [`super::reassembler::Reassembler`]).
pub struct DecodedDatagram {
    /// `[seq_num u16][flags u16]` carrier datagram header.
    pub header: DatagramHeader,
    /// Decompressed payload (the bytes `frames` borrows from). Held so
    /// that the `MessageData::data` `Bytes` views into it stay live.
    pub payload: Bytes,
    /// Per-frame data extracted by [`MessageData::read_header`], in
    /// wire order. Frames on `MAX_CHANNELS..` indices are dropped here;
    /// the production receive path treats them as malformed.
    pub frames: Vec<MessageData>,
}

/// Failure modes for [`decode_datagram`]. Consolidates everything the
/// receive pipeline can complain about so callers get one error
/// vocabulary instead of three.
#[derive(Debug, Error)]
pub enum DecodeError {
    /// Bytes too short or shape wrong to form a [`DatagramHeader`].
    #[error("datagram header: {0}")]
    Header(#[source] MarshalerError),

    /// LZ4 decompress failed on a `0x81` compressed payload.
    #[error("lz4 decompress failed ({source}); payload {payload_len} bytes")]
    Decompress {
        payload_len: usize,
        #[source]
        source: lzzzz::Error,
    },

    /// Frame walk stopped before exhausting the payload. The leftover
    /// bytes are unparseable; callers usually log them and move on.
    #[error("frame parse stopped with {leftover} bytes remaining in payload of {payload_len}")]
    FrameParseStop { payload_len: usize, leftover: usize },
}

/// Maximum decompressed payload size. Matches the production receive
/// path's scratch buffer (`process_incoming_datagram` allocates 64 KiB
/// per call). UDP datagrams above this are rejected by the kernel,
/// and LZ4 expansion past 64 KiB would indicate a malformed compressed
/// frame anyway.
const MAX_DECOMPRESSED_BYTES: usize = 65536;

/// Decode one UDP datagram into its header + frames.
///
/// Pure: takes ownership of the input via `Bytes` so the returned
/// frames can borrow into the (possibly decompressed) payload
/// without copying. If the datagram is uncompressed, the payload is a
/// zero-copy slice of `data`; if compressed, it's a freshly-decompressed
/// `Bytes`.
///
/// Errors are consolidated under [`DecodeError`]; the production
/// `ConnectionState::process_incoming_datagram` and capture tooling
/// both use this same fn and pattern-match on the same variants.
pub fn decode_datagram(data: Bytes) -> Result<DecodedDatagram, DecodeError> {
    let mut header_rb = ReadBuffer::new(CARRIER_ENDIAN, data.as_ref());
    let header = DatagramHeader::unmarshal(&mut header_rb).map_err(DecodeError::Header)?;
    let payload_start = header_rb.position();

    let payload: Bytes = if header.is_compressed {
        let mut scratch = vec![0u8; MAX_DECOMPRESSED_BYTES];
        let payload_slice = &data[payload_start..];
        let n = lzzzz::lz4::decompress_partial(payload_slice, &mut scratch, MAX_DECOMPRESSED_BYTES)
            .map_err(|e| DecodeError::Decompress {
                payload_len: payload_slice.len(),
                source: e,
            })?;
        scratch.truncate(n);
        Bytes::from(scratch)
    } else {
        // Zero-copy: `Bytes::slice` only adjusts ref-counted bounds.
        data.slice(payload_start..)
    };

    #[cfg(debug_assertions)]
    let frames = {
        let mut frames = walk_frames(&payload);
        for frame in &mut frames {
            for span in &mut frame.wire_spans {
                span.datagram_sequence = Some(header.sequence_number);
            }
        }
        frames
    };
    #[cfg(not(debug_assertions))]
    let frames = walk_frames(&payload);
    Ok(DecodedDatagram {
        header,
        payload,
        frames,
    })
}

/// Walk every carrier frame in `payload`. Returns frames in wire
/// order. `MessageData::read_header` infers sequential IDs from a
/// scratch `[prev_seq, prev_rel_seq, current_channel]` triple — that
/// state is per-datagram (reset each call) so we keep it local here.
fn walk_frames(payload: &Bytes) -> Vec<MessageData> {
    let mut rb = ReadBuffer::new(CARRIER_ENDIAN, payload.as_ref());
    let mut prev_seq = [SequenceNumber::ZERO; MAX_CHANNELS];
    let mut prev_rel_seq = [SequenceNumber::ZERO; MAX_CHANNELS];
    let mut current_channel = 0u8;
    let mut frames = Vec::new();
    while !rb.is_empty() {
        let Some(msg) = MessageData::read_header(
            &mut rb,
            &mut prev_seq,
            &mut prev_rel_seq,
            &mut current_channel,
        ) else {
            break;
        };
        frames.push(msg);
    }
    frames
}
