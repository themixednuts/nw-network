//!
//! Each byte holds 7 field-presence bits in positions 0–6; bit 7 is a
//! **continuation marker**. Set means "another byte follows in this
//! masks and inside `#[replicated_state]` group field masks.
//!
//! Before this module the same loop was hand-rolled at every callsite —
//! `loop { let m = rb.read_u8()?; masks.push(m); if (m & 0x80) == 0 { break; } }`
//! plus a private `mask_has(&masks, field) = masks[field/7] & (1 << field%7)`
//! lookup, both copy-pasted across `attribute.rs`, `chat.rs`,
//! `cooldown_timers.rs`, and `musical_performance_player.rs`.

use super::{
    buffer::{ReadBuffer, WriteBuffer},
    error::MarshalerError,
    marshaler::Marshaler,
};

/// Field-presence chain encoded as `u8` bytes with continuation: bits 0–6
/// of each byte mark fields, bit 7 set means "another byte follows."
///
/// The empty / no-fields-set chain is one byte `0x00` (the terminator with
/// zero field bits). Iteration through `is_field_set` treats fields beyond
/// the chain as absent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaskChain {
    bytes: Vec<u8>,
}

impl Default for MaskChain {
    /// One-byte terminator chain. Matches the wire-required minimum so a
    /// `marshal()` on a freshly defaulted chain produces a valid empty
    /// chain (a single `0x00` byte).
    fn default() -> Self {
        Self::empty()
    }
}

impl MaskChain {
    /// Number of field bits per byte (bits 0..=6).
    pub const FIELDS_PER_BYTE: usize = 7;
    /// Bit position of the continuation marker.
    pub const CONTINUATION_BIT: u8 = 0x80;

    /// Empty chain — one terminator byte with no field bits set.
    #[must_use]
    pub fn empty() -> Self {
        Self { bytes: vec![0] }
    }

    /// Build a chain from a slice of field-presence flags.
    ///
    /// Always emits at least one byte (the terminator). For `dirty.len() <= 7`
    /// the result is one byte; for longer slices, one byte per 7 fields with
    /// the continuation bit set on all but the last.
    #[must_use]
    pub fn from_dirty_fields(dirty: &[bool]) -> Self {
        if dirty.is_empty() {
            return Self::empty();
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
            if chunk_idx + 1 < byte_count {
                byte |= Self::CONTINUATION_BIT;
            }
            bytes.push(byte);
        }
        Self { bytes }
    }

    /// Check whether field `field_index` is marked present.
    ///
    /// Fields beyond the chain length read as absent.
    #[must_use]
    pub fn is_field_set(&self, field_index: usize) -> bool {
        let byte_idx = field_index / Self::FIELDS_PER_BYTE;
        let bit_idx = field_index % Self::FIELDS_PER_BYTE;
        self.bytes
            .get(byte_idx)
            .is_some_and(|byte| (byte & (1 << bit_idx)) != 0)
    }

    /// True when all chain bytes have zero field bits set. Useful for
    /// "is this chain empty / does it carry any updates" guards.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes
            .iter()
            .all(|&b| (b & !Self::CONTINUATION_BIT) == 0)
    }

    /// Raw chain bytes (for cases that need to forward the wire payload
    /// verbatim — e.g. `PreservedBaseState` round-tripping).
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Read and discard a chain from `rb` without storing it. Matches the
    /// default "consume the empty mask chain" step for fragments that don't
    /// preserve their base state.
    ///
    /// # Errors
    ///
    /// Returns an error when the chain is truncated before its terminator byte.
    pub fn skip(rb: &mut ReadBuffer) -> Result<(), MarshalerError> {
        loop {
            let m = rb.read_u8()?;
            if (m & Self::CONTINUATION_BIT) == 0 {
                return Ok(());
            }
        }
    }
}

impl Marshaler for MaskChain {
    fn marshal(&self, wb: &mut WriteBuffer) {
        wb.write_bytes(&self.bytes);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let mut bytes = Vec::new();
        loop {
            let m = rb.read_u8()?;
            bytes.push(m);
            if (m & Self::CONTINUATION_BIT) == 0 {
                return Ok(Self { bytes });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serialize::buffer::CARRIER_ENDIAN;

    #[test]
    fn empty_chain_is_one_terminator_byte() {
        let chain = MaskChain::empty();
        assert_eq!(chain.as_bytes(), &[0]);
        assert!(chain.is_empty());
        assert!(!chain.is_field_set(0));
    }

    #[test]
    fn from_dirty_fields_packs_seven_per_byte() {
        // 9 fields → 2 bytes; bytes[0] has bit 7 set (continuation).
        let dirty = [true, false, true, false, false, false, false, true, true];
        let chain = MaskChain::from_dirty_fields(&dirty);
        assert_eq!(chain.as_bytes().len(), 2);
        // bits 0,2 in first byte + continuation bit
        assert_eq!(
            chain.as_bytes()[0],
            0b1000_0101 | MaskChain::CONTINUATION_BIT
        );
        // bit 0 (= field 7), bit 1 (= field 8) in second byte, no continuation
        assert_eq!(chain.as_bytes()[1], 0b0000_0011);
    }

    #[test]
    fn is_field_set_walks_per_seven() {
        let dirty = [false; 8];
        let mut dirty = dirty;
        dirty[0] = true;
        dirty[6] = true;
        dirty[7] = true; // straddles into second byte
        let chain = MaskChain::from_dirty_fields(&dirty);
        assert!(chain.is_field_set(0));
        assert!(!chain.is_field_set(1));
        assert!(chain.is_field_set(6));
        assert!(chain.is_field_set(7));
        assert!(!chain.is_field_set(99));
    }

    #[test]
    fn round_trip_through_marshaler() {
        let dirty = [
            true, false, true, true, false, true, false, true, false, true,
        ];
        let original = MaskChain::from_dirty_fields(&dirty);

        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        original.marshal(&mut wb);
        let bytes = wb.into_vec();

        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bytes);
        let decoded = MaskChain::unmarshal(&mut rb).unwrap();
        assert_eq!(rb.left(), 0);
        assert_eq!(decoded, original);
    }

    #[test]
    fn skip_consumes_chain_without_storing() {
        let dirty = [true; 15]; // 3 bytes
        let chain = MaskChain::from_dirty_fields(&dirty);

        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        chain.marshal(&mut wb);
        let bytes = wb.into_vec();
        assert_eq!(bytes.len(), 3);

        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bytes);
        MaskChain::skip(&mut rb).unwrap();
        assert_eq!(rb.left(), 0);
    }

    #[test]
    fn empty_dirty_slice_yields_terminator_only() {
        let chain = MaskChain::from_dirty_fields(&[]);
        assert_eq!(chain.as_bytes(), &[0]);
    }
}
