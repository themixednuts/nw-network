//! Compression-oriented codecs for packed floats, vectors, quaternions,
//! transforms, and bit-size fields.
//!
//! The codecs preserve compact wire layouts built from sentinel flag bytes,
//! half-floats, quantized Euler bytes, omitted-largest quaternion components,
//! and byte-aligned packed sizes.

use super::{
    buffer::{ReadBuffer, WriteBuffer},
    error::MarshalerError,
    marshaler::{Codec, Marshaler},
    quantize::{f32_to_i32, f32_to_u8, f32_to_u16, f32_to_u32, i32_to_f32, u32_to_f32},
    utility_marshal::HalfF32,
};
use glam::{Affine3A, EulerRot, Mat3A, Quat, Vec2, Vec3, Vec3A};

fn is_exact_zero(value: f32) -> bool {
    value.to_bits().trailing_zeros() >= 31
}

fn is_exact_one(value: f32) -> bool {
    value.to_bits() == 1.0f32.to_bits()
}

/// Quantized Euler-angle quaternion codec:
/// - one flag byte for per-axis zero/one sentinels,
/// - one `u8` per axis not covered by the sentinel bits,
/// - byte values represent `angle / 1.40625`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuatCompNormQuantized(pub Quat);

impl QuatCompNormQuantized {
    const DEGREES_PER_QUANTIZED_VALUE: f32 = 360.0 / 256.0;
    const X_ZERO: u8 = 1 << 0;
    const Y_ZERO: u8 = 1 << 1;
    const Z_ZERO: u8 = 1 << 2;
    const X_ONE: u8 = 1 << 3;
    const Y_ONE: u8 = 1 << 4;
    const Z_ONE: u8 = 1 << 5;

    #[must_use]
    pub fn from_euler_degrees(values: [f32; 3]) -> Self {
        Self(Quat::from_euler(
            EulerRot::XYZ,
            values[0].to_radians(),
            values[1].to_radians(),
            values[2].to_radians(),
        ))
    }

    #[must_use]
    pub fn euler_degrees(&self) -> [f32; 3] {
        let (x, y, z) = self.0.to_euler(EulerRot::XYZ);
        [x.to_degrees(), y.to_degrees(), z.to_degrees()]
    }

    #[inline]
    fn write_axis(angle: f32, zero_flag: u8, one_flag: u8, flags: &mut u8) {
        if is_exact_zero(angle) {
            *flags |= zero_flag;
        } else if is_exact_one(angle) {
            *flags |= one_flag;
        }
    }

    #[inline]
    fn write_axis_payload(
        angle: f32,
        flags: u8,
        zero_flag: u8,
        one_flag: u8,
        wb: &mut WriteBuffer,
    ) {
        if (flags & (zero_flag | one_flag)) == 0 {
            wb.write_u8(f32_to_u8(
                (angle / Self::DEGREES_PER_QUANTIZED_VALUE).clamp(0.0, 255.0),
            ));
        }
    }

    #[inline]
    fn read_axis(
        rb: &mut ReadBuffer,
        flags: u8,
        zero_flag: u8,
        one_flag: u8,
    ) -> Result<f32, MarshalerError> {
        let quantized = if (flags & zero_flag) != 0 {
            0
        } else if (flags & one_flag) != 0 {
            1
        } else {
            rb.read_u8()?
        };
        Ok(f32::from(quantized) * Self::DEGREES_PER_QUANTIZED_VALUE)
    }
}

impl Default for QuatCompNormQuantized {
    fn default() -> Self {
        Self(Quat::IDENTITY)
    }
}

impl Marshaler for QuatCompNormQuantized {
    fn marshal(&self, wb: &mut WriteBuffer) {
        let [x, y, z] = self.euler_degrees();
        let mut flags = 0u8;
        Self::write_axis(x, Self::X_ZERO, Self::X_ONE, &mut flags);
        Self::write_axis(y, Self::Y_ZERO, Self::Y_ONE, &mut flags);
        Self::write_axis(z, Self::Z_ZERO, Self::Z_ONE, &mut flags);

        wb.write_u8(flags);
        Self::write_axis_payload(x, flags, Self::X_ZERO, Self::X_ONE, wb);
        Self::write_axis_payload(y, flags, Self::Y_ZERO, Self::Y_ONE, wb);
        Self::write_axis_payload(z, flags, Self::Z_ZERO, Self::Z_ONE, wb);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let flags = rb.read_u8()?;
        let x = Self::read_axis(rb, flags, Self::X_ZERO, Self::X_ONE)?;
        let y = Self::read_axis(rb, flags, Self::Y_ZERO, Self::Y_ONE)?;
        let z = Self::read_axis(rb, flags, Self::Z_ZERO, Self::Z_ONE)?;
        Ok(Self::from_euler_degrees([x, y, z]))
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct QuatCompNormQuantizedMarshaler;

impl Codec<QuatCompNormQuantized> for QuatCompNormQuantizedMarshaler {
    fn marshal(value: &QuatCompNormQuantized, wb: &mut WriteBuffer) {
        value.marshal(wb);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<QuatCompNormQuantized, MarshalerError> {
        QuatCompNormQuantized::unmarshal(rb)
    }
}

///
/// helper below.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct QuatCompNormQuantizedAngles {
    pub first: u32,
    pub second: u32,
}

impl Marshaler for QuatCompNormQuantizedAngles {
    fn marshal(&self, wb: &mut WriteBuffer) {
        self.first.marshal(wb);
        self.second.marshal(wb);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        Ok(Self {
            first: u32::unmarshal(rb)?,
            second: u32::unmarshal(rb)?,
        })
    }
}

///
/// - flags bit 0: omitted component sign,
/// - flags bits 1..=4: component is exactly zero,
/// - flags bits 5..=6: index of the omitted largest component,
/// - payload: one byte for each non-omitted, non-zero component.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuatSmallestThreeQuantized {
    pub components: [f32; 4],
}

impl QuatSmallestThreeQuantized {
    const COMPONENT_SCALE: f32 = std::f32::consts::SQRT_2 / 255.0;
    const COMPONENT_OFFSET: f32 = std::f32::consts::FRAC_1_SQRT_2;
    const SIGN_NEGATIVE: u8 = 1 << 0;
    const INDEX_SHIFT: u8 = 5;

    #[must_use]
    pub fn from_xyzw(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self {
            components: [x, y, z, w],
        }
    }

    #[must_use]
    pub fn as_quat(&self) -> Quat {
        Quat::from_xyzw(
            self.components[0],
            self.components[1],
            self.components[2],
            self.components[3],
        )
    }

    #[must_use]
    pub fn from_euler_degrees(values: [f32; 3]) -> Self {
        let q = Quat::from_euler(
            EulerRot::XYZ,
            values[0].to_radians(),
            values[1].to_radians(),
            values[2].to_radians(),
        );
        Self::from_xyzw(q.x, q.y, q.z, q.w)
    }

    #[must_use]
    pub fn euler_degrees(&self) -> [f32; 3] {
        let (x, y, z) = self.as_quat().to_euler(EulerRot::XYZ);
        [x.to_degrees(), y.to_degrees(), z.to_degrees()]
    }

    fn zero_flag(index: usize) -> u8 {
        1 << (index + 1)
    }

    fn quantize_component(value: f32) -> u8 {
        f32_to_u8(
            ((value + Self::COMPONENT_OFFSET) / Self::COMPONENT_SCALE)
                .round()
                .clamp(0.0, f32::from(u8::MAX)),
        )
    }

    fn unquantize_component(value: u8) -> f32 {
        f32::from(value) * Self::COMPONENT_SCALE - Self::COMPONENT_OFFSET
    }
}

impl Default for QuatSmallestThreeQuantized {
    fn default() -> Self {
        Self::from_xyzw(0.0, 0.0, 0.0, 1.0)
    }
}

impl Marshaler for QuatSmallestThreeQuantized {
    fn marshal(&self, wb: &mut WriteBuffer) {
        let mut largest_index = 0usize;
        let mut largest_abs = self.components[0].abs();
        for (index, value) in self.components.iter().copied().enumerate().skip(1) {
            let abs = value.abs();
            if abs > largest_abs {
                largest_abs = abs;
                largest_index = index;
            }
        }

        let largest_index_byte = u8::try_from(largest_index).expect("quaternion index fits in u8");
        let mut flags = largest_index_byte << Self::INDEX_SHIFT;
        if self.components[largest_index] < 0.0 {
            flags |= Self::SIGN_NEGATIVE;
        }
        for (index, value) in self.components.iter().copied().enumerate() {
            if index != largest_index && is_exact_zero(value) {
                flags |= Self::zero_flag(index);
            }
        }
        wb.write_u8(flags);
        for (index, value) in self.components.iter().copied().enumerate() {
            if index != largest_index && (flags & Self::zero_flag(index)) == 0 {
                wb.write_u8(Self::quantize_component(value));
            }
        }
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let flags = rb.read_u8()?;
        let largest_index = usize::from((flags >> Self::INDEX_SHIFT) & 0x03);
        let mut components = [0.0; 4];
        let mut sum_squares = 0.0f32;

        for (index, component) in components.iter_mut().enumerate() {
            if index == largest_index {
                continue;
            }

            *component = if (flags & Self::zero_flag(index)) != 0 {
                0.0
            } else {
                Self::unquantize_component(rb.read_u8()?)
            };
            sum_squares += *component * *component;
        }

        let mut omitted = (1.0 - sum_squares).max(0.0).sqrt();
        if (flags & Self::SIGN_NEGATIVE) != 0 {
            omitted = -omitted;
        }
        components[largest_index] = omitted;

        Ok(Self { components })
    }
}

/// Policy marshaler for [`QuatSmallestThreeQuantized`].
#[derive(Debug, Clone, Copy, Default)]
pub struct QuatSmallestThreeQuantizedMarshaler;

impl Codec<QuatSmallestThreeQuantized> for QuatSmallestThreeQuantizedMarshaler {
    fn marshal(value: &QuatSmallestThreeQuantized, wb: &mut WriteBuffer) {
        value.marshal(wb);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<QuatSmallestThreeQuantized, MarshalerError> {
        QuatSmallestThreeQuantized::unmarshal(rb)
    }
}

/// Compressed normalized quaternion.
///
/// - one flag byte for xyz zero/one sentinels and w sign,
/// - big-endian `u16` quantized values for xyz components not covered by
///   sentinel bits,
/// - w reconstructed as `sqrt(1 - x*x - y*y - z*z)`.
#[derive(Debug, Clone, Copy, PartialEq, derive_more::From, derive_more::Into)]
pub struct QuatCompNorm(pub Quat);

impl Default for QuatCompNorm {
    fn default() -> Self {
        Self(Quat::IDENTITY)
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct QuatCompNormMarshaler;

impl QuatCompNormMarshaler {
    const X_ZERO: u8 = 1 << 0;
    const Y_ZERO: u8 = 1 << 1;
    const Z_ZERO: u8 = 1 << 2;
    const X_ONE: u8 = 1 << 3;
    const Y_ONE: u8 = 1 << 4;
    const Z_ONE: u8 = 1 << 5;
    const W_NEGATIVE: u8 = 1 << 6;

    #[inline]
    fn quantize_component(value: f32) -> u16 {
        f32_to_u16(((value + 1.0) * 32767.5).clamp(0.0, f32::from(u16::MAX)))
    }

    #[inline]
    fn unquantize_component(value: u16) -> f32 {
        ((f32::from(value) * (2.0 / f32::from(u16::MAX))) - 1.0).clamp(-1.0, 1.0)
    }

    #[inline]
    fn write_component(value: f32, zero_flag: u8, one_flag: u8, flags: &mut u8) {
        if is_exact_zero(value) {
            *flags |= zero_flag;
        } else if is_exact_one(value) {
            *flags |= one_flag;
        }
    }

    #[inline]
    fn read_component(
        rb: &mut ReadBuffer,
        flags: u8,
        zero_flag: u8,
        one_flag: u8,
    ) -> Result<f32, MarshalerError> {
        if (flags & zero_flag) != 0 {
            Ok(0.0)
        } else if (flags & one_flag) != 0 {
            Ok(1.0)
        } else {
            Ok(Self::unquantize_component(rb.read_u16()?))
        }
    }
}

impl Codec<QuatCompNorm> for QuatCompNormMarshaler {
    fn marshal(value: &QuatCompNorm, wb: &mut WriteBuffer) {
        let q = value.0;
        let mut flags = 0u8;
        Self::write_component(q.x, Self::X_ZERO, Self::X_ONE, &mut flags);
        Self::write_component(q.y, Self::Y_ZERO, Self::Y_ONE, &mut flags);
        Self::write_component(q.z, Self::Z_ZERO, Self::Z_ONE, &mut flags);
        if q.w < 0.0 {
            flags |= Self::W_NEGATIVE;
        }

        wb.write_u8(flags);
        if (flags & (Self::X_ZERO | Self::X_ONE)) == 0 {
            wb.write_u16(Self::quantize_component(q.x));
        }
        if (flags & (Self::Y_ZERO | Self::Y_ONE)) == 0 {
            wb.write_u16(Self::quantize_component(q.y));
        }
        if (flags & (Self::Z_ZERO | Self::Z_ONE)) == 0 {
            wb.write_u16(Self::quantize_component(q.z));
        }
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<QuatCompNorm, MarshalerError> {
        let flags = rb.read_u8()?;
        let x = Self::read_component(rb, flags, Self::X_ZERO, Self::X_ONE)?;
        let y = Self::read_component(rb, flags, Self::Y_ZERO, Self::Y_ONE)?;
        let z = Self::read_component(rb, flags, Self::Z_ZERO, Self::Z_ONE)?;

        let mut w = (1.0 - x * x - y * y - z * z).max(0.0).sqrt();
        if (flags & Self::W_NEGATIVE) != 0 {
            w = -w;
        }

        let len = (x * x + y * y + z * z + w * w).sqrt();
        let quat = if len > 0.0 {
            Quat::from_xyzw(x / len, y / len, z / len, w / len)
        } else {
            Quat::IDENTITY
        };
        Ok(QuatCompNorm(quat))
    }
}

impl Marshaler for QuatCompNorm {
    fn marshal(&self, wb: &mut WriteBuffer) {
        QuatCompNormMarshaler::marshal(self, wb);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        QuatCompNormMarshaler::unmarshal(rb)
    }
}

///
/// 6 bytes. `Vec3CompNormMarshaler` is the variable-width normalized-vector
#[derive(Debug, Clone, Copy, Default)]
pub struct Vec3CompMarshaler;

impl Codec<Vec3> for Vec3CompMarshaler {
    const MARSHAL_SIZE: usize = 6;

    fn marshal(value: &Vec3, wb: &mut WriteBuffer) {
        HalfF32(value.x).marshal(wb);
        HalfF32(value.y).marshal(wb);
        HalfF32(value.z).marshal(wb);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Vec3, MarshalerError> {
        let HalfF32(x) = HalfF32::unmarshal(rb)?;
        let HalfF32(y) = HalfF32::unmarshal(rb)?;
        let HalfF32(z) = HalfF32::unmarshal(rb)?;
        Ok(Vec3::new(x, y, z))
    }
}

/// Quantizes an `f32` into a `u16` within a caller-supplied
/// `[range_min, range_max]` interval.
///
/// Use for fields whose value range is known and bounded — e.g. weapon
/// accuracy scalar in [0.0, 1.0] — where the full 32-bit float is wasteful.
/// For unbounded-range half-float compression see [`HalfF32`] in
/// [`super::utility_marshal`].
///
/// This type stores range state, so it is used as a value-level codec rather
/// than a zero-sized type-level policy.
#[derive(Debug, Clone, Copy)]
pub struct Float16Marshaler {
    min: f32,
    range: f32,
}

impl Float16Marshaler {
    pub const MARSHAL_SIZE: usize = 2;

    #[must_use]
    pub fn new(range_min: f32, range_max: f32) -> Self {
        debug_assert!(
            range_max > range_min,
            "Float16Marshaler range must be positive"
        );
        Self {
            min: range_min,
            range: range_max - range_min,
        }
    }

    pub fn marshal(&self, wb: &mut WriteBuffer, value: f32) {
        let normalized = ((value - self.min) / self.range).clamp(0.0, 1.0);
        let q = f32_to_u16((normalized * f32::from(u16::MAX)).round());
        q.marshal(wb);
    }

    /// Decode a quantized bounded float.
    ///
    /// # Errors
    ///
    /// Returns an error when the underlying `u16` payload is truncated.
    pub fn unmarshal(&self, rb: &mut ReadBuffer) -> Result<f32, MarshalerError> {
        let q = u16::unmarshal(rb)?;
        Ok(self.min + (f32::from(q) / f32::from(u16::MAX)) * self.range)
    }
}

/// Half-precision two-component vector codec.
#[derive(Debug, Clone, Copy, Default)]
pub struct Vec2CompMarshaler;

impl Codec<Vec2> for Vec2CompMarshaler {
    const MARSHAL_SIZE: usize = 4;

    fn marshal(value: &Vec2, wb: &mut WriteBuffer) {
        HalfF32(value.x).marshal(wb);
        HalfF32(value.y).marshal(wb);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Vec2, MarshalerError> {
        let HalfF32(x) = HalfF32::unmarshal(rb)?;
        let HalfF32(y) = HalfF32::unmarshal(rb)?;
        Ok(Vec2::new(x, y))
    }
}

/// Half-precision quaternion codec that writes `x`, `y`, `z`, and `w` as
/// four `f16` values, 8 bytes total.
#[derive(Debug, Clone, Copy, Default)]
pub struct QuatCompMarshaler;

impl Codec<Quat> for QuatCompMarshaler {
    const MARSHAL_SIZE: usize = 8;

    fn marshal(value: &Quat, wb: &mut WriteBuffer) {
        HalfF32(value.x).marshal(wb);
        HalfF32(value.y).marshal(wb);
        HalfF32(value.z).marshal(wb);
        HalfF32(value.w).marshal(wb);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Quat, MarshalerError> {
        let HalfF32(x) = HalfF32::unmarshal(rb)?;
        let HalfF32(y) = HalfF32::unmarshal(rb)?;
        let HalfF32(z) = HalfF32::unmarshal(rb)?;
        let HalfF32(w) = HalfF32::unmarshal(rb)?;
        Ok(Quat::from_xyzw(x, y, z, w))
    }
}

/// Compressed normalized-vector codec:
/// - one flag byte holding `X_NEG` (bit 0), `Y_ZERO` (bit 1), `Z_ZERO`
///   (bit 2), `Y_ONE` (bit 3), `Z_ONE` (bit 4);
/// - `y` written via `Float16Marshaler [-1.0, 1.0]` (2 bytes) when not
///   covered by `Y_ZERO` / `Y_ONE`;
/// - `z` written via `Float16Marshaler [-1.0, 1.0]` (2 bytes) when not
///   covered by `Z_ZERO` / `Z_ONE`;
/// - `x` reconstructed as `±sqrt(1 - y*y - z*z)` with sign from `X_NEG`.
///
/// The shape compresses the common cases (unit basis vectors, axis-aligned)
/// down to a single flag byte.
#[derive(Debug, Clone, Copy, Default)]
pub struct Vec3CompNormMarshaler;

impl Vec3CompNormMarshaler {
    const X_NEG: u8 = 1 << 0;
    const Y_ZERO: u8 = 1 << 1;
    const Z_ZERO: u8 = 1 << 2;
    const Y_ONE: u8 = 1 << 3;
    const Z_ONE: u8 = 1 << 4;

    #[inline]
    fn float16_unit() -> Float16Marshaler {
        Float16Marshaler::new(-1.0, 1.0)
    }

    #[inline]
    fn classify_yz(value: f32, zero_flag: u8, one_flag: u8, flags: &mut u8) {
        if is_exact_zero(value) {
            *flags |= zero_flag;
        } else if is_exact_one(value) {
            *flags |= one_flag;
        }
    }
}

impl Codec<Vec3> for Vec3CompNormMarshaler {
    fn marshal(value: &Vec3, wb: &mut WriteBuffer) {
        let mut flags = 0u8;
        if value.x < 0.0 {
            flags |= Self::X_NEG;
        }
        Self::classify_yz(value.y, Self::Y_ZERO, Self::Y_ONE, &mut flags);
        Self::classify_yz(value.z, Self::Z_ZERO, Self::Z_ONE, &mut flags);
        flags.marshal(wb);
        let codec = Self::float16_unit();
        if (flags & (Self::Y_ZERO | Self::Y_ONE)) == 0 {
            codec.marshal(wb, value.y);
        }
        if (flags & (Self::Z_ZERO | Self::Z_ONE)) == 0 {
            codec.marshal(wb, value.z);
        }
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Vec3, MarshalerError> {
        let flags = u8::unmarshal(rb)?;
        let codec = Self::float16_unit();
        let y = if (flags & Self::Y_ZERO) != 0 {
            0.0
        } else if (flags & Self::Y_ONE) != 0 {
            1.0
        } else {
            codec.unmarshal(rb)?
        };
        let z = if (flags & Self::Z_ZERO) != 0 {
            0.0
        } else if (flags & Self::Z_ONE) != 0 {
            1.0
        } else {
            codec.unmarshal(rb)?
        };
        let mut x = (1.0 - y * y - z * z).max(0.0).sqrt();
        if (flags & Self::X_NEG) != 0 {
            x = -x;
        }
        Ok(Vec3::new(x, y, z))
    }
}

///
/// - one flag byte with `HAS_SCALE` (bit 0), `HAS_ROT` (bit 1),
///   `HAS_POS` (bit 2);
/// - if `HAS_SCALE`: 3 raw `HalfF32` values for scale (6 bytes);
/// - if `HAS_ROT`:   4 raw `HalfF32` values for rotation (8 bytes,
///   equivalent to [`QuatCompMarshaler`]);
/// - if `HAS_POS`:   3 raw `f32` values for translation (12 bytes).
///
/// Round-trip preserves only the basis vector lengths (scale) and the
/// rotation/translation components — affine shear is not representable.
/// Identity transforms encode to one byte (flags = 0).
#[derive(Debug, Clone, Copy, Default)]
pub struct TransformCompressor;

impl TransformCompressor {
    const HAS_SCALE: u8 = 1 << 0;
    const HAS_ROT: u8 = 1 << 1;
    const HAS_POS: u8 = 1 << 2;
    const EPSILON: f32 = 1.0e-6;

    fn extract_scale_rotation(matrix3: Mat3A) -> (Vec3, Quat) {
        let x = Vec3::from(matrix3.x_axis);
        let y = Vec3::from(matrix3.y_axis);
        let z = Vec3::from(matrix3.z_axis);
        let scale = Vec3::new(x.length(), y.length(), z.length());
        let normalize = |v: Vec3, len: f32| {
            if len > Self::EPSILON {
                v / len
            } else {
                Vec3::ZERO
            }
        };
        let basis = Mat3A::from_cols(
            Vec3A::from(normalize(x, scale.x)),
            Vec3A::from(normalize(y, scale.y)),
            Vec3A::from(normalize(z, scale.z)),
        );
        let rot = Quat::from_mat3a(&basis);
        (scale, rot)
    }
}

impl Codec<Affine3A> for TransformCompressor {
    fn marshal(value: &Affine3A, wb: &mut WriteBuffer) {
        let (scale, rot) = Self::extract_scale_rotation(value.matrix3);
        let translation = Vec3::from(value.translation);
        let mut flags = 0u8;
        let has_scale = (scale - Vec3::ONE).length_squared() > Self::EPSILON;
        let has_rot = (rot - Quat::IDENTITY).length_squared() > Self::EPSILON;
        let has_pos = translation.length_squared() > Self::EPSILON;
        if has_scale {
            flags |= Self::HAS_SCALE;
        }
        if has_rot {
            flags |= Self::HAS_ROT;
        }
        if has_pos {
            flags |= Self::HAS_POS;
        }
        flags.marshal(wb);
        if has_scale {
            HalfF32(scale.x).marshal(wb);
            HalfF32(scale.y).marshal(wb);
            HalfF32(scale.z).marshal(wb);
        }
        if has_rot {
            QuatCompMarshaler::marshal(&rot, wb);
        }
        if has_pos {
            translation.marshal(wb);
        }
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Affine3A, MarshalerError> {
        let flags = u8::unmarshal(rb)?;
        let scale = if (flags & Self::HAS_SCALE) != 0 {
            let HalfF32(sx) = HalfF32::unmarshal(rb)?;
            let HalfF32(sy) = HalfF32::unmarshal(rb)?;
            let HalfF32(sz) = HalfF32::unmarshal(rb)?;
            Vec3::new(sx, sy, sz)
        } else {
            Vec3::ONE
        };
        let rot = if (flags & Self::HAS_ROT) != 0 {
            QuatCompMarshaler::unmarshal(rb)?
        } else {
            Quat::IDENTITY
        };
        let translation = if (flags & Self::HAS_POS) != 0 {
            Vec3::unmarshal(rb)?
        } else {
            Vec3::ZERO
        };
        Ok(Affine3A::from_scale_rotation_translation(
            scale,
            rot,
            translation,
        ))
    }
}

/// Compact byte/bit count.
///
/// `additional_bits` is normalized into the `0..8` range. Wire encoding stores
/// the total bit count as a VLQ value, then reconstructs the normalized split
/// on read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PackedSize {
    bytes: u32,
    additional_bits: u8,
}

impl PackedSize {
    #[must_use]
    pub fn new(bytes: u32, additional_bits: u8) -> Self {
        let extra_bytes = u32::from(additional_bits / 8);
        Self {
            bytes: bytes.saturating_add(extra_bytes),
            additional_bits: additional_bits % 8,
        }
    }

    #[must_use]
    pub fn from_bits(total_bits: u32) -> Self {
        Self {
            bytes: total_bits / 8,
            additional_bits: u8::try_from(total_bits % 8).unwrap_or(0),
        }
    }

    #[must_use]
    pub const fn from_bytes(bytes: u32) -> Self {
        Self {
            bytes,
            additional_bits: 0,
        }
    }

    #[must_use]
    pub fn total_size_in_bits(&self) -> u32 {
        self.bytes * 8 + u32::from(self.additional_bits)
    }

    #[must_use]
    pub const fn bytes(&self) -> u32 {
        self.bytes
    }

    #[must_use]
    pub const fn additional_bits(&self) -> u8 {
        self.additional_bits
    }
}

impl Marshaler for PackedSize {
    fn marshal(&self, wb: &mut WriteBuffer) {
        use crate::serialize::vlq::VlqU32Marshaler;
        VlqU32Marshaler.marshal(wb, self.total_size_in_bits());
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        use crate::serialize::vlq::VlqU32Marshaler;
        let total_bits = VlqU32Marshaler.unmarshal(rb)?;
        Ok(Self::from_bits(total_bits))
    }
}

/// Quantizes a bounded integer value into `Bytes` wire bytes:
/// - 1-byte form: maps `[MIN, MAX]` onto `[0, 255]` (`u8`),
/// - 2-byte form: maps `[MIN, MAX]` onto `[0, 65535]` (`u16`),
/// - 4-byte form: maps `[MIN, MAX]` onto `[0, 0xffffffff]` (`u32`).
///
/// Wire formula: `q = clamp((value - MIN) / (MAX - MIN), 0, 1) * ratio_max`.
/// Round-trip is lossy when the integer range exceeds the quantization
/// bucket count (a 1-byte codec carrying a [0, 1000] range collapses 1000
/// distinct values into 256 buckets).
///
/// Three separate zero-sized policies keep the encoded integer width explicit
/// at the type level.
pub struct IntegerQuantizationMarshalerU8<const MIN: i32, const MAX: i32>;

impl<const MIN: i32, const MAX: i32> IntegerQuantizationMarshalerU8<MIN, MAX> {
    pub const MARSHAL_SIZE: usize = 1;

    fn range() -> f32 {
        i32_to_f32(MAX - MIN)
    }
}

impl<const MIN: i32, const MAX: i32> Codec<i32> for IntegerQuantizationMarshalerU8<MIN, MAX> {
    const MARSHAL_SIZE: usize = 1;

    fn marshal(value: &i32, wb: &mut WriteBuffer) {
        let scale = (i32_to_f32(*value - MIN) / Self::range()).clamp(0.0, 1.0);
        let q = f32_to_u8(scale * f32::from(u8::MAX));
        q.marshal(wb);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<i32, MarshalerError> {
        let q = u8::unmarshal(rb)?;
        let ratio = Self::range() / f32::from(u8::MAX);
        Ok(MIN + f32_to_i32(f32::from(q) * ratio))
    }
}

pub struct IntegerQuantizationMarshalerU16<const MIN: i32, const MAX: i32>;

impl<const MIN: i32, const MAX: i32> IntegerQuantizationMarshalerU16<MIN, MAX> {
    pub const MARSHAL_SIZE: usize = 2;

    fn range() -> f32 {
        i32_to_f32(MAX - MIN)
    }
}

impl<const MIN: i32, const MAX: i32> Codec<i32> for IntegerQuantizationMarshalerU16<MIN, MAX> {
    const MARSHAL_SIZE: usize = 2;

    fn marshal(value: &i32, wb: &mut WriteBuffer) {
        let scale = (i32_to_f32(*value - MIN) / Self::range()).clamp(0.0, 1.0);
        let q = f32_to_u16(scale * f32::from(u16::MAX));
        q.marshal(wb);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<i32, MarshalerError> {
        let q = u16::unmarshal(rb)?;
        let ratio = Self::range() / f32::from(u16::MAX);
        Ok(MIN + f32_to_i32(f32::from(q) * ratio))
    }
}

pub struct IntegerQuantizationMarshalerU32<const MIN: i32, const MAX: i32>;

impl<const MIN: i32, const MAX: i32> IntegerQuantizationMarshalerU32<MIN, MAX> {
    pub const MARSHAL_SIZE: usize = 4;

    fn range() -> f32 {
        i32_to_f32(MAX - MIN)
    }
}

impl<const MIN: i32, const MAX: i32> Codec<i32> for IntegerQuantizationMarshalerU32<MIN, MAX> {
    const MARSHAL_SIZE: usize = 4;

    fn marshal(value: &i32, wb: &mut WriteBuffer) {
        let scale = (i32_to_f32(*value - MIN) / Self::range()).clamp(0.0, 1.0);
        let q = f32_to_u32(scale * u32_to_f32(u32::MAX));
        q.marshal(wb);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<i32, MarshalerError> {
        let q = u32::unmarshal(rb)?;
        let ratio = Self::range() / u32_to_f32(u32::MAX);
        Ok(MIN + f32_to_i32(u32_to_f32(q) * ratio))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serialize::buffer::CARRIER_ENDIAN;

    #[test]
    fn quat_smallest_three_quantized_roundtrip_wire_flags() {
        let y = QuatSmallestThreeQuantized::unquantize_component(100);
        let value = QuatSmallestThreeQuantized::from_xyzw(0.0, y, 0.0, (1.0 - y * y).sqrt());
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        value.marshal(&mut wb);
        let bytes = wb.into_vec();
        assert_eq!(bytes, [0x6a, 100]);

        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bytes);
        let decoded = QuatSmallestThreeQuantized::unmarshal(&mut rb).unwrap();
        assert_eq!(rb.left(), 0);
        assert_eq!(decoded.components[0].to_bits(), 0.0f32.to_bits());
        assert!((decoded.components[1] - y).abs() < f32::EPSILON);
        assert_eq!(decoded.components[2].to_bits(), 0.0f32.to_bits());
        assert!(decoded.components[3] > 0.0);
    }

    #[test]
    fn default_encodes_smallest_three_identity() {
        let value = QuatSmallestThreeQuantized::default();
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        value.marshal(&mut wb);
        assert_eq!(wb.into_vec(), [0x6e]);
    }

    #[test]
    fn quat_comp_norm_quantized_is_euler_angle_pair_format() {
        let value = QuatCompNormQuantized::default();
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        value.marshal(&mut wb);
        assert_eq!(wb.into_vec(), [0x07]);

        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &[0x01, 1, 100]);
        let decoded = QuatCompNormQuantized::unmarshal(&mut rb).unwrap();
        let euler = decoded.euler_degrees();
        assert!((euler[1] - 1.40625).abs() < 0.001);
        assert!((euler[2] - 140.625).abs() < 0.001);
    }

    #[test]
    fn vec3_comp_is_fixed_three_half_floats() {
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        Vec3CompMarshaler::marshal(&Vec3::ZERO, &mut wb);
        assert_eq!(wb.into_vec(), [0, 0, 0, 0, 0, 0]);

        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        Vec3CompMarshaler::marshal(&Vec3::splat(2.0), &mut wb);
        assert_eq!(wb.into_vec(), [0x40, 0x00, 0x40, 0x00, 0x40, 0x00]);

        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        Vec3CompMarshaler::marshal(&Vec3::new(1.0, 2.0, 3.0), &mut wb);
        let bytes = wb.into_vec();
        assert_eq!(bytes, [0x3c, 0x00, 0x40, 0x00, 0x42, 0x00]);

        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bytes);
        let decoded = Vec3CompMarshaler::unmarshal(&mut rb).unwrap();
        assert_eq!(decoded, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(rb.left(), 0);
    }

    #[test]
    fn quat_comp_norm_identity_uses_zero_flags() {
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        QuatCompNorm::default().marshal(&mut wb);
        assert_eq!(wb.into_vec(), [0x07]);
    }

    #[test]
    fn quat_comp_norm_preserves_w_sign() {
        let value = QuatCompNorm(Quat::from_xyzw(0.0, 0.0, 0.0, -1.0));
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        value.marshal(&mut wb);
        assert_eq!(wb.into_vec(), [0x47]);

        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &[0x47]);
        let decoded = QuatCompNorm::unmarshal(&mut rb).unwrap();
        assert_eq!(decoded.0.w.to_bits(), (-1.0f32).to_bits());
    }

    #[test]
    fn float16_marshaler_roundtrips_within_quantization_step() {
        let codec = Float16Marshaler::new(0.0, 1.0);
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        codec.marshal(&mut wb, 0.5);
        let bytes = wb.into_vec();
        assert_eq!(bytes.len(), 2);
        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bytes);
        let decoded = codec.unmarshal(&mut rb).unwrap();
        // 65535 / 2 ≈ 32768 → 32768/65535 ≈ 0.5000076
        assert!((decoded - 0.5).abs() < 1.0 / 65535.0);
    }

    #[test]
    fn float16_marshaler_clamps_out_of_range() {
        let codec = Float16Marshaler::new(0.0, 1.0);
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        codec.marshal(&mut wb, -5.0);
        let bytes = wb.into_vec();
        assert_eq!(bytes, [0x00, 0x00]);

        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        codec.marshal(&mut wb, 5.0);
        let bytes = wb.into_vec();
        assert_eq!(bytes, [0xff, 0xff]);
    }

    #[test]
    fn vec2_comp_marshaler_is_two_halves() {
        let v = Vec2::new(1.5, -2.25);
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        Vec2CompMarshaler::marshal(&v, &mut wb);
        let bytes = wb.into_vec();
        assert_eq!(bytes.len(), 4);
        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bytes);
        let decoded = Vec2CompMarshaler::unmarshal(&mut rb).unwrap();
        // Half-precision tolerance.
        assert!((decoded.x - v.x).abs() < 0.01);
        assert!((decoded.y - v.y).abs() < 0.01);
    }

    #[test]
    fn quat_comp_marshaler_is_four_halves_8_bytes() {
        let q = Quat::from_xyzw(0.5, -0.5, 0.5, 0.5);
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        QuatCompMarshaler::marshal(&q, &mut wb);
        let bytes = wb.into_vec();
        assert_eq!(bytes.len(), 8);
        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bytes);
        let decoded = QuatCompMarshaler::unmarshal(&mut rb).unwrap();
        // Half-precision tolerance per component.
        for (a, b) in [
            (decoded.x, q.x),
            (decoded.y, q.y),
            (decoded.z, q.z),
            (decoded.w, q.w),
        ] {
            assert!((a - b).abs() < 0.01, "{a} vs {b}");
        }
    }

    #[test]
    fn vec3_comp_norm_identity_basis_is_one_byte() {
        // (1, 0, 0): X is positive, Y/Z are zero — pure flag byte.
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        Vec3CompNormMarshaler::marshal(&Vec3::X, &mut wb);
        // X_NEG=0, Y_ZERO=1, Z_ZERO=1 → 0b00000110 = 0x06.
        assert_eq!(wb.into_vec(), [0x06]);

        // (-1, 0, 0): X negative.
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        Vec3CompNormMarshaler::marshal(&-Vec3::X, &mut wb);
        // X_NEG=1, Y_ZERO=1, Z_ZERO=1 → 0b00000111 = 0x07.
        assert_eq!(wb.into_vec(), [0x07]);
    }

    #[test]
    fn vec3_comp_norm_marshaler_roundtrip_arbitrary_normalized() {
        let v = Vec3::new(0.5, 0.5, std::f32::consts::FRAC_1_SQRT_2).normalize();
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        Vec3CompNormMarshaler::marshal(&v, &mut wb);
        let bytes = wb.into_vec();
        // Flag byte + 2 Float16 = 5 bytes.
        assert_eq!(bytes.len(), 5);
        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bytes);
        let decoded = Vec3CompNormMarshaler::unmarshal(&mut rb).unwrap();
        assert!((decoded - v).length() < 0.001);
    }

    #[test]
    fn transform_compressor_identity_is_one_byte() {
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        TransformCompressor::marshal(&Affine3A::IDENTITY, &mut wb);
        assert_eq!(wb.into_vec(), [0x00]);
    }

    #[test]
    fn transform_compressor_translation_only() {
        let t = Affine3A::from_translation(Vec3::new(10.0, 20.0, 30.0));
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        TransformCompressor::marshal(&t, &mut wb);
        let bytes = wb.into_vec();
        // flag byte (0x04 = HAS_POS) + 3 raw f32 = 13 bytes.
        assert_eq!(bytes[0], 0x04);
        assert_eq!(bytes.len(), 13);
        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bytes);
        let decoded = TransformCompressor::unmarshal(&mut rb).unwrap();
        assert_eq!(Vec3::from(decoded.translation), Vec3::new(10.0, 20.0, 30.0));
    }

    #[test]
    fn packed_size_roundtrip_via_vlq() {
        let p = PackedSize::new(4, 3);
        assert_eq!(p.total_size_in_bits(), 35);
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        p.marshal(&mut wb);
        let bytes = wb.into_vec();
        // VlqU32(35) → 0x23 (single byte, < 0x80).
        assert_eq!(bytes, [0x23]);
        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bytes);
        let decoded = PackedSize::unmarshal(&mut rb).unwrap();
        assert_eq!(decoded.total_size_in_bits(), 35);
        assert_eq!(decoded.bytes(), 4);
        assert_eq!(decoded.additional_bits(), 3);
    }

    #[test]
    fn integer_quantization_u8_zero_to_one_range() {
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        IntegerQuantizationMarshalerU8::<0, 100>::marshal(&50, &mut wb);
        let bytes = wb.into_vec();
        assert_eq!(bytes.len(), 1);
        // 50/100 * 255 = 127.5 → 127 (cast-truncation).
        assert_eq!(bytes[0], 127);
        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bytes);
        let decoded = IntegerQuantizationMarshalerU8::<0, 100>::unmarshal(&mut rb).unwrap();
        // 127 * (100/255) ≈ 49.8 → truncates to 49 (within one bucket).
        assert!((decoded - 50).abs() <= 1);
    }

    #[test]
    fn integer_quantization_u16_signed_range() {
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        IntegerQuantizationMarshalerU16::<-1000, 1000>::marshal(&0, &mut wb);
        let bytes = wb.into_vec();
        assert_eq!(bytes.len(), 2);
        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bytes);
        let decoded = IntegerQuantizationMarshalerU16::<-1000, 1000>::unmarshal(&mut rb).unwrap();
        // Within one 16-bit bucket of zero.
        assert!(decoded.abs() <= 1);
    }
}
