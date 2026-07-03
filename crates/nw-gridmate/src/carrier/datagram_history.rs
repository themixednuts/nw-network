//! ACK history tracking for the carrier protocol.
//!
//! [`DatagramHistoryList`] is a direct port of Lumberyard's
//! `GridMate::DataGramHistoryList`: a slot-addressed circular buffer
//! that remembers the last N inbound datagrams so the connection
//! can ack them on the next outbound packet (single ack, continuous
//! range, or sparse bitmap). The peer's responder side runs the
//! mirror algorithm on [`apply_ack_data_to_send_queue`] to drop
//! acknowledged datagrams from its retransmit queue.
//!
//! The wire encoding of an ACK frame is defined by the
//! `ACK_FLAGS_*` bits inside [`read_ack_data_frame`] and
//! [`write_ack_history`]:
//!
//! ```text
//! 0x20  AHF_KEEP_ALIVE   — no datagrams to ack
//! 0x40  AHF_CONTINUOUS   — [last_to_ack][first_to_ack], range
//! 0x80  AHF_BITS         — [last_to_ack][history_bits...], sparse
//! ```

use super::datagram::DatagramData;
use super::types::SequenceNumber;
use crate::serialize::{ReadBuffer, WriteBuffer};
use std::collections::VecDeque;
use tracing::debug;

/// GridMate: `SequenceNumberSequentialDistance`.
///
/// Returns the sequential distance between two sequence numbers.
pub(super) fn sequence_number_sequential_distance(from: SequenceNumber, to: SequenceNumber) -> u16 {
    from.sequential_distance_to(to)
}

/// GridMate: `SequenceNumberLessThan` — wrapping-aware less-than.
///
/// Returns true if `a` is earlier than `b` in the sequence number
/// space (i.e., the forward distance from `a` to `b` is within the
/// half-span).
pub(super) fn seq_less_than(a: SequenceNumber, b: SequenceNumber) -> bool {
    let diff = b.get().wrapping_sub(a.get());
    a != b && diff > 0 && diff < 0x8000
}

/// History element (GridMate: `DataGramHistoryList::Element`).
#[derive(Debug, Clone, Copy)]
struct HistoryElement {
    sequence_number: SequenceNumber,
    /// `-1` if slot not used, otherwise count of ACKs sent
    /// (`0`, `1`, `2` → removed at `3`).
    num_acks_sent: i32,
}

/// Slot-addressed circular buffer of received datagram sequence
/// numbers. Each datagram is stored at
/// `sequence_number % MAX_ELEMENTS`. `first` and `last`
/// (one-past-end) delimit the active range, which may contain
/// invalid (`-1`) gaps.
pub struct DatagramHistoryList {
    elements: Vec<HistoryElement>,
    /// Index of the oldest active element.
    first: usize,
    /// One-past-end index (the slot AFTER the newest element).
    last: usize,
    num_active_elements: usize,
}

impl DatagramHistoryList {
    pub const MAX_NUM_ACKS: u8 = 3;
    pub const MAX_NUM_BYTES: u8 = 64;
    pub const MAX_ELEMENTS: usize = (Self::MAX_NUM_BYTES as usize) * 8; // 512

    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self {
            elements: vec![
                HistoryElement {
                    sequence_number: SequenceNumber::ZERO,
                    num_acks_sent: -1,
                };
                Self::MAX_ELEMENTS
            ],
            first: 0,
            last: 0,
            num_active_elements: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.num_active_elements
    }

    pub fn is_empty(&self) -> bool {
        self.num_active_elements == 0
    }

    pub fn is_full(&self) -> bool {
        self.first == self.last && self.num_active_elements != 0
    }

    /// Wrap-safe forward step.
    fn add(n: usize) -> usize {
        if n == Self::MAX_ELEMENTS - 1 {
            0
        } else {
            n + 1
        }
    }

    /// Wrap-safe backward step.
    fn sub(n: usize) -> usize {
        if n == 0 {
            Self::MAX_ELEMENTS - 1
        } else {
            n - 1
        }
    }

    /// Increment an offset in-place with wrapping.
    pub fn inc(n: &mut usize) {
        *n = Self::add(*n);
    }

    /// Decrement an offset in-place with wrapping.
    pub fn dec(n: &mut usize) {
        *n = Self::sub(*n);
    }

    /// Index of the actual last valid element = `sub(last)`.
    pub fn last_index(&self) -> usize {
        Self::sub(self.last)
    }

    pub fn begin(&self) -> usize {
        self.first
    }

    pub fn end(&self) -> usize {
        self.last
    }

    pub fn is_valid(&self, offset: usize) -> bool {
        self.elements[offset].num_acks_sent != -1
    }

    pub fn at(&self, offset: usize) -> SequenceNumber {
        self.elements[offset].sequence_number
    }

    /// First sequence number (only valid when non-empty).
    pub fn first(&self) -> Option<SequenceNumber> {
        if self.is_empty() {
            None
        } else {
            Some(self.elements[self.first].sequence_number)
        }
    }

    /// Last sequence number (only valid when non-empty).
    pub fn last(&self) -> Option<SequenceNumber> {
        if self.is_empty() {
            None
        } else {
            Some(self.elements[self.last_index()].sequence_number)
        }
    }

    /// Get a datagram for ACK, incrementing its send count and
    /// removing it when the count reaches [`Self::MAX_NUM_ACKS`].
    /// Returns the sequence number and the next iteration offset.
    ///
    /// Direct port of GridMate `DataGramHistoryList::GetForAck`.
    pub fn get_for_ack(&mut self, offset: usize) -> (SequenceNumber, usize) {
        let seq = self.elements[offset].sequence_number;
        self.elements[offset].num_acks_sent += 1;
        if self.elements[offset].num_acks_sent >= Self::MAX_NUM_ACKS as i32 {
            let next = self.remove(offset);
            (seq, next)
        } else {
            (seq, Self::add(offset))
        }
    }

    /// Remove the element at `offset`, returning the next valid offset.
    ///
    /// Direct port of GridMate `DataGramHistoryList::Remove`.
    fn remove(&mut self, offset: usize) -> usize {
        if self.elements[offset].num_acks_sent != -1 {
            self.num_active_elements -= 1;
            self.elements[offset].num_acks_sent = -1;
        }
        if self.num_active_elements == 0 {
            self.first = self.last;
            return self.last;
        }
        if self.first == offset {
            // Advance first past invalid slots
            while self.elements[self.first].num_acks_sent == -1 {
                Self::inc(&mut self.first);
            }
            return self.first;
        }
        if self.last_index() == offset {
            // Retreat last past invalid slots
            while self.elements[self.last_index()].num_acks_sent == -1 {
                Self::dec(&mut self.last);
            }
            return self.last;
        }
        Self::add(offset)
    }

    /// Insert a datagram into the history. Returns false if already
    /// present.
    ///
    /// Direct port of GridMate `DataGramHistoryList::Insert`.
    pub fn insert(&mut self, id: SequenceNumber) -> bool {
        let offset = id.get() as usize % Self::MAX_ELEMENTS;

        if self.num_active_elements != 0 {
            let last_seq = self.elements[self.last_index()].sequence_number;
            if seq_less_than(last_seq, id) {
                // id is newer than current last — append. Evict old.
                let new_first_seq =
                    SequenceNumber::from(id.get().wrapping_sub(Self::MAX_ELEMENTS as u16 - 1));
                while self.num_active_elements != 0
                    && seq_less_than(self.elements[self.first].sequence_number, new_first_seq)
                {
                    if self.elements[self.first].num_acks_sent != -1 {
                        self.num_active_elements -= 1;
                        self.elements[self.first].num_acks_sent = -1;
                    }
                    Self::inc(&mut self.first);
                }

                let new_last = Self::add(offset);
                if self.num_active_elements == 0 {
                    self.first = offset;
                    self.last = new_last;
                } else {
                    while self.last != new_last {
                        Self::inc(&mut self.last);
                    }
                }
                self.num_active_elements += 1;
                let li = self.last_index();
                self.elements[li].sequence_number = id;
                self.elements[li].num_acks_sent = 0;
            } else {
                // id is within existing range — check for duplicate
                if self.elements[offset].sequence_number == id
                    && self.elements[offset].num_acks_sent != -1
                {
                    return false; // duplicate
                }
                if self.elements[offset].num_acks_sent == -1 {
                    self.num_active_elements += 1;
                }
                self.elements[offset].sequence_number = id;
                self.elements[offset].num_acks_sent = 0;
            }
        } else {
            self.first = offset;
            self.last = Self::add(offset);
            self.num_active_elements = 1;
            self.elements[offset].sequence_number = id;
            self.elements[offset].num_acks_sent = 0;
        }
        true
    }
}

/// Parsed ACK frame body. `history_bits` is a zero-copy borrow
/// out of the inbound `ReadBuffer` — capped at
/// [`DatagramHistoryList::MAX_NUM_BYTES`] (64) by the wire format
/// but every byte avoided is per-datagram.
///
/// Parsed and consumed inside one synchronous call to
/// [`super::connection_state::ConnectionState::read_ack_data`] — no
/// await point sits between read and apply, so the borrow stays live.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum AckData<'a> {
    KeepAlive,
    Range {
        first_to_ack: SequenceNumber,
        last_to_ack: SequenceNumber,
    },
    Bits {
        last_to_ack: SequenceNumber,
        history_bits: &'a [u8],
    },
}

pub(super) fn read_ack_data_frame<'a>(read_buffer: &mut ReadBuffer<'a>) -> Option<AckData<'a>> {
    let ack_flags = read_buffer.read_u8().ok()?;

    if (ack_flags & 0x80) != 0 {
        let last_to_ack = SequenceNumber::from(read_buffer.read_u16().ok()?);
        let num_history_bytes = (ack_flags & 0x7f) as usize;
        if num_history_bytes > DatagramHistoryList::MAX_NUM_BYTES as usize {
            return None;
        }
        let history_bits = read_buffer.read_bytes(num_history_bytes).ok()?;
        Some(AckData::Bits {
            last_to_ack,
            history_bits,
        })
    } else if (ack_flags & 0x40) != 0 {
        let last_to_ack = SequenceNumber::from(read_buffer.read_u16().ok()?);
        let first_to_ack = SequenceNumber::from(read_buffer.read_u16().ok()?);
        Some(AckData::Range {
            first_to_ack,
            last_to_ack,
        })
    } else if (ack_flags & 0x20) != 0 {
        Some(AckData::KeepAlive)
    } else {
        let last_to_ack = SequenceNumber::from(read_buffer.read_u16().ok()?);
        Some(AckData::Range {
            first_to_ack: last_to_ack,
            last_to_ack,
        })
    }
}

pub(super) fn write_ack_history(
    received_history: &mut DatagramHistoryList,
    write_buffer: &mut WriteBuffer,
) -> Option<SequenceNumber> {
    let mut ack_flags: u8 = 0;
    let num_datagrams_to_ack = received_history.len();

    if num_datagrams_to_ack == 0 {
        ack_flags |= 0x20; // AHF_KEEP_ALIVE
        write_buffer.write_u8(ack_flags);
        return None;
    }

    debug!(
        "write_ack_data: len={} first_slot={} last_slot={}",
        num_datagrams_to_ack,
        received_history.begin(),
        received_history.end(),
    );

    let mut history_index = received_history.begin();
    let first_to_ack = received_history.at(history_index);
    let (last_to_ack, _) = received_history.get_for_ack(received_history.last_index());
    let dist = sequence_number_sequential_distance(first_to_ack, last_to_ack);
    let mut history_bits = Vec::new();

    if dist == 0 {
        debug!("ACK: single seq={}", last_to_ack.get());
    } else if dist == (num_datagrams_to_ack - 1) as u16 {
        ack_flags |= 0x40; // AHF_CONTINUOUS_ACK
        debug!(
            "ACK: continuous first={} last={} count={}",
            first_to_ack.get(),
            last_to_ack.get(),
            num_datagrams_to_ack,
        );

        loop {
            if history_index == received_history.last_index()
                || history_index == received_history.end()
            {
                break;
            }
            if received_history.is_valid(history_index) {
                let (_, next) = received_history.get_for_ack(history_index);
                history_index = next;
            } else {
                DatagramHistoryList::inc(&mut history_index);
            }
        }
    } else {
        let num_history_bytes = (dist as usize)
            .div_ceil(8)
            .min(DatagramHistoryList::MAX_NUM_BYTES as usize);
        ack_flags |= 0x80; // AHF_BITS
        ack_flags |= num_history_bytes as u8;
        history_bits.resize(num_history_bytes, 0);

        debug!(
            "ACK: bitmap first={} last={} count={} dist={} bytes={}",
            first_to_ack.get(),
            last_to_ack.get(),
            num_datagrams_to_ack,
            dist,
            num_history_bytes,
        );

        loop {
            if history_index == received_history.last_index()
                || history_index == received_history.end()
            {
                break;
            }
            if received_history.is_valid(history_index) {
                let (current_to_ack, next) = received_history.get_for_ack(history_index);
                let bit_distance = sequence_number_sequential_distance(current_to_ack, last_to_ack);
                if bit_distance > 0 {
                    let bit_index = (bit_distance - 1) as usize;
                    let byte_index = bit_index / 8;
                    if byte_index < history_bits.len() {
                        history_bits[byte_index] |= 1 << (bit_index % 8);
                    }
                }
                history_index = next;
            } else {
                DatagramHistoryList::inc(&mut history_index);
            }
        }
    }

    write_buffer.write_u8(ack_flags);
    write_buffer.write_u16(last_to_ack.get());
    if (ack_flags & 0x80) != 0 {
        write_buffer.write_bytes(&history_bits);
    } else if (ack_flags & 0x40) != 0 {
        write_buffer.write_u16(first_to_ack.get());
    }

    Some(last_to_ack)
}

pub(super) fn apply_ack_data_to_send_queue(
    send_datagrams: &mut VecDeque<DatagramData>,
    ack_data: &AckData,
) -> Vec<SequenceNumber> {
    let mut nacked = Vec::new();

    match ack_data {
        AckData::KeepAlive => {}
        AckData::Range {
            first_to_ack,
            last_to_ack,
        } => {
            send_datagrams.retain(|dgram| {
                let seq = dgram.flow_control.sequence_number;
                let is_acked = if first_to_ack == last_to_ack {
                    seq == *last_to_ack
                } else {
                    seq == *first_to_ack
                        || seq == *last_to_ack
                        || (seq_less_than(*first_to_ack, seq) && seq_less_than(seq, *last_to_ack))
                };

                if is_acked {
                    false
                } else if seq_less_than(seq, *first_to_ack) {
                    nacked.push(seq);
                    true
                } else {
                    true
                }
            });
        }
        AckData::Bits {
            last_to_ack,
            history_bits,
        } => {
            let ack_num_history_bits = history_bits.len() * 8;
            send_datagrams.retain(|dgram| {
                let seq = dgram.flow_control.sequence_number;
                let is_acked = if seq == *last_to_ack {
                    true
                } else if seq_less_than(seq, *last_to_ack) {
                    let distance = sequence_number_sequential_distance(seq, *last_to_ack) as usize;
                    if distance == 0 || distance > ack_num_history_bits {
                        nacked.push(seq);
                        return true;
                    }
                    let bit_index = distance - 1;
                    let byte_index = bit_index / 8;
                    (history_bits[byte_index] & (1 << (bit_index % 8))) != 0
                } else {
                    false
                };

                if is_acked {
                    false
                } else {
                    if seq_less_than(seq, *last_to_ack) {
                        nacked.push(seq);
                    }
                    true
                }
            });
        }
    }

    nacked
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serialize::CARRIER_ENDIAN;

    fn send_queue(seqs: &[u16]) -> VecDeque<DatagramData> {
        seqs.iter()
            .copied()
            .map(|seq| DatagramData::new(SequenceNumber::from(seq)))
            .collect()
    }

    fn send_queue_ids(queue: &VecDeque<DatagramData>) -> Vec<u16> {
        queue
            .iter()
            .map(|dgram| dgram.flow_control.sequence_number.get())
            .collect()
    }

    #[test]
    fn write_ack_history_writes_keepalive_when_empty() {
        let mut history = DatagramHistoryList::new();
        let mut buffer = WriteBuffer::new(CARRIER_ENDIAN);

        assert_eq!(write_ack_history(&mut history, &mut buffer), None);
        assert_eq!(buffer.into_vec(), vec![0x20]);
    }

    #[test]
    fn write_ack_history_writes_single_before_removing_after_third_ack() {
        let mut history = DatagramHistoryList::new();
        history.insert(SequenceNumber::from(7));

        for _ in 0..3 {
            let mut buffer = WriteBuffer::new(CARRIER_ENDIAN);
            assert_eq!(
                write_ack_history(&mut history, &mut buffer),
                Some(SequenceNumber::from(7))
            );
            assert_eq!(buffer.into_vec(), vec![0x00, 0x00, 0x07]);
        }

        let mut buffer = WriteBuffer::new(CARRIER_ENDIAN);
        assert_eq!(write_ack_history(&mut history, &mut buffer), None);
        assert_eq!(buffer.into_vec(), vec![0x20]);
    }

    #[test]
    fn write_ack_history_writes_continuous_range() {
        let mut history = DatagramHistoryList::new();
        for seq in 10..=12 {
            history.insert(SequenceNumber::from(seq));
        }

        let mut buffer = WriteBuffer::new(CARRIER_ENDIAN);
        assert_eq!(
            write_ack_history(&mut history, &mut buffer),
            Some(SequenceNumber::from(12))
        );
        assert_eq!(buffer.into_vec(), vec![0x40, 0x00, 0x0c, 0x00, 0x0a]);
    }

    #[test]
    fn write_ack_history_writes_bitmap_for_gapped_history() {
        let mut history = DatagramHistoryList::new();
        history.insert(SequenceNumber::from(10));
        history.insert(SequenceNumber::from(12));

        let mut buffer = WriteBuffer::new(CARRIER_ENDIAN);
        assert_eq!(
            write_ack_history(&mut history, &mut buffer),
            Some(SequenceNumber::from(12))
        );
        assert_eq!(buffer.into_vec(), vec![0x81, 0x00, 0x0c, 0x02]);
    }

    #[test]
    fn read_ack_data_frame_parses_bitmap() {
        let bytes = [0x81, 0x00, 0x0c, 0x02];
        let mut buffer = ReadBuffer::new(CARRIER_ENDIAN, &bytes);

        assert_eq!(
            read_ack_data_frame(&mut buffer),
            Some(AckData::Bits {
                last_to_ack: SequenceNumber::from(12),
                history_bits: &[0x02][..],
            })
        );
    }

    #[test]
    fn apply_range_ack_removes_range_and_nacks_older_datagrams() {
        let mut queue = send_queue(&[10, 11, 12, 13, 14]);
        let nacked = apply_ack_data_to_send_queue(
            &mut queue,
            &AckData::Range {
                first_to_ack: SequenceNumber::from(11),
                last_to_ack: SequenceNumber::from(13),
            },
        );

        assert_eq!(send_queue_ids(&queue), vec![10, 14]);
        assert_eq!(nacked, vec![SequenceNumber::from(10)]);
    }

    #[test]
    fn apply_bitmap_ack_removes_marked_bits_and_nacks_missing_bits() {
        let mut queue = send_queue(&[10, 11, 12, 13, 14]);
        let nacked = apply_ack_data_to_send_queue(
            &mut queue,
            &AckData::Bits {
                last_to_ack: SequenceNumber::from(13),
                history_bits: &[0b0000_0101][..],
            },
        );

        assert_eq!(send_queue_ids(&queue), vec![11, 14]);
        assert_eq!(nacked, vec![SequenceNumber::from(11)]);
    }
}
