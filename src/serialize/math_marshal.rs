//! Wire marshalers for AZ math types as supplied by Bevy / glam.
//!
//! Wire format: each scalar component as one f32, in declaration order.

use super::{
    buffer::{ReadBuffer, WriteBuffer},
    error::MarshalerError,
    marshaler::Marshaler,
};
use bevy_math::bounding::{Aabb2d, Aabb3d};
use glam::{Affine3A, Mat3, Mat3A, Quat, Vec2, Vec3, Vec3A, Vec4};

impl Marshaler for Vec2 {
    const MARSHAL_SIZE: usize = 8;

    fn marshal(&self, wb: &mut WriteBuffer) {
        self.x.marshal(wb);
        self.y.marshal(wb);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let x = f32::unmarshal(rb)?;
        let y = f32::unmarshal(rb)?;
        Ok(Vec2::new(x, y))
    }
}

/// `AZ::Bounds` wire layout: two raw `AZ::Vector2` values, min then max.
impl Marshaler for Aabb2d {
    const MARSHAL_SIZE: usize = 16;

    fn marshal(&self, wb: &mut WriteBuffer) {
        self.min.marshal(wb);
        self.max.marshal(wb);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        Ok(Self {
            min: Vec2::unmarshal(rb)?,
            max: Vec2::unmarshal(rb)?,
        })
    }
}

impl Marshaler for Vec3 {
    const MARSHAL_SIZE: usize = 12;

    fn marshal(&self, wb: &mut WriteBuffer) {
        self.x.marshal(wb);
        self.y.marshal(wb);
        self.z.marshal(wb);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let x = f32::unmarshal(rb)?;
        let y = f32::unmarshal(rb)?;
        let z = f32::unmarshal(rb)?;
        Ok(Vec3::new(x, y, z))
    }
}

impl Marshaler for Vec4 {
    const MARSHAL_SIZE: usize = 16;

    fn marshal(&self, wb: &mut WriteBuffer) {
        self.x.marshal(wb);
        self.y.marshal(wb);
        self.z.marshal(wb);
        self.w.marshal(wb);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let x = f32::unmarshal(rb)?;
        let y = f32::unmarshal(rb)?;
        let z = f32::unmarshal(rb)?;
        let w = f32::unmarshal(rb)?;
        Ok(Vec4::new(x, y, z, w))
    }
}

/// variant). For the compressed normalized form see
/// [`super::compression_marshal::QuatCompNorm`].
impl Marshaler for Quat {
    const MARSHAL_SIZE: usize = 16;

    fn marshal(&self, wb: &mut WriteBuffer) {
        self.x.marshal(wb);
        self.y.marshal(wb);
        self.z.marshal(wb);
        self.w.marshal(wb);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let x = f32::unmarshal(rb)?;
        let y = f32::unmarshal(rb)?;
        let z = f32::unmarshal(rb)?;
        let w = f32::unmarshal(rb)?;
        Ok(Quat::from_xyzw(x, y, z, w))
    }
}

/// `GetBasisX/Y/Z` in order. glam's `Mat3` stores basis vectors as
/// `x_axis`, `y_axis`, `z_axis` so the on-wire order matches one-to-one.
impl Marshaler for Mat3 {
    const MARSHAL_SIZE: usize = 36;

    fn marshal(&self, wb: &mut WriteBuffer) {
        self.x_axis.marshal(wb);
        self.y_axis.marshal(wb);
        self.z_axis.marshal(wb);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let x_axis = Vec3::unmarshal(rb)?;
        let y_axis = Vec3::unmarshal(rb)?;
        let z_axis = Vec3::unmarshal(rb)?;
        Ok(Mat3::from_cols(x_axis, y_axis, z_axis))
    }
}

/// equivalent SIMD-aligned affine; we round-trip through `Vec3` for the
impl Marshaler for Affine3A {
    const MARSHAL_SIZE: usize = 48;

    fn marshal(&self, wb: &mut WriteBuffer) {
        Vec3::from(self.matrix3.x_axis).marshal(wb);
        Vec3::from(self.matrix3.y_axis).marshal(wb);
        Vec3::from(self.matrix3.z_axis).marshal(wb);
        Vec3::from(self.translation).marshal(wb);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let x_axis = Vec3::unmarshal(rb)?;
        let y_axis = Vec3::unmarshal(rb)?;
        let z_axis = Vec3::unmarshal(rb)?;
        let translation = Vec3::unmarshal(rb)?;
        Ok(Affine3A {
            matrix3: Mat3A::from_cols(
                Vec3A::from(x_axis),
                Vec3A::from(y_axis),
                Vec3A::from(z_axis),
            ),
            translation: Vec3A::from(translation),
        })
    }
}

/// with only 6 floats on the wire. We use Bevy's `Aabb3d` (whose `Vec3A`
/// fields are themselves SIMD-aligned) for the semantic type.
impl Marshaler for Aabb3d {
    const MARSHAL_SIZE: usize = 24;

    fn marshal(&self, wb: &mut WriteBuffer) {
        Vec3::from(self.min).marshal(wb);
        Vec3::from(self.max).marshal(wb);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let min = Vec3::unmarshal(rb)?;
        let max = Vec3::unmarshal(rb)?;
        Ok(Aabb3d {
            min: Vec3A::from(min),
            max: Vec3A::from(max),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serialize::buffer::Endian;

    #[test]
    fn aabb3d_marshal_roundtrip() {
        let aabb = Aabb3d {
            min: Vec3A::new(1.0, 2.0, 3.0),
            max: Vec3A::new(4.0, 5.0, 6.0),
        };
        let mut wb = WriteBuffer::new(Endian::BigEndian);
        aabb.marshal(&mut wb);
        let bytes = wb.into_vec();
        assert_eq!(bytes.len(), 24);
        let mut rb = ReadBuffer::new(Endian::BigEndian, &bytes);
        let decoded = Aabb3d::unmarshal(&mut rb).unwrap();
        assert_eq!(decoded.min, aabb.min);
        assert_eq!(decoded.max, aabb.max);
    }

    #[test]
    fn bounds_marshal_is_two_vec2_16_bytes() {
        let bounds = Aabb2d {
            min: Vec2::new(1.0, 2.0),
            max: Vec2::new(3.0, 4.0),
        };
        let mut wb = WriteBuffer::new(Endian::BigEndian);
        bounds.marshal(&mut wb);
        let bytes = wb.into_vec();
        assert_eq!(bytes.len(), 16);
        let mut rb = ReadBuffer::new(Endian::BigEndian, &bytes);
        let decoded = Aabb2d::unmarshal(&mut rb).unwrap();
        assert_eq!(decoded, bounds);
    }

    #[test]
    fn vec4_marshal_is_four_f32_16_bytes() {
        let value = Vec4::new(1.0, -2.0, 3.5, 4.25);
        let mut wb = WriteBuffer::new(Endian::BigEndian);
        value.marshal(&mut wb);
        let bytes = wb.into_vec();
        assert_eq!(bytes.len(), 16);

        let mut rb = ReadBuffer::new(Endian::BigEndian, &bytes);
        let decoded = Vec4::unmarshal(&mut rb).unwrap();
        assert_eq!(decoded, value);
        assert_eq!(rb.left(), 0);
    }

    #[test]
    fn quat_marshal_is_xyzw_16_bytes() {
        let q = Quat::from_xyzw(1.0, 2.0, 3.0, 4.0);
        let mut wb = WriteBuffer::new(Endian::BigEndian);
        q.marshal(&mut wb);
        let bytes = wb.into_vec();
        assert_eq!(bytes.len(), 16);
        let mut rb = ReadBuffer::new(Endian::BigEndian, &bytes);
        let decoded = Quat::unmarshal(&mut rb).unwrap();
        assert_eq!(decoded, q);
        // Wire order is x, y, z, w — big-endian f32(1.0) = 0x3f800000.
        assert_eq!(&bytes[0..4], &1.0f32.to_be_bytes());
        assert_eq!(&bytes[12..16], &4.0f32.to_be_bytes());
    }

    #[test]
    fn mat3_marshal_is_three_basis_36_bytes() {
        let m = Mat3::from_cols(
            Vec3::new(1.0, 2.0, 3.0),
            Vec3::new(4.0, 5.0, 6.0),
            Vec3::new(7.0, 8.0, 9.0),
        );
        let mut wb = WriteBuffer::new(Endian::BigEndian);
        m.marshal(&mut wb);
        let bytes = wb.into_vec();
        assert_eq!(bytes.len(), 36);
        let mut rb = ReadBuffer::new(Endian::BigEndian, &bytes);
        let decoded = Mat3::unmarshal(&mut rb).unwrap();
        assert_eq!(decoded.x_axis, m.x_axis);
        assert_eq!(decoded.y_axis, m.y_axis);
        assert_eq!(decoded.z_axis, m.z_axis);
    }

    #[test]
    fn affine3a_marshal_is_basis_plus_translation_48_bytes() {
        let t = Affine3A {
            matrix3: Mat3A::from_cols(
                Vec3A::new(1.0, 0.0, 0.0),
                Vec3A::new(0.0, 1.0, 0.0),
                Vec3A::new(0.0, 0.0, 1.0),
            ),
            translation: Vec3A::new(10.0, 20.0, 30.0),
        };
        let mut wb = WriteBuffer::new(Endian::BigEndian);
        t.marshal(&mut wb);
        let bytes = wb.into_vec();
        assert_eq!(bytes.len(), 48);
        let mut rb = ReadBuffer::new(Endian::BigEndian, &bytes);
        let decoded = Affine3A::unmarshal(&mut rb).unwrap();
        // Compare via Vec3 for cross-Vec3A alignment correctness.
        assert_eq!(Vec3::from(decoded.matrix3.x_axis), Vec3::X);
        assert_eq!(Vec3::from(decoded.matrix3.y_axis), Vec3::Y);
        assert_eq!(Vec3::from(decoded.matrix3.z_axis), Vec3::Z);
        assert_eq!(Vec3::from(decoded.translation), Vec3::new(10.0, 20.0, 30.0));
    }
}
