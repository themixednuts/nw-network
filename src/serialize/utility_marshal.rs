// Utility marshalers: UUID as 16 bytes; Duration as u32 milliseconds

use super::{
    buffer::{ReadBuffer, WriteBuffer},
    error::MarshalerError,
    marshaler::Marshaler,
};
use uuid::Uuid;

/// "absent value" wire shape for u32 fields that don't have a separate
/// has-value byte.
///
/// Used at field sites via `#[marshal(as = "Sentinel")]` (e.g. throughout
/// `messages/contracts.rs`); the field stays typed as `Option<u32>` and
/// round-trips through `From<Option<u32>>` / `From<Sentinel>`.
///
/// Was previously `Sentinel<T>` with a single concrete `Sentinel<u32>`
/// instantiation; the generic parameter was unused polymorphism, since
/// `u32::MAX`-as-None is an integer-specific wire idiom that does not
/// generalize to other types.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct Sentinel(pub Option<u32>);

impl Sentinel {
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Sentinel(Some(value))
    }
}

impl From<Option<u32>> for Sentinel {
    fn from(value: Option<u32>) -> Self {
        Self(value)
    }
}

impl From<Sentinel> for Option<u32> {
    fn from(value: Sentinel) -> Self {
        value.0
    }
}

impl Marshaler for Sentinel {
    const MARSHAL_SIZE: usize = <u32 as Marshaler>::MARSHAL_SIZE;

    fn marshal(&self, wb: &mut WriteBuffer) {
        match self.0 {
            Some(value) => value.marshal(wb),
            None => u32::MAX.marshal(wb),
        }
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        match u32::unmarshal(rb)? {
            u32::MAX => Ok(Sentinel(None)),
            value => Ok(Sentinel(Some(value))),
        }
    }
}

// UUID marshaled as 16 raw bytes
impl Marshaler for Uuid {
    fn marshal(&self, wb: &mut WriteBuffer) {
        wb.write_bytes(self.as_bytes());
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let bytes = rb.read_bytes(16)?;
        Ok(Uuid::from_slice(bytes)?)
    }
}

/// fields.
///
/// Distinct from the default `Marshaler<u64>` impl, which encodes 8 bytes
/// in big-endian (carrier order). This base-state sequence is an outlier:
/// host-side `u64`, so on our LE x86 client the on-wire bytes are LE too.
/// Round-trip with [`Marshaler<u64>`]'s default would silently change the wire
/// shape from LE (server-emitted) to BE.
///
/// Use as the field type whenever a wire `u64` was previously decoded with
/// recipe into a single named type.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, derive_more::From, derive_more::Into,
)]
pub struct RawSequenceNumber(pub u64);

impl Marshaler for RawSequenceNumber {
    const MARSHAL_SIZE: usize = 8;

    fn marshal(&self, wb: &mut WriteBuffer) {
        wb.write_bytes(&self.0.to_ne_bytes());
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let bytes = rb.read_bytes(8)?;
        let mut raw = [0u8; 8];
        raw.copy_from_slice(bytes);
        Ok(RawSequenceNumber(u64::from_ne_bytes(raw)))
    }
}

/// 16-bit half-precision float wire encoding for an `f32` payload.
///
/// effect / replicated-field float slots where a full 32-bit IEEE 754 single is
/// overkill.
///
/// Use at field sites via `#[marshal(as = "HalfF32")]`; the field stays
/// typed as `f32` and round-trips through `From<f32>` / `From<HalfF32>`.
/// Wire shape is 2 raw bytes (carrier-endian, like every other integer in
/// this crate).
#[derive(Debug, Clone, Copy, Default, PartialEq, derive_more::From, derive_more::Into)]
pub struct HalfF32(pub f32);

impl Marshaler for HalfF32 {
    const MARSHAL_SIZE: usize = 2;

    fn marshal(&self, wb: &mut WriteBuffer) {
        wb.write_u16(half::f16::from_f32(self.0).to_bits());
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let bits = rb.read_u16()?;
        Ok(HalfF32(half::f16::from_bits(bits).to_f32()))
    }
}

// std::time::Duration as u32 milliseconds
impl Marshaler for std::time::Duration {
    const MARSHAL_SIZE: usize = <u32 as Marshaler>::MARSHAL_SIZE;

    fn marshal(&self, wb: &mut WriteBuffer) {
        let ms = self.as_millis().min(u128::from(u32::MAX));
        let ms = u32::try_from(ms).expect("duration milliseconds are clamped to u32::MAX");
        ms.marshal(wb);
    }
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let ms = u32::unmarshal(rb)?;
        Ok(std::time::Duration::from_millis(u64::from(ms)))
    }
}

/// Boxed marshaler — wire shape is identical to the inner `T`.
///
/// Lets recursive value types compose with the existing derives without
/// hand-written impls. `T` is required to be sized because
/// [`Marshaler::unmarshal`] returns `Result<T, _>`.
impl<T: Marshaler> Marshaler for Box<T> {
    const MARSHAL_SIZE: usize = T::MARSHAL_SIZE;

    fn marshal(&self, wb: &mut WriteBuffer) {
        (**self).marshal(wb);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        Ok(Box::new(T::unmarshal(rb)?))
    }
}

/// Fixed word-count bitset encoded as little-endian `u64` words.
///
/// Const-generic over the **word count** (`WORDS`), not the bit count,
/// because stable Rust can't yet compute `WORDS = (BITS + 63) / 64` at
/// trait-impl scope (requires `feature(generic_const_exprs)`). Pick
/// `WORDS = ceil(BITS / 64)` at the call site — e.g. `BitSet<1>` for a
/// `bitset<64>`, `BitSet<2>` for `bitset<128>`, etc.
///
/// Distinct wire shape from this crate's three other bit-pack helpers:
/// - [`super::flat_bitmask::FlatBitmask`] — 8 bits per `u8` byte, no
///   continuation; used by state-bundle outer descriptor masks.
/// - [`super::mask_chain::MaskChain`] — 7 bits per `u8`, bit-7 = continuation;
///   used by base-state sub-field masks.
/// - [`super::live_mask::read_live_mask_batches`] /
///   [`super::live_mask::write_live_mask_batches`] — 8 entries per `u8`
///   live-mask, then per-entry bodies; used by delta-style collections.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BitSet<const WORDS: usize>(pub [u64; WORDS]);

impl<const WORDS: usize> Default for BitSet<WORDS> {
    fn default() -> Self {
        Self([0; WORDS])
    }
}

impl<const WORDS: usize> BitSet<WORDS> {
    /// Total bit capacity (`WORDS * 64`).
    pub const BITS: usize = WORDS * 64;

    /// Read bit `index` (LSB-first within each word).
    ///
    /// per-bit accessor (`operator[]` is bounds-checked in debug only;
    /// our impl defaults to "absent" for safety).
    #[must_use]
    pub fn get(&self, index: usize) -> bool {
        let word_idx = index / 64;
        let bit_idx = index % 64;
        word_idx < WORDS && (self.0[word_idx] & (1u64 << bit_idx)) != 0
    }

    /// Set bit `index` to `value`. No-op if `index >= WORDS * 64`.
    pub fn set(&mut self, index: usize, value: bool) {
        let word_idx = index / 64;
        let bit_idx = index % 64;
        if word_idx >= WORDS {
            return;
        }
        if value {
            self.0[word_idx] |= 1u64 << bit_idx;
        } else {
            self.0[word_idx] &= !(1u64 << bit_idx);
        }
    }

    /// Count of set bits.
    #[must_use]
    pub fn count(&self) -> u32 {
        self.0.iter().map(|w| w.count_ones()).sum()
    }
}

impl<const WORDS: usize> Marshaler for BitSet<WORDS> {
    fn marshal(&self, wb: &mut WriteBuffer) {
        for &word in &self.0 {
            word.marshal(wb);
        }
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let mut words = [0u64; WORDS];
        for word in &mut words {
            *word = u64::unmarshal(rb)?;
        }
        Ok(Self(words))
    }
}

// Generic Option<T> marshaler
// Wire format: 0x00 = None, 0x01 = Some(T)
// If Some, follows with T's marshaled representation
impl<T: Marshaler> Marshaler for Option<T> {
    fn marshal(&self, wb: &mut WriteBuffer) {
        match self {
            Some(inner) => {
                1u8.marshal(wb); // 0x01 = Some
                inner.marshal(wb);
            }
            None => {
                0u8.marshal(wb); // 0x00 = None
            }
        }
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let discriminant = u8::unmarshal(rb)?;
        match discriminant {
            0 => Ok(None),
            1 => Ok(Some(T::unmarshal(rb)?)),
            other => Err(MarshalerError::InvalidDiscriminant { value: other }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serialize::buffer::CARRIER_ENDIAN;

    #[test]
    fn bitset_one_word_round_trip_carrier_endian() {
        let mut bs = BitSet::<1>::default();
        bs.set(0, true);
        bs.set(7, true);
        bs.set(63, true);
        assert_eq!(bs.count(), 3);

        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        bs.marshal(&mut wb);
        let bytes = wb.into_vec();
        assert_eq!(bytes.len(), 8);
        // Word value: bit 0 + bit 7 + bit 63 = 0x8000_0000_0000_0081.
        // Carrier endian = big-endian, so MSB first.
        assert_eq!(bytes, [0x80, 0, 0, 0, 0, 0, 0, 0x81]);

        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bytes);
        let decoded = BitSet::<1>::unmarshal(&mut rb).unwrap();
        assert_eq!(decoded, bs);
    }

    #[test]
    fn bitset_two_words_emits_16_bytes() {
        let mut bs = BitSet::<2>::default();
        bs.set(64, true);
        bs.set(127, true);

        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        bs.marshal(&mut wb);
        let bytes = wb.into_vec();
        assert_eq!(bytes.len(), 16);
        // First u64 = 0 (no bits 0..63 set), second u64 = bit 0 + bit 63
        // → 0x8000_0000_0000_0001 BE.
        assert_eq!(&bytes[0..8], &[0, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(&bytes[8..16], &[0x80, 0, 0, 0, 0, 0, 0, 0x01]);

        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bytes);
        let decoded = BitSet::<2>::unmarshal(&mut rb).unwrap();
        assert!(decoded.get(64));
        assert!(decoded.get(127));
        assert!(!decoded.get(0));
    }

    #[test]
    fn bitset_out_of_range_set_is_noop() {
        let mut bs = BitSet::<1>::default();
        bs.set(1000, true);
        assert_eq!(bs.count(), 0);
        assert!(!bs.get(1000));
    }
}
