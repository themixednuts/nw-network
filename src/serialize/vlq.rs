use derive_more::{AsRef, Deref, DerefMut, Display, From, Into};

use super::{
    buffer::{ReadBuffer, WriteBuffer},
    error::MarshalerError,
    marshaler::{Codec, Marshaler},
};

#[inline]
fn byte(value: u64) -> u8 {
    u8::try_from(value & 0xff).expect("value is masked to one byte")
}

/// Encodes a `u16` in 1..=3 bytes using the crate's VLQ format. The wire
/// shape is a `VlqU32` value bounded to `u16::MAX`; the dedicated codec
/// sites that carry a `u16` field on the wire don't have to re-derive the
/// pattern every time.
#[derive(Debug, Clone, Copy, Default)]
pub struct VlqU16Marshaler;

impl VlqU16Marshaler {
    #[inline]
    pub fn marshal(&self, wb: &mut WriteBuffer, v: u16) {
        let v32 = u32::from(v);
        let mut data = [0u8; 3];
        if v < 0x80 {
            data[0] = byte(u64::from(v));
            wb.write_bytes(&data[..1]);
        } else if v < 0x4000 {
            data[0] = 0x80 | byte(u64::from(v32 & 0x3f));
            data[1] = byte(u64::from((v32 & 0x3fc0) >> 6));
            wb.write_bytes(&data[..2]);
        } else {
            data[0] = 0xc0 | byte(u64::from(v32 & 0x1f));
            data[1] = byte(u64::from((v32 & 0x1f_e0) >> 5));
            data[2] = byte(u64::from((v32 & 0x1f_e000) >> 13));
            wb.write_bytes(&data[..3]);
        }
    }

    #[inline]
    /// Decode a VLQ value bounded to `u16`.
    ///
    /// # Errors
    ///
    /// Returns an error when the encoded value exceeds `u16::MAX` or the buffer is truncated.
    pub fn unmarshal(&self, rb: &mut ReadBuffer) -> Result<u16, MarshalerError> {
        let value = VlqU32Marshaler.unmarshal(rb)?;
        if value > u32::from(u16::MAX) {
            return Err(MarshalerError::ContainerOverflow {
                len: usize::try_from(value).unwrap_or(usize::MAX),
                capacity: usize::from(u16::MAX),
            });
        }
        u16::try_from(value).map_err(|_| MarshalerError::ContainerOverflow {
            len: usize::try_from(value).unwrap_or(usize::MAX),
            capacity: usize::from(u16::MAX),
        })
    }
}

impl Codec<u16> for VlqU16Marshaler {
    fn marshal(value: &u16, wb: &mut WriteBuffer) {
        VlqU16Marshaler.marshal(wb, *value);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<u16, MarshalerError> {
        VlqU16Marshaler.unmarshal(rb)
    }
}

/// Newtype wrapper around `u16` whose [`Marshaler`] impl emits the
/// VLQ encoding bounded to `u16::MAX`. Use as the field type when the wire
/// carries a logical u16 as a VLQ — a type swap (`pub count: VlqU16`) is
/// enough to get the correct wire shape and the bound check.
///
/// Mirrors [`VlqU32`] / [`VlqU64`] ergonomics: `Deref<Target = u16>`,
/// `From<u16>`, `Into<u16>`, `PartialEq<u16>`, `Display`, `AsRef<u16>`.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    AsRef,
    Deref,
    DerefMut,
    Display,
    From,
    Into,
)]
pub struct VlqU16(pub u16);

impl VlqU16 {
    /// Construct from a raw `u16`. Same as `VlqU16::from(value)` but `const`.
    #[inline]
    #[must_use]
    pub const fn new(value: u16) -> Self {
        VlqU16(value)
    }

    /// Extract the inner `u16`. Same as `u16::from(value)` but `const`.
    #[inline]
    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
}

impl PartialEq<u16> for VlqU16 {
    #[inline]
    fn eq(&self, other: &u16) -> bool {
        self.0 == *other
    }
}

impl PartialEq<VlqU16> for u16 {
    #[inline]
    fn eq(&self, other: &VlqU16) -> bool {
        *self == other.0
    }
}

impl Marshaler for VlqU16 {
    fn marshal(&self, wb: &mut WriteBuffer) {
        VlqU16Marshaler.marshal(wb, self.0);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        Ok(VlqU16(VlqU16Marshaler.unmarshal(rb)?))
    }
}

/// Encodes a `u32` in 1..=5 bytes using the crate's VLQ format.
#[derive(Debug, Clone, Copy, Default)]
pub struct VlqU32Marshaler;

impl VlqU32Marshaler {
    pub const MAX_ENCODING_BYTES: usize = 5;

    #[inline]
    pub fn marshal(&self, wb: &mut WriteBuffer, v: u32) {
        let mut data = [0u8; 5];
        if v < 0x80 {
            data[0] = byte(u64::from(v));
            wb.write_bytes(&data[..1]);
        } else if v < 0x4000 {
            data[0] = 0x80 | byte(u64::from(v & 0x3f));
            data[1] = byte(u64::from((v & 0x3fc0) >> 6));
            wb.write_bytes(&data[..2]);
        } else if v < 0x20_0000 {
            data[0] = 0xc0 | byte(u64::from(v & 0x1f));
            data[1] = byte(u64::from((v & 0x1f_e0) >> 5));
            data[2] = byte(u64::from((v & 0x1f_e000) >> 13));
            wb.write_bytes(&data[..3]);
        } else if v < 0x1000_0000 {
            data[0] = 0xe0 | byte(u64::from(v & 0x0f));
            data[1] = byte(u64::from((v & 0x0000_0ff0) >> 4));
            data[2] = byte(u64::from((v & 0x000f_f000) >> 12));
            data[3] = byte(u64::from((v & 0x0ff0_0000) >> 20));
            wb.write_bytes(&data[..4]);
        } else {
            data[0] = 0xf0 | byte(u64::from(v & 0x07));
            data[1] = byte(u64::from((v & 0x0000_07f8) >> 3));
            data[2] = byte(u64::from((v & 0x0007_f800) >> 11));
            data[3] = byte(u64::from((v & 0x07f8_0000) >> 19));
            data[4] = byte(u64::from((v & 0xf800_0000) >> 27));
            wb.write_bytes(&data[..5]);
        }
    }

    #[inline]
    /// Decode a `u32` VLQ.
    ///
    /// # Errors
    ///
    /// Returns an error when the buffer is truncated.
    pub fn unmarshal(&self, rb: &mut ReadBuffer) -> Result<u32, MarshalerError> {
        let first = rb.read_u8()?;
        if first < 0x80 {
            Ok(u32::from(first))
        } else if first < 0xc0 {
            let b1 = rb.read_u8()?;
            let v = (u32::from(first & !0xc0)) | ((u32::from(b1)) << 6);
            Ok(v)
        } else if first < 0xe0 {
            let b1 = rb.read_u8()?;
            let b2 = rb.read_u8()?;
            let v = (u32::from(first & !0xe0)) | ((u32::from(b1)) << 5) | ((u32::from(b2)) << 13);
            Ok(v)
        } else if first < 0xf0 {
            let b1 = rb.read_u8()?;
            let b2 = rb.read_u8()?;
            let b3 = rb.read_u8()?;
            let v = (u32::from(first & !0xf0))
                | ((u32::from(b1)) << 4)
                | ((u32::from(b2)) << 12)
                | ((u32::from(b3)) << 20);
            Ok(v)
        } else {
            let b1 = rb.read_u8()?;
            let b2 = rb.read_u8()?;
            let b3 = rb.read_u8()?;
            let b4 = rb.read_u8()?;
            let v = (u32::from(first & !0xf8))
                | ((u32::from(b1)) << 3)
                | ((u32::from(b2)) << 11)
                | ((u32::from(b3)) << 19)
                | ((u32::from(b4)) << 27);
            Ok(v)
        }
    }
}

/// `VlqU32Marshaler` doubles as a [`Codec<u32>`] policy. Field sites that
/// hold a raw `u32` but encode as a VLQ on the wire compose without a
/// wrapper:
///
/// ```ignore
/// pub ranged_attack_index: ReplicatedFieldHandler<u32, VlqU32Marshaler>,
/// ```
///
/// Same idiom as [`HalfF32Marshaler`](super::replicated_field::HalfF32Marshaler) and
/// [`Vec3CompMarshaler`](super::compression_marshal::Vec3CompMarshaler):
/// one zero-overhead policy type for an alternate per-field wire shape.
///
/// [`Codec<u32>`]: super::marshaler::Codec
impl super::marshaler::Codec<u32> for VlqU32Marshaler {
    fn marshal(value: &u32, wb: &mut WriteBuffer) {
        VlqU32Marshaler.marshal(wb, *value);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<u32, MarshalerError> {
        VlqU32Marshaler.unmarshal(rb)
    }
}

/// Newtype wrapper around `u32` whose [`Marshaler`] impl emits the
/// 1..=5-byte VLQ encoding (the same encoding `Vec<T>::Marshaler` uses for
/// its length prefix). Use this *as the field type* — e.g.
/// `pub seq: VlqU32` — whenever the wire format is a standalone VLQ rather
/// than the default 4-byte big-endian `Marshaler<u32>` write.
///
/// (`From<u32>`, `From<VlqU32> for u32`, `PartialEq<u32>`, `Display`,
/// `AsRef<u32>`, `From<&VlqU32> for VlqU32`) let the wrapper drop into
/// existing formatting and conversion sites without ceremony — and
/// `*v` (deref) yields a `u32` for arithmetic. A field carrying a logical
/// `u32` only needs the type swap to gain the right wire shape.
///
/// ```
/// use nw_network::serialize::VlqU32;
/// let v: VlqU32 = 42_u32.into();
/// assert_eq!(*v, 42);                 // Deref to u32 for arithmetic
/// assert_eq!(*v + 1, 43);
/// assert_eq!(v, 42_u32);              // PartialEq<u32>
/// assert_eq!(format!("{v}"), "42");   // Display passthrough
/// let raw: u32 = v.into();            // From<VlqU32> for u32
/// assert_eq!(raw, 42);
/// ```
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    AsRef,
    Deref,
    DerefMut,
    Display,
    From,
    Into,
)]
pub struct VlqU32(pub u32);

impl VlqU32 {
    /// Construct from a raw `u32`. Same as `VlqU32::from(value)` but `const`.
    #[inline]
    #[must_use]
    pub const fn new(value: u32) -> Self {
        VlqU32(value)
    }

    /// Extract the inner `u32`. Same as `u32::from(value)` but `const`.
    #[inline]
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl PartialEq<u32> for VlqU32 {
    #[inline]
    fn eq(&self, other: &u32) -> bool {
        self.0 == *other
    }
}

impl PartialEq<VlqU32> for u32 {
    #[inline]
    fn eq(&self, other: &VlqU32) -> bool {
        *self == other.0
    }
}

impl Marshaler for VlqU32 {
    fn marshal(&self, wb: &mut WriteBuffer) {
        VlqU32Marshaler.marshal(wb, self.0);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        Ok(VlqU32(VlqU32Marshaler.unmarshal(rb)?))
    }
}

/// Encodes a `u64` in 1..=9 bytes using the crate's VLQ format.
#[derive(Debug, Clone, Copy, Default)]
pub struct VlqU64Marshaler;

impl VlqU64Marshaler {
    pub const MAX_ENCODING_BYTES: usize = 9;

    #[inline]
    fn byte_after_bits(v: u64, bits: u32) -> u8 {
        byte(v >> bits)
    }

    #[inline]
    pub fn marshal(&self, wb: &mut WriteBuffer, v: u64) {
        let mut data = [0u8; 9];
        if v < 0x80 {
            data[0] = byte(v);
            wb.write_bytes(&data[..1]);
        } else if v < 0x4000 {
            data[0] = 0x80 | byte(v & 0x3f);
            data[1] = Self::byte_after_bits(v, 6);
            wb.write_bytes(&data[..2]);
        } else if v < 0x20_0000 {
            data[0] = 0xc0 | byte(v & 0x1f);
            data[1] = Self::byte_after_bits(v, 5);
            data[2] = Self::byte_after_bits(v, 13);
            wb.write_bytes(&data[..3]);
        } else if v < 0x1000_0000 {
            data[0] = 0xe0 | byte(v & 0x0f);
            data[1] = Self::byte_after_bits(v, 4);
            data[2] = Self::byte_after_bits(v, 12);
            data[3] = Self::byte_after_bits(v, 20);
            wb.write_bytes(&data[..4]);
        } else if v < 0x0000_0000_0800_0000 {
            data[0] = 0xf0 | byte(v & 0x07);
            data[1] = Self::byte_after_bits(v, 3);
            data[2] = Self::byte_after_bits(v, 11);
            data[3] = Self::byte_after_bits(v, 19);
            data[4] = Self::byte_after_bits(v, 27);
            wb.write_bytes(&data[..5]);
        } else if v < 0x0000_0400_0000_0000 {
            data[0] = 0xF8 | byte(v & 0x03);
            data[1] = Self::byte_after_bits(v, 2);
            data[2] = Self::byte_after_bits(v, 10);
            data[3] = Self::byte_after_bits(v, 18);
            data[4] = Self::byte_after_bits(v, 26);
            data[5] = Self::byte_after_bits(v, 34);
            wb.write_bytes(&data[..6]);
        } else if v < 0x0002_0000_0000_0000 {
            data[0] = 0xFC | byte(v & 0x01);
            data[1] = Self::byte_after_bits(v, 1);
            data[2] = Self::byte_after_bits(v, 9);
            data[3] = Self::byte_after_bits(v, 17);
            data[4] = Self::byte_after_bits(v, 25);
            data[5] = Self::byte_after_bits(v, 33);
            data[6] = Self::byte_after_bits(v, 41);
            wb.write_bytes(&data[..7]);
        } else if v < 0x0100_0000_0000_0000 {
            data[0] = 0xFE;
            data[1] = Self::byte_after_bits(v, 0);
            data[2] = Self::byte_after_bits(v, 8);
            data[3] = Self::byte_after_bits(v, 16);
            data[4] = Self::byte_after_bits(v, 24);
            data[5] = Self::byte_after_bits(v, 32);
            data[6] = Self::byte_after_bits(v, 40);
            data[7] = Self::byte_after_bits(v, 48);
            wb.write_bytes(&data[..8]);
        } else {
            data[0] = 0xFF;
            data[1] = Self::byte_after_bits(v, 0);
            data[2] = Self::byte_after_bits(v, 8);
            data[3] = Self::byte_after_bits(v, 16);
            data[4] = Self::byte_after_bits(v, 24);
            data[5] = Self::byte_after_bits(v, 32);
            data[6] = Self::byte_after_bits(v, 40);
            data[7] = Self::byte_after_bits(v, 48);
            data[8] = Self::byte_after_bits(v, 56);
            wb.write_bytes(&data[..9]);
        }
    }

    #[inline]
    /// Decode a `u64` VLQ.
    ///
    /// # Errors
    ///
    /// Returns an error when the buffer is truncated.
    pub fn unmarshal(&self, rb: &mut ReadBuffer) -> Result<u64, MarshalerError> {
        let first = rb.read_u8()?;
        if first < 0x80 {
            Ok(u64::from(first))
        } else if first < 0xc0 {
            let b1 = rb.read_u8()?;
            Ok((u64::from(first & !0xc0)) | ((u64::from(b1)) << 6))
        } else if first < 0xe0 {
            let b1 = rb.read_u8()?;
            let b2 = rb.read_u8()?;
            Ok((u64::from(first & !0xe0)) | ((u64::from(b1)) << 5) | ((u64::from(b2)) << 13))
        } else if first < 0xf0 {
            let b1 = rb.read_u8()?;
            let b2 = rb.read_u8()?;
            let b3 = rb.read_u8()?;
            Ok((u64::from(first & !0xf0))
                | ((u64::from(b1)) << 4)
                | ((u64::from(b2)) << 12)
                | ((u64::from(b3)) << 20))
        } else if first < 0xF8 {
            let b1 = rb.read_u8()?;
            let b2 = rb.read_u8()?;
            let b3 = rb.read_u8()?;
            let b4 = rb.read_u8()?;
            Ok((u64::from(first & !0xf8))
                | ((u64::from(b1)) << 3)
                | ((u64::from(b2)) << 11)
                | ((u64::from(b3)) << 19)
                | ((u64::from(b4)) << 27))
        } else if first < 0xFC {
            let b1 = rb.read_u8()?;
            let b2 = rb.read_u8()?;
            let b3 = rb.read_u8()?;
            let b4 = rb.read_u8()?;
            let b5 = rb.read_u8()?;
            Ok((u64::from(first & !0xFC))
                | ((u64::from(b1)) << 2)
                | ((u64::from(b2)) << 10)
                | ((u64::from(b3)) << 18)
                | ((u64::from(b4)) << 26)
                | ((u64::from(b5)) << 34))
        } else if first < 0xFE {
            let b1 = rb.read_u8()?;
            let b2 = rb.read_u8()?;
            let b3 = rb.read_u8()?;
            let b4 = rb.read_u8()?;
            let b5 = rb.read_u8()?;
            let b6 = rb.read_u8()?;
            Ok((u64::from(first & !0xFE))
                | ((u64::from(b1)) << 1)
                | ((u64::from(b2)) << 9)
                | ((u64::from(b3)) << 17)
                | ((u64::from(b4)) << 25)
                | ((u64::from(b5)) << 33)
                | ((u64::from(b6)) << 41))
        } else if first < 0xFF {
            let b1 = rb.read_u8()?;
            let b2 = rb.read_u8()?;
            let b3 = rb.read_u8()?;
            let b4 = rb.read_u8()?;
            let b5 = rb.read_u8()?;
            let b6 = rb.read_u8()?;
            let b7 = rb.read_u8()?;
            Ok(u64::from(b1)
                | ((u64::from(b2)) << 8)
                | ((u64::from(b3)) << 16)
                | ((u64::from(b4)) << 24)
                | ((u64::from(b5)) << 32)
                | ((u64::from(b6)) << 40)
                | ((u64::from(b7)) << 48))
        } else {
            let b1 = rb.read_u8()?;
            let b2 = rb.read_u8()?;
            let b3 = rb.read_u8()?;
            let b4 = rb.read_u8()?;
            let b5 = rb.read_u8()?;
            let b6 = rb.read_u8()?;
            let b7 = rb.read_u8()?;
            let b8 = rb.read_u8()?;
            Ok(u64::from(b1)
                | ((u64::from(b2)) << 8)
                | ((u64::from(b3)) << 16)
                | ((u64::from(b4)) << 24)
                | ((u64::from(b5)) << 32)
                | ((u64::from(b6)) << 40)
                | ((u64::from(b7)) << 48)
                | ((u64::from(b8)) << 56))
        }
    }
}

/// `VlqU64Marshaler` is also a [`Codec<u64>`] policy for source-shaped
/// replicated containers and fields whose native value is `u64` but whose wire
/// encoding is VLQ.
///
/// [`Codec<u64>`]: super::marshaler::Codec
impl super::marshaler::Codec<u64> for VlqU64Marshaler {
    fn marshal(value: &u64, wb: &mut WriteBuffer) {
        VlqU64Marshaler.marshal(wb, *value);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<u64, MarshalerError> {
        VlqU64Marshaler.unmarshal(rb)
    }
}

/// Newtype wrapper around `u64` whose [`Marshaler`] impl emits the
/// 1..=9-byte VLQ encoding. Use as the field type when the wire format is a
/// standalone VLQ-u64 (for example sequence numbers consumed mid-struct)
/// rather than the default 8-byte big-endian `Marshaler<u64>` write.
///
/// # Composing with `Option<T>`
///
/// `Option<VlqU64>` expresses the concrete wire shape `[u8 has][opt VLQ u64]`
/// `Option<T>: Marshaler` writes the strict-bool `0`/`1` prefix and rejects
/// `> 1` on read; `VlqU64` supplies the VLQ-u64 payload, so no dedicated helper
/// type is needed:
///
/// ```ignore
/// pub sequence: Option<VlqU64>,   // wire: [u8 has][opt VLQ u64]
/// ```
///
/// # Ergonomics
///
/// Mirrors [`VlqU32`]'s ergonomics: `Deref<Target = u64>`, `From<u64>`,
/// `From<VlqU64> for u64`, `PartialEq<u64>`, `Display`, and `AsRef<u64>` so
/// the wrapper drops into existing arithmetic and formatting sites without
/// ceremony. This is only a scalar encoding building block; higher-level
/// container type that owns it.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    AsRef,
    Deref,
    DerefMut,
    Display,
    From,
    Into,
)]
pub struct VlqU64(pub u64);

impl VlqU64 {
    /// Construct from a raw `u64`. Same as `VlqU64::from(value)` but `const`.
    #[inline]
    #[must_use]
    pub const fn new(value: u64) -> Self {
        VlqU64(value)
    }

    /// Extract the inner `u64`. Same as `u64::from(value)` but `const`.
    #[inline]
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl PartialEq<u64> for VlqU64 {
    #[inline]
    fn eq(&self, other: &u64) -> bool {
        self.0 == *other
    }
}

impl PartialEq<VlqU64> for u64 {
    #[inline]
    fn eq(&self, other: &VlqU64) -> bool {
        *self == other.0
    }
}

impl Marshaler for VlqU64 {
    fn marshal(&self, wb: &mut WriteBuffer) {
        VlqU64Marshaler.marshal(wb, self.0);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        Ok(VlqU64(VlqU64Marshaler.unmarshal(rb)?))
    }
}

#[cfg(test)]
mod wrapper_tests {
    use super::*;
    use crate::serialize::buffer::CARRIER_ENDIAN;

    /// `VlqU16` round-trips identically to `VlqU16Marshaler` itself, and
    /// rejects values that don't fit in u16 on read.
    #[test]
    fn vlq_u16_wrapper_round_trip_matches_codec() {
        for &v in &[0u16, 0x7f, 0x80, 0x3fff, 0x4000, u16::MAX] {
            let mut wb_codec = WriteBuffer::new(CARRIER_ENDIAN);
            VlqU16Marshaler.marshal(&mut wb_codec, v);

            let mut wb_wrapper = WriteBuffer::new(CARRIER_ENDIAN);
            VlqU16(v).marshal(&mut wb_wrapper);

            let codec_bytes = wb_codec.into_vec();
            let wrapper_bytes = wb_wrapper.into_vec();
            assert_eq!(codec_bytes, wrapper_bytes, "byte mismatch for {v}");

            let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &wrapper_bytes);
            let decoded = VlqU16::unmarshal(&mut rb).unwrap();
            assert_eq!(decoded.get(), v);
            assert_eq!(rb.left(), 0);
        }
    }

    #[test]
    fn vlq_u16_unmarshal_rejects_overflow() {
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        VlqU32Marshaler.marshal(&mut wb, u32::from(u16::MAX) + 1);
        let bytes = wb.into_vec();
        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bytes);
        match VlqU16Marshaler.unmarshal(&mut rb) {
            Err(MarshalerError::ContainerOverflow { .. }) => {}
            other => panic!("expected ContainerOverflow, got {other:?}"),
        }
    }

    #[test]
    fn vlq_u16_policy_codec_matches_wrapper() {
        for &value in &[0u16, 0x7f, 0x80, 0x3fff, 0x4000, u16::MAX] {
            let mut wb_policy = WriteBuffer::new(CARRIER_ENDIAN);
            <VlqU16Marshaler as crate::serialize::Codec<u16>>::marshal(&value, &mut wb_policy);

            let mut wb_wrapper = WriteBuffer::new(CARRIER_ENDIAN);
            VlqU16::new(value).marshal(&mut wb_wrapper);

            assert_eq!(wb_policy.as_slice(), wb_wrapper.as_slice());

            let mut rb = ReadBuffer::new(CARRIER_ENDIAN, wb_policy.as_slice());
            let decoded =
                <VlqU16Marshaler as crate::serialize::Codec<u16>>::unmarshal(&mut rb).unwrap();
            assert_eq!(decoded, value);
            assert_eq!(rb.left(), 0);
        }
    }

    /// `VlqU32` round-trips identically to `VlqU32Marshaler` itself. The
    /// wrapper is supposed to be a pure ergonomic shell over the existing
    /// codec, so the byte streams must match exactly.
    #[test]
    fn vlq_u32_wrapper_round_trip_matches_codec() {
        for &v in &[
            0u32,
            0x7f,
            0x80,
            0x3fff,
            0x4000,
            0x001f_ffff,
            0x0020_0000,
            u32::MAX,
        ] {
            let mut wb_codec = WriteBuffer::new(CARRIER_ENDIAN);
            VlqU32Marshaler.marshal(&mut wb_codec, v);

            let mut wb_wrapper = WriteBuffer::new(CARRIER_ENDIAN);
            VlqU32(v).marshal(&mut wb_wrapper);

            let codec_bytes = wb_codec.into_vec();
            let wrapper_bytes = wb_wrapper.into_vec();
            assert_eq!(
                codec_bytes, wrapper_bytes,
                "wrapper must emit the same bytes as the codec for {v}"
            );

            let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &wrapper_bytes);
            let decoded = VlqU32::unmarshal(&mut rb).unwrap();
            assert_eq!(decoded.get(), v);
            assert_eq!(
                rb.left(),
                0,
                "wrapper unmarshal must consume exactly the encoded bytes"
            );
        }
    }

    /// `VlqU64` round-trips identically to `VlqU64Marshaler` itself.
    #[test]
    fn vlq_u64_wrapper_round_trip_matches_codec() {
        for &v in &[
            0u64,
            0x7f,
            0x80,
            0x3fff,
            0x4000,
            0x001f_ffff,
            0x1000_0000,
            0x0800_0000,
            0x0400_0000_0000,
            0x0002_0000_0000_0000,
            0x0100_0000_0000_0000,
            u64::MAX,
        ] {
            let mut wb_codec = WriteBuffer::new(CARRIER_ENDIAN);
            VlqU64Marshaler.marshal(&mut wb_codec, v);

            let mut wb_wrapper = WriteBuffer::new(CARRIER_ENDIAN);
            VlqU64(v).marshal(&mut wb_wrapper);

            let codec_bytes = wb_codec.into_vec();
            let wrapper_bytes = wb_wrapper.into_vec();
            assert_eq!(codec_bytes, wrapper_bytes, "byte mismatch for {v}");

            let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &wrapper_bytes);
            let decoded = VlqU64::unmarshal(&mut rb).unwrap();
            assert_eq!(decoded.get(), v);
            assert_eq!(rb.left(), 0);
        }
    }

    #[test]
    fn vlq_u64_policy_codec_matches_wrapper() {
        for &value in &[
            0u64,
            0x7f,
            0x80,
            0x3fff,
            0x4000,
            0x001f_ffff,
            0x1000_0000,
            0x0002_0000_0000_0000,
            u64::MAX,
        ] {
            let mut wb_policy = WriteBuffer::new(CARRIER_ENDIAN);
            <VlqU64Marshaler as crate::serialize::Codec<u64>>::marshal(&value, &mut wb_policy);

            let mut wb_wrapper = WriteBuffer::new(CARRIER_ENDIAN);
            VlqU64::new(value).marshal(&mut wb_wrapper);

            assert_eq!(wb_policy.as_slice(), wb_wrapper.as_slice());

            let mut rb = ReadBuffer::new(CARRIER_ENDIAN, wb_policy.as_slice());
            let decoded =
                <VlqU64Marshaler as crate::serialize::Codec<u64>>::unmarshal(&mut rb).unwrap();
            assert_eq!(decoded, value);
            assert_eq!(rb.left(), 0);
        }
    }

    /// inner integer, and `Display`. These are what makes the wrapper usable
    /// as a drop-in field type.
    #[test]
    fn vlq_u32_newtype_traits() {
        let v: VlqU32 = 42_u32.into();
        // Deref into u32 enables arithmetic and indexing.
        assert_eq!(*v, 42);
        // PartialEq against u32 (both directions).
        assert_eq!(v, 42_u32);
        assert_eq!(42_u32, v);
        // Into<u32> via the symmetric From impl.
        let raw: u32 = v.into();
        assert_eq!(raw, 42);
        // Display passes through to the underlying integer formatter.
        assert_eq!(format!("{v}"), "42");
        // Const constructor.
        let c = VlqU32::new(7);
        assert_eq!(c.get(), 7);
    }

    #[test]
    fn vlq_u64_newtype_traits() {
        let v: VlqU64 = 0xDEAD_BEEF_u64.into();
        assert_eq!(*v, 0xDEAD_BEEF);
        assert_eq!(v, 0xDEAD_BEEF_u64);
        assert_eq!(0xDEAD_BEEF_u64, v);
        let raw: u64 = v.into();
        assert_eq!(raw, 0xDEAD_BEEF);
        assert_eq!(format!("{v}"), "3735928559");
        let c = VlqU64::new(7);
        assert_eq!(c.get(), 7);
    }
}
