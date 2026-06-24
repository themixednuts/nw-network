use crate::serialize::vlq::VlqU64;
use crate::serialize::{Marshaler, MarshalerError, ReadBuffer, WriteBuffer};

/// Hub-level replication sequence number.
///
/// Distinct from the carrier-frame sequence number, which is a wrapping `u16`
/// used for datagram ordering inside the transport.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SequenceNumber {
    /// The field has never been modified.
    #[default]
    Invalid,
    /// The field was set locally with no specific remote sequence.
    ValidNonSequence,
    /// A network-supplied sequence number.
    Seq(u64),
}

impl SequenceNumber {
    #[must_use]
    pub const fn is_valid(self) -> bool {
        !matches!(self, Self::Invalid)
    }

    /// Recover the inner `u64`, if any.
    #[must_use]
    pub const fn as_seq(self) -> Option<u64> {
        match self {
            Self::Seq(n) => Some(n),
            _ => None,
        }
    }

    const fn rank(self) -> u8 {
        match self {
            Self::Invalid => 0,
            Self::ValidNonSequence => 1,
            Self::Seq(_) => 2,
        }
    }
}

impl From<u64> for SequenceNumber {
    fn from(sequence: u64) -> Self {
        Self::Seq(sequence)
    }
}

impl From<VlqU64> for SequenceNumber {
    fn from(sequence: VlqU64) -> Self {
        Self::Seq(sequence.get())
    }
}

impl PartialOrd for SequenceNumber {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SequenceNumber {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (*self, *other) {
            (Self::Seq(a), Self::Seq(b)) => a.cmp(&b),
            (left, right) => left.rank().cmp(&right.rank()),
        }
    }
}

impl Marshaler for SequenceNumber {
    fn marshal(&self, wb: &mut WriteBuffer) {
        let raw = match *self {
            Self::Invalid => None,
            Self::ValidNonSequence => Some(VlqU64::new(0)),
            Self::Seq(sequence) => Some(VlqU64::new(sequence)),
        };
        raw.marshal(wb);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        match Option::<VlqU64>::unmarshal(rb)? {
            None => Ok(Self::Invalid),
            Some(sequence) if sequence == 0 => Ok(Self::ValidNonSequence),
            Some(sequence) => Ok(Self::Seq(sequence.get())),
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
}
