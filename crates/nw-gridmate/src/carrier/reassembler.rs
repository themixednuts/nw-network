//! Per-channel chunk reassembly + reliable-message ordering.
//!
//! Lives downstream of [`super::decoder::decode_datagram`] and upstream
//! of the application-level envelope parse (`SessionService::dispatch_messages`).
//! The contract is GridMate's `Carrier.cpp::ProcessReceivedMessages`
//! ordering — reliable messages must deliver in sequence on their
//! channel; unreliable messages on the same channel wait until the
//! latest reliable ID has been delivered.
//!
//! Used by both:
//! - the production peer state machine
//!   ([`super::connection_state::ConnectionState`]), which holds one
//!   `Reassembler` privately and layers its dedup + ACK + retransmit
//!   logic around it;
//! - external decode consumers ([`super::receiver::Receiver`], `cap`,
//!   capture-extractor tools), which want the same correctness
//!   guarantees without standing up a full peer connection.

use bytes::Bytes;
use std::collections::VecDeque;
use tracing::debug;

use super::datagram_history::sequence_number_sequential_distance;
#[cfg(debug_assertions)]
use super::message::MessageWireSpans;
use super::message::{DataReliability, MessageData};
use super::types::{MAX_CHANNELS, SEQUENCE_NUMBER_MAX, SequenceNumber};

/// Per-channel chunk reassembly + reliable-ordering for inbound
/// carrier frames. Push frames in arrival order via [`Self::accept`];
/// drain reassembled messages in delivery order via [`Self::drain`].
///
/// The struct owns four parallel arrays indexed by channel:
///
/// - `staging` — frames not yet delivered (waiting for a reliable
///   predecessor, or for chunks to complete a multi-chunk message).
/// - `received_reliable_seq_num` — the latest reliable sequence id
///   delivered on this channel. Initialised to
///   [`SEQUENCE_NUMBER_MAX`] which the ordering logic treats as
///   "nothing delivered yet" (so the first message in either
///   direction can flow regardless of its sequence id).
/// - `received_seq_num` — the latest sequence id (reliable or
///   unreliable) delivered on this channel. Stats-only today; the
///   ordering gate uses the reliable counter.
pub struct Reassembler {
    /// Per-channel staging queue: frames received but not yet ready
    /// to deliver to the application.
    staging: [VecDeque<MessageData>; MAX_CHANNELS],
    /// Last delivered reliable message sequence number, per channel.
    /// Drives the ordering gate in [`Self::drain_channel`].
    received_reliable_seq_num: [SequenceNumber; MAX_CHANNELS],
    /// Last delivered sequence number, per channel (reliable or not).
    /// Not used by the ordering gate; tracked to match GridMate's
    /// `m_receivedSeqNum` for stats / diagnostics parity.
    received_seq_num: [SequenceNumber; MAX_CHANNELS],
}

impl Default for Reassembler {
    fn default() -> Self {
        Self::new()
    }
}

impl Reassembler {
    /// Fresh reassembler. All channels start with
    /// `received_reliable_seq_num = SequenceNumberMax` to match
    /// GridMate's "nothing delivered yet" sentinel.
    pub fn new() -> Self {
        Self {
            staging: Default::default(),
            received_reliable_seq_num: [SEQUENCE_NUMBER_MAX; MAX_CHANNELS],
            received_seq_num: [SEQUENCE_NUMBER_MAX; MAX_CHANNELS],
        }
    }

    /// Push one frame into the staging queue for its channel. Frames
    /// on channels `>= MAX_CHANNELS` are dropped (the production
    /// receive path does the same — out-of-range channels are
    /// malformed).
    pub fn accept(&mut self, frame: MessageData) {
        let ch = frame.channel as usize;
        if ch < MAX_CHANNELS {
            self.staging[ch].push_back(frame);
        }
    }

    /// Drain every channel's ready messages in delivery order.
    /// Yields `(channel, message)` tuples; the iterator borrows
    /// `&mut self` so callers must collect or step through it before
    /// dropping the borrow.
    pub fn drain(&mut self) -> Drain<'_> {
        Drain {
            reassembler: self,
            channel: 0,
        }
    }

    /// Snapshot of the latest reliable sequence id delivered on
    /// `channel`. Mirrors `ConnectionState::received_reliable_seq_num`
    /// for telemetry / test harnesses that want to assert delivery
    /// order without poking private fields.
    pub fn received_reliable_seq_num(&self, channel: u8) -> SequenceNumber {
        self.received_reliable_seq_num[channel as usize]
    }

    /// Pop the next ready message on `channel`, applying the
    /// ordering gate and multi-chunk reassembly. Returns `None` when
    /// the channel has no deliverable message right now (either
    /// empty, blocked on a reliable predecessor, or waiting for more
    /// chunks).
    fn next_ready(&mut self, channel: usize) -> Option<MessageData> {
        // Peek the head and decide whether it's deliverable.
        let head = self.staging[channel].front()?;
        let head_reliable = head.reliability == DataReliability::Reliable;
        let head_send_rel = head.send_reliable_seq_num;
        let head_num_chunks = head.num_chunks.get() as usize;

        let last_rel = self.received_reliable_seq_num[channel];
        let nothing_delivered_yet = last_rel == SEQUENCE_NUMBER_MAX;

        if head_reliable {
            // Reliable: must be the next sequential reliable id, and
            // for multi-chunk messages every chunk must already be
            // queued with sequential reliable ids behind it.
            let dist = sequence_number_sequential_distance(last_rel, head_send_rel);
            if dist != 1 {
                return None;
            }
            if head_num_chunks > 1 {
                if self.staging[channel].len() < head_num_chunks {
                    return None;
                }
                if !self.chunks_ready(channel, head_num_chunks) {
                    return None;
                }
                return self.assemble_chunks(channel, head_num_chunks);
            }
        } else {
            // Unreliable: wait until the latest reliable predecessor
            // has been delivered. The special-case `nothing_delivered_yet`
            // unblocks the first message on a freshly-opened channel
            // (handles SM_CONNECT_ACK and friends).
            if !nothing_delivered_yet {
                let dist = sequence_number_sequential_distance(last_rel, head_send_rel);
                if dist > 0 {
                    return None;
                }
            }
        }

        // Single-frame delivery: pop and update the trailing seq ids.
        let msg = self.staging[channel].pop_front()?;
        if msg.reliability == DataReliability::Reliable {
            self.received_reliable_seq_num[channel] = msg.send_reliable_seq_num;
        }
        self.received_seq_num[channel] = msg.sequence_number;
        Some(msg)
    }

    /// Pre-flight check: the next `num_chunks` frames in `staging` are
    /// all reliable and carry sequential reliable ids. Mirrors
    /// `ConnectionState::verify_chunks_ready`.
    fn chunks_ready(&self, channel: usize, num_chunks: usize) -> bool {
        if self.staging[channel].len() < num_chunks {
            return false;
        }
        let mut iter = self.staging[channel].iter();
        let Some(first) = iter.next() else {
            return false;
        };
        if first.reliability != DataReliability::Reliable {
            return false;
        }
        let mut prev_rel_seq = first.send_reliable_seq_num;
        for chunk in iter.take(num_chunks - 1) {
            if chunk.reliability != DataReliability::Reliable {
                return false;
            }
            let dist =
                sequence_number_sequential_distance(prev_rel_seq, chunk.send_reliable_seq_num);
            if dist != 1 {
                return false;
            }
            prev_rel_seq = chunk.send_reliable_seq_num;
        }
        true
    }

    /// Pop `num_chunks` frames from `staging[channel]` and concatenate
    /// their `data` into one reassembled [`MessageData`]. Updates the
    /// channel's trailing seq counters from the *last* chunk so the
    /// gate moves forward.
    fn assemble_chunks(&mut self, channel: usize, num_chunks: usize) -> Option<MessageData> {
        let total_size: usize = self.staging[channel]
            .iter()
            .take(num_chunks)
            .map(|m| m.data.len())
            .sum();
        debug!(
            "[REASSEMBLE] channel={} chunks={} total={}",
            channel, num_chunks, total_size
        );
        let mut data = Vec::with_capacity(total_size);
        #[cfg(debug_assertions)]
        let mut wire_spans = MessageWireSpans::new();
        let mut last_seq = SequenceNumber::ZERO;
        let mut last_rel = SequenceNumber::ZERO;
        for _ in 0..num_chunks {
            let chunk = self.staging[channel].pop_front()?;
            data.extend_from_slice(&chunk.data);
            #[cfg(debug_assertions)]
            wire_spans.extend(chunk.wire_spans);
            last_seq = chunk.sequence_number;
            last_rel = chunk.send_reliable_seq_num;
        }
        self.received_reliable_seq_num[channel] = last_rel;
        self.received_seq_num[channel] = last_seq;

        let mut msg = MessageData::new();
        msg.channel = channel as u8;
        msg.reliability = DataReliability::Reliable;
        msg.num_chunks = SequenceNumber::from(1);
        msg.sequence_number = last_seq;
        msg.send_reliable_seq_num = last_rel;
        msg.data = Bytes::from(data);
        #[cfg(debug_assertions)]
        {
            msg.wire_spans = wire_spans;
        }
        Some(msg)
    }
}

/// Iterator returned by [`Reassembler::drain`]. Walks channels in
/// numerical order and yields each channel's ready messages until
/// none can be delivered, then advances to the next channel.
pub struct Drain<'a> {
    reassembler: &'a mut Reassembler,
    channel: usize,
}

impl<'a> Iterator for Drain<'a> {
    type Item = (u8, MessageData);

    fn next(&mut self) -> Option<Self::Item> {
        while self.channel < MAX_CHANNELS {
            if let Some(msg) = self.reassembler.next_ready(self.channel) {
                return Some((self.channel as u8, msg));
            }
            self.channel += 1;
        }
        None
    }
}
