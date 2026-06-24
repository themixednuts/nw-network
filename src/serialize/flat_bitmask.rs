//! `FlatBitmask` — 8-bit-per-byte field-presence mask, no continuation marker.
//!
//! Sibling to [`MaskChain`](super::mask_chain::MaskChain), which uses 7
//! field bits per byte plus a continuation bit and is self-delimiting on
//! read. `FlatBitmask` packs 8 field bits per byte and relies on the
//! caller to know the byte count externally — appropriate when the field
//! count is fixed at compile time.
//!
//! Used by state-bundle descriptor masks and by the
//! `marshal_replicated_fields!` macro for chunks whose field count is known
//! at compile time and whose mask does not need a continuation bit.

use super::buffer::WriteBuffer;

/// Field-presence mask: 8 fields per byte, no continuation bit.
///
/// Reading a `FlatBitmask` requires the caller to know the byte count;
/// there's no terminator. Writing emits exactly `dirty.len().div_ceil(8)`
/// bytes (or one zero byte for an empty `dirty` slice).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlatBitmask {
    bytes: Vec<u8>,
}

impl FlatBitmask {
    /// Field bits per byte (no continuation marker).
    pub const FIELDS_PER_BYTE: usize = 8;

    /// Build from a slice of field-presence flags. Empty input emits one
    #[must_use]
    pub fn from_dirty(dirty: &[bool]) -> Self {
        if dirty.is_empty() {
            return Self { bytes: vec![0] };
        }
        let byte_count = dirty.len().div_ceil(Self::FIELDS_PER_BYTE);
        let mut bytes = Vec::with_capacity(byte_count);
        for chunk_idx in 0..byte_count {
            let start = chunk_idx * Self::FIELDS_PER_BYTE;
            let end = (start + Self::FIELDS_PER_BYTE).min(dirty.len());
            let mut byte = 0u8;
            for (bit, &flag) in dirty[start..end].iter().enumerate() {
                if flag {
                    byte |= 1 << bit;
                }
            }
            bytes.push(byte);
        }
        Self { bytes }
    }

    /// Look up bit `field_index` in a raw byte slice, treating the slice
    /// as an 8-bit-per-byte flat mask. Out-of-range fields read as absent.
    ///
    /// `ReplicatedState` derive reads N descriptor mask bytes into a `Vec<u8>`
    /// and then queries `is_field_set` per descriptor index.
    #[must_use]
    pub fn is_field_set(masks: &[u8], field_index: usize) -> bool {
        let byte_idx = field_index / Self::FIELDS_PER_BYTE;
        let bit_idx = field_index % Self::FIELDS_PER_BYTE;
        masks
            .get(byte_idx)
            .is_some_and(|byte| (byte & (1 << bit_idx)) != 0)
    }

    /// Write the mask bytes verbatim. No length prefix — the byte count
    /// is implicit (the caller knows the field count).
    pub fn marshal(&self, wb: &mut WriteBuffer) {
        wb.write_bytes(&self.bytes);
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serialize::buffer::CARRIER_ENDIAN;

    #[test]
    fn empty_dirty_yields_one_zero_byte() {
        let mask = FlatBitmask::from_dirty(&[]);
        assert_eq!(mask.as_bytes(), &[0]);
    }

    #[test]
    fn from_dirty_packs_eight_per_byte() {
        // 9 fields → 2 bytes; no continuation.
        let dirty = [true, false, true, false, false, false, false, true, true];
        let mask = FlatBitmask::from_dirty(&dirty);
        assert_eq!(mask.as_bytes().len(), 2);
        assert_eq!(mask.as_bytes()[0], 0b1000_0101);
        assert_eq!(mask.as_bytes()[1], 0b0000_0001);
    }

    #[test]
    fn is_field_set_walks_per_eight() {
        let bytes = [0b1000_0101u8, 0b0000_0010u8];
        assert!(FlatBitmask::is_field_set(&bytes, 0));
        assert!(!FlatBitmask::is_field_set(&bytes, 1));
        assert!(FlatBitmask::is_field_set(&bytes, 2));
        assert!(FlatBitmask::is_field_set(&bytes, 7));
        assert!(!FlatBitmask::is_field_set(&bytes, 8));
        assert!(FlatBitmask::is_field_set(&bytes, 9));
        assert!(!FlatBitmask::is_field_set(&bytes, 16));
    }

    #[test]
    fn round_trip_through_marshal() {
        let dirty = [
            true, false, true, true, false, true, true, false, true, false, false,
        ];
        let original = FlatBitmask::from_dirty(&dirty);

        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        original.marshal(&mut wb);
        let bytes = wb.into_vec();
        assert_eq!(bytes, original.as_bytes());

        for (i, &flag) in dirty.iter().enumerate() {
            assert_eq!(FlatBitmask::is_field_set(&bytes, i), flag);
        }
    }
}
