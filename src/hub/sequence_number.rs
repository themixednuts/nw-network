use crate::serialize::vlq::VlqU64;
use crate::serialize::{Marshaler, MarshalerError, ReadBuffer, WriteBuffer};
use std::hash::{Hash, Hasher};

/// Hub-level replication sequence number.
///
/// Distinct from the carrier-frame sequence number, which is a wrapping `u16`
/// used for datagram ordering inside the transport.
///
/// The protocol models this as one `u64` value space. `u64::MAX` is the invalid
/// sentinel, `0` is the valid-but-not-a-sequence sentinel, and real sequence
/// values start at `1`. Raw-value conversions normalize those sentinel values
/// to [`Invalid`](Self::Invalid) and [`ValidNonSequence`](Self::ValidNonSequence)
/// instead of constructing `Seq(u64::MAX)` or `Seq(0)`.
///
/// The wire form is `Option<VLQ-u64>`:
/// `Invalid` is `None` (`0x00`), `ValidNonSequence` is `Some(0)`
/// (`0x01 0x00`), and `Seq(n)` is `Some(n)` (`0x01` followed by the VLQ-u64
/// sequence value).
#[derive(Debug, Clone, Copy, Default)]
pub enum SequenceNumber {
    /// The field has never been modified and marshals as `None`.
    #[default]
    Invalid,
    /// The field was set locally with no specific remote sequence and marshals as `Some(0)`.
    ValidNonSequence,
    /// A network-supplied sequence number marshaled as `Some(n)`.
    Seq(u64),
}

impl SequenceNumber {
    const INVALID_SEQUENCE_VALUE: u64 = u64::MAX;
    const VALID_NON_SEQUENCE_VALUE: u64 = 0;
    const STARTING_SEQUENCE_VALUE: u64 = 1;

    /// The first real sequence value.
    ///
    /// Fresh reliable and unreliable streams begin at this sequence.
    pub const STARTING_SEQUENCE: Self = Self::Seq(Self::STARTING_SEQUENCE_VALUE);

    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.raw_value() != Self::INVALID_SEQUENCE_VALUE
    }

    /// Recover the real sequence value, if any.
    #[must_use]
    pub const fn as_seq(self) -> Option<u64> {
        match self.raw_value() {
            Self::VALID_NON_SEQUENCE_VALUE | Self::INVALID_SEQUENCE_VALUE => None,
            sequence => Some(sequence),
        }
    }

    /// Return the next sequence number.
    ///
    /// Incrementing [`Invalid`](Self::Invalid) is a programming error. Debug
    /// builds assert on that case; release builds leave the value invalid.
    #[must_use]
    pub fn next(self) -> Self {
        let sequence = self.raw_value();
        if sequence == Self::INVALID_SEQUENCE_VALUE {
            debug_assert!(false, "attempting to increment an invalid SequenceNumber");
            return Self::Invalid;
        }

        Self::from_raw(sequence + 1)
    }

    const fn from_raw(sequence: u64) -> Self {
        match sequence {
            Self::VALID_NON_SEQUENCE_VALUE => Self::ValidNonSequence,
            Self::INVALID_SEQUENCE_VALUE => Self::Invalid,
            sequence => Self::Seq(sequence),
        }
    }

    const fn raw_value(self) -> u64 {
        match self {
            Self::Invalid => Self::INVALID_SEQUENCE_VALUE,
            Self::ValidNonSequence => Self::VALID_NON_SEQUENCE_VALUE,
            Self::Seq(sequence) => sequence,
        }
    }
}

impl From<u64> for SequenceNumber {
    fn from(sequence: u64) -> Self {
        Self::from_raw(sequence)
    }
}

impl From<VlqU64> for SequenceNumber {
    fn from(sequence: VlqU64) -> Self {
        Self::from(sequence.get())
    }
}

impl PartialEq for SequenceNumber {
    fn eq(&self, other: &Self) -> bool {
        self.raw_value() == other.raw_value()
    }
}

impl Eq for SequenceNumber {}

impl Hash for SequenceNumber {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.raw_value().hash(state);
    }
}

impl PartialOrd for SequenceNumber {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SequenceNumber {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let left = self.raw_value();
        let right = other.raw_value();

        match (
            left == Self::INVALID_SEQUENCE_VALUE,
            right == Self::INVALID_SEQUENCE_VALUE,
        ) {
            (true, true) => std::cmp::Ordering::Equal,
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            (false, false) => left.cmp(&right),
        }
    }
}

impl Marshaler for SequenceNumber {
    fn marshal(&self, wb: &mut WriteBuffer) {
        let raw = match Self::from_raw(self.raw_value()) {
            Self::Invalid => None,
            Self::ValidNonSequence => Some(VlqU64::new(0)),
            Self::Seq(sequence) => Some(VlqU64::new(sequence)),
        };
        raw.marshal(wb);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        match Option::<VlqU64>::unmarshal(rb)? {
            None => Ok(Self::Invalid),
            Some(sequence) => Ok(sequence.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serialize::buffer::CARRIER_ENDIAN;

    #[test]
    fn uses_vlq_u64_wire_shape() {
        let cases = [
            (SequenceNumber::Invalid, vec![0]),
            (SequenceNumber::ValidNonSequence, vec![1, 0]),
            (SequenceNumber::Seq(7), vec![1, 7]),
            (SequenceNumber::Seq(0x80), vec![1, 0x80, 0x02]),
        ];

        for (sequence, expected) in cases {
            let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
            sequence.marshal(&mut wb);
            assert_eq!(wb.as_slice(), expected.as_slice());

            let mut rb = ReadBuffer::new(CARRIER_ENDIAN, wb.as_slice());
            assert_eq!(SequenceNumber::unmarshal(&mut rb).unwrap(), sequence);
            assert_eq!(rb.left(), 0);
        }
    }

    #[test]
    fn exposes_starting_sequence() {
        assert_eq!(SequenceNumber::STARTING_SEQUENCE, SequenceNumber::Seq(1));
        assert_eq!(SequenceNumber::STARTING_SEQUENCE.as_seq(), Some(1));
    }

    #[test]
    fn next_advances_valid_sequences() {
        assert_eq!(
            SequenceNumber::ValidNonSequence.next(),
            SequenceNumber::STARTING_SEQUENCE
        );
        assert_eq!(SequenceNumber::Seq(1).next(), SequenceNumber::Seq(2));
    }

    #[test]
    fn next_rejects_invalid_sequences() {
        let result = std::panic::catch_unwind(|| SequenceNumber::Invalid.next());

        if cfg!(debug_assertions) {
            assert!(result.is_err());
        } else {
            assert_eq!(result.unwrap(), SequenceNumber::Invalid);
        }
    }

    #[test]
    fn from_raw_values_normalizes_sentinels() {
        assert_eq!(SequenceNumber::from(0), SequenceNumber::ValidNonSequence);
        assert_eq!(SequenceNumber::from(1), SequenceNumber::Seq(1));
        assert_eq!(SequenceNumber::from(u64::MAX), SequenceNumber::Invalid);
    }

    #[test]
    fn from_vlq_u64_normalizes_sentinels() {
        assert_eq!(
            SequenceNumber::from(VlqU64::new(0)),
            SequenceNumber::ValidNonSequence
        );
        assert_eq!(SequenceNumber::from(VlqU64::new(1)), SequenceNumber::Seq(1));
        assert_eq!(
            SequenceNumber::from(VlqU64::new(u64::MAX)),
            SequenceNumber::Invalid
        );
    }

    #[test]
    fn orders_sequence_numbers_by_protocol_semantics() {
        assert_eq!(
            SequenceNumber::Invalid.partial_cmp(&SequenceNumber::Invalid),
            Some(core::cmp::Ordering::Equal)
        );
        assert!(SequenceNumber::Invalid < SequenceNumber::ValidNonSequence);
        assert!(SequenceNumber::ValidNonSequence < SequenceNumber::Seq(1));
        assert!(SequenceNumber::Seq(1) < SequenceNumber::Seq(2));
    }

    #[test]
    fn direct_sentinel_shaped_sequences_use_raw_value_semantics() {
        assert_eq!(SequenceNumber::Seq(0), SequenceNumber::ValidNonSequence);
        assert_eq!(SequenceNumber::Seq(u64::MAX), SequenceNumber::Invalid);
        assert!(!SequenceNumber::Seq(u64::MAX).is_valid());
        assert_eq!(SequenceNumber::Seq(0).next(), SequenceNumber::Seq(1));
    }
}
