use crate::serialize::marshaler::{Marshal, Unmarshal};
// DataMarshal equivalents (fundamental types, bool)

use super::{
    buffer::{ReadBuffer, WriteBuffer},
    container_marshal::{WIRE_VEC_CAP, marshal_wire_count},
    error::MarshalerError,
    marshaler::{Codec, Marshaler},
    vlq::VlqU32Marshaler,
};
use crate::source::DyeData;
use crate::types::{
    ActorRequestId, AfflictionData, AssetId, CharacterAttributeType, ComponentId, Crc32, EntityId,
    EntityRef, GatheringStatus, GdeId, GeneralCooldownType, PaperdollSlotAlias,
    RemoteServerContextRef, RemoteServerFacetRefGameModeParticipantComponentServerFacet,
    RemoteServerFacetRefHousingPlotComponentServerFacet, RemoteServerGdeRef,
    RemoteTypelessServerFacetRef, ReplicationCategory, TimePoint, WallClockTimePoint,
};
use nw_network_types::az::uuid::Uuid as AzUuid;
use std::marker::PhantomData;

impl Marshal for u8 {
    const MARSHAL_SIZE: usize = 1;

    #[inline]
    fn marshal(&self, wb: &mut WriteBuffer) {
        wb.write_u8(*self);
    }
}

impl Unmarshal for u8 {
    #[inline]
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        rb.read_u8()
    }
}

impl Marshal for i8 {
    const MARSHAL_SIZE: usize = 1;

    #[inline]
    fn marshal(&self, wb: &mut WriteBuffer) {
        wb.write_u8((*self).cast_unsigned());
    }
}

impl Unmarshal for i8 {
    #[inline]
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        Ok(rb.read_u8()?.cast_signed())
    }
}

impl Marshal for u16 {
    const MARSHAL_SIZE: usize = 2;

    #[inline]
    fn marshal(&self, wb: &mut WriteBuffer) {
        wb.write_u16(*self);
    }
}

impl Unmarshal for u16 {
    #[inline]
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        rb.read_u16()
    }
}

impl Marshal for i16 {
    const MARSHAL_SIZE: usize = 2;

    #[inline]
    fn marshal(&self, wb: &mut WriteBuffer) {
        wb.write_u16((*self).cast_unsigned());
    }
}

impl Unmarshal for i16 {
    #[inline]
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        Ok(rb.read_u16()?.cast_signed())
    }
}

impl Marshal for u32 {
    const MARSHAL_SIZE: usize = 4;

    #[inline]
    fn marshal(&self, wb: &mut WriteBuffer) {
        wb.write_u32(*self);
    }
}

impl Unmarshal for u32 {
    #[inline]
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let value = rb.read_u32()?;
        Ok(value)
    }
}

impl Marshal for i32 {
    const MARSHAL_SIZE: usize = 4;

    #[inline]
    fn marshal(&self, wb: &mut WriteBuffer) {
        wb.write_u32((*self).cast_unsigned());
    }
}

impl Unmarshal for i32 {
    #[inline]
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        Ok(rb.read_u32()?.cast_signed())
    }
}

impl Marshal for f32 {
    const MARSHAL_SIZE: usize = 4;

    #[inline]
    fn marshal(&self, wb: &mut WriteBuffer) {
        self.to_bits().marshal(wb);
    }
}

impl Unmarshal for f32 {
    #[inline]
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        Ok(f32::from_bits(u32::unmarshal(rb)?))
    }
}

impl Marshal for u64 {
    const MARSHAL_SIZE: usize = 8;

    #[inline]
    fn marshal(&self, wb: &mut WriteBuffer) {
        let bytes = match wb.endian() {
            super::buffer::Endian::BigEndian => self.to_be_bytes(),
            super::buffer::Endian::LittleEndian => self.to_le_bytes(),
        };
        wb.write_bytes(&bytes);
    }
}

impl Unmarshal for u64 {
    #[inline]
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let bytes = rb.read_bytes(8)?;
        let mut arr = [0u8; 8];
        arr.copy_from_slice(bytes);
        Ok(match rb.endian() {
            super::buffer::Endian::BigEndian => u64::from_be_bytes(arr),
            super::buffer::Endian::LittleEndian => u64::from_le_bytes(arr),
        })
    }
}

impl Marshal for EntityId {
    const MARSHAL_SIZE: usize = <u64 as Marshal>::MARSHAL_SIZE;

    #[inline]
    fn marshal(&self, wb: &mut WriteBuffer) {
        self.value().marshal(wb);
    }
}

impl Unmarshal for EntityId {
    #[inline]
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        Ok(Self::new(u64::unmarshal(rb)?))
    }
}

impl Marshal for ComponentId {
    const MARSHAL_SIZE: usize = <u64 as Marshal>::MARSHAL_SIZE;

    #[inline]
    fn marshal(&self, wb: &mut WriteBuffer) {
        self.value().marshal(wb);
    }
}

impl Unmarshal for ComponentId {
    #[inline]
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        Ok(Self::new(u64::unmarshal(rb)?))
    }
}

impl Marshal for GdeId {
    const MARSHAL_SIZE: usize = <u64 as Marshal>::MARSHAL_SIZE;

    #[inline]
    fn marshal(&self, wb: &mut WriteBuffer) {
        self.value().marshal(wb);
    }
}

impl Unmarshal for GdeId {
    #[inline]
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        Ok(Self::new(u64::unmarshal(rb)?))
    }
}

impl Marshal for ActorRequestId {
    fn marshal(&self, wb: &mut WriteBuffer) {
        self.target_local_id.marshal(wb);
        self.source_actor_ref.marshal(wb);
    }
}

impl Unmarshal for ActorRequestId {
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        Ok(Self {
            target_local_id: u64::unmarshal(rb)?,
            source_actor_ref: u64::unmarshal(rb)?,
        })
    }
}

impl Marshal for AzUuid {
    const MARSHAL_SIZE: usize = <uuid::Uuid as Marshal>::MARSHAL_SIZE;

    #[inline]
    fn marshal(&self, wb: &mut WriteBuffer) {
        wb.write_bytes(self.as_bytes());
    }
}

impl Unmarshal for AzUuid {
    #[inline]
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let bytes = rb.read_bytes(16)?;
        let mut raw = [0; 16];
        raw.copy_from_slice(bytes);
        Ok(Self::from_bytes(raw))
    }
}

impl Marshal for RemoteServerContextRef {
    fn marshal(&self, wb: &mut WriteBuffer) {
        self.actor_id.marshal(wb);
    }
}

impl Unmarshal for RemoteServerContextRef {
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        Ok(Self {
            actor_id: AzUuid::unmarshal(rb)?,
        })
    }
}

impl Marshal for RemoteServerGdeRef {
    fn marshal(&self, wb: &mut WriteBuffer) {
        self.remote_server_context.marshal(wb);
        self.target_id.marshal(wb);
    }
}

impl Unmarshal for RemoteServerGdeRef {
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        Ok(Self {
            remote_server_context: RemoteServerContextRef::unmarshal(rb)?,
            target_id: GdeId::unmarshal(rb)?,
        })
    }
}

/// Replicated-field codec that reconstructs a server GDE reference from its
/// actor UUID. The target ID is the UUID's native low 64-bit word.
#[derive(Debug, Clone, Copy, Default)]
pub struct RemoteServerGdeRefMarshaler;

impl Codec<RemoteServerGdeRef> for RemoteServerGdeRefMarshaler {
    const MARSHAL_SIZE: usize = 16;

    #[inline]
    fn marshal(value: &RemoteServerGdeRef, wb: &mut WriteBuffer) {
        value.remote_server_context.actor_id.marshal(wb);
    }

    #[inline]
    fn unmarshal(rb: &mut ReadBuffer) -> Result<RemoteServerGdeRef, MarshalerError> {
        let actor_id = AzUuid::unmarshal(rb)?;
        let mut low = [0; 8];
        low.copy_from_slice(&actor_id.as_bytes()[..8]);
        Ok(RemoteServerGdeRef {
            remote_server_context: RemoteServerContextRef { actor_id },
            target_id: GdeId::new(u64::from_le_bytes(low)),
        })
    }
}

impl Marshal for RemoteTypelessServerFacetRef {
    fn marshal(&self, wb: &mut WriteBuffer) {
        self.remote_server_gde_ref.marshal(wb);
        self.target_id.marshal(wb);
    }
}

impl Unmarshal for RemoteTypelessServerFacetRef {
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        Ok(Self {
            remote_server_gde_ref: RemoteServerGdeRef::unmarshal(rb)?,
            target_id: u64::unmarshal(rb)?,
        })
    }
}

impl Marshal for RemoteServerFacetRefGameModeParticipantComponentServerFacet {
    fn marshal(&self, wb: &mut WriteBuffer) {
        self.typeless_ref.marshal(wb);
    }
}

impl Unmarshal for RemoteServerFacetRefGameModeParticipantComponentServerFacet {
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        Ok(Self {
            typeless_ref: RemoteTypelessServerFacetRef::unmarshal(rb)?,
        })
    }
}

impl Marshal for RemoteServerFacetRefHousingPlotComponentServerFacet {
    fn marshal(&self, wb: &mut WriteBuffer) {
        self.typeless_ref.marshal(wb);
    }
}

impl Unmarshal for RemoteServerFacetRefHousingPlotComponentServerFacet {
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        Ok(Self {
            typeless_ref: RemoteTypelessServerFacetRef::unmarshal(rb)?,
        })
    }
}

impl Marshal for i64 {
    const MARSHAL_SIZE: usize = 8;

    #[inline]
    fn marshal(&self, wb: &mut WriteBuffer) {
        (*self).cast_unsigned().marshal(wb);
    }
}

impl Unmarshal for i64 {
    #[inline]
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        Ok(u64::unmarshal(rb)?.cast_signed())
    }
}

impl Marshal for f64 {
    const MARSHAL_SIZE: usize = 8;

    #[inline]
    fn marshal(&self, wb: &mut WriteBuffer) {
        self.to_bits().marshal(wb);
    }
}

impl Unmarshal for f64 {
    #[inline]
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        Ok(f64::from_bits(u64::unmarshal(rb)?))
    }
}

impl Marshal for bool {
    const MARSHAL_SIZE: usize = 1;

    #[inline]
    fn marshal(&self, wb: &mut WriteBuffer) {
        wb.write_raw(*self);
    }
}

impl Unmarshal for bool {
    #[inline]
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        rb.read_raw()
    }
}

/// Converts a value to and from the representation used by a [`ConversionMarshaler`].
pub trait MarshalerConversion<SerializedType>: Copy {
    fn to_serialized(self) -> SerializedType;

    fn try_from_serialized(value: SerializedType) -> Result<Self, MarshalerError>;
}

/// Encodes a value through a different serialized representation.
#[derive(Debug, Clone, Copy, Default)]
pub struct ConversionMarshaler<SerializedType, OriginalType>(
    PhantomData<fn() -> (SerializedType, OriginalType)>,
);

impl<SerializedType, OriginalType> Codec<OriginalType>
    for ConversionMarshaler<SerializedType, OriginalType>
where
    SerializedType: Marshaler,
    OriginalType: MarshalerConversion<SerializedType>,
{
    const MARSHAL_SIZE: usize = SerializedType::MARSHAL_SIZE;

    fn marshal(value: &OriginalType, wb: &mut WriteBuffer) {
        (*value).to_serialized().marshal(wb);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<OriginalType, MarshalerError> {
        OriginalType::try_from_serialized(SerializedType::unmarshal(rb)?)
    }
}

/// A CRC-32 value is carried as one raw `u32`.
impl Marshal for Crc32 {
    const MARSHAL_SIZE: usize = <u32 as Marshal>::MARSHAL_SIZE;

    fn marshal(&self, wb: &mut WriteBuffer) {
        self.value().marshal(wb);
    }
}

impl Unmarshal for Crc32 {
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        Ok(Self::new(u32::unmarshal(rb)?))
    }
}

impl Marshal for AssetId {
    const MARSHAL_SIZE: usize = <AzUuid as Marshal>::MARSHAL_SIZE + <u32 as Marshal>::MARSHAL_SIZE;

    fn marshal(&self, wb: &mut WriteBuffer) {
        self.guid.marshal(wb);
        self.sub_id.marshal(wb);
    }
}

impl Unmarshal for AssetId {
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        Ok(Self {
            guid: AzUuid::unmarshal(rb)?,
            sub_id: u32::unmarshal(rb)?,
        })
    }
}

/// A game time point is carried as one raw nanosecond `u64`.
impl Marshal for TimePoint {
    const MARSHAL_SIZE: usize = <u64 as Marshal>::MARSHAL_SIZE;

    fn marshal(&self, wb: &mut WriteBuffer) {
        self.nanoseconds_since_server_start.marshal(wb);
    }
}

impl Unmarshal for TimePoint {
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        Ok(Self {
            nanoseconds_since_server_start: u64::unmarshal(rb)?,
        })
    }
}

/// A wall-clock time point is carried as one raw nanosecond `u64`.
impl Marshal for WallClockTimePoint {
    const MARSHAL_SIZE: usize = <u64 as Marshal>::MARSHAL_SIZE;

    fn marshal(&self, wb: &mut WriteBuffer) {
        self.nanoseconds_since_epoc.marshal(wb);
    }
}

impl Unmarshal for WallClockTimePoint {
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        Ok(Self {
            nanoseconds_since_epoc: u64::unmarshal(rb)?,
        })
    }
}

impl Marshal for AfflictionData {
    fn marshal(&self, wb: &mut WriteBuffer) {
        self.last_amount.marshal(wb);
        self.last_amount_time_point.marshal(wb);
        self.target_amount.marshal(wb);
        self.target_amount_time_point.marshal(wb);
        self.max.marshal(wb);
        self.affliction_id.marshal(wb);
        self.is_afflicted.marshal(wb);
    }
}

impl Unmarshal for AfflictionData {
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        Ok(Self {
            last_amount: f32::unmarshal(rb)?,
            last_amount_time_point: TimePoint::unmarshal(rb)?,
            target_amount: f32::unmarshal(rb)?,
            target_amount_time_point: TimePoint::unmarshal(rb)?,
            max: f32::unmarshal(rb)?,
            affliction_id: i8::unmarshal(rb)?,
            is_afflicted: bool::unmarshal(rb)?,
        })
    }
}

impl Marshal for DyeData {
    const MARSHAL_SIZE: usize = 4;

    fn marshal(&self, wb: &mut WriteBuffer) {
        self.r_color_id.marshal(wb);
        self.g_color_id.marshal(wb);
        self.b_color_id.marshal(wb);
        self.a_color_id.marshal(wb);
    }
}

impl Unmarshal for DyeData {
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        Ok(Self {
            r_color_id: u8::unmarshal(rb)?,
            g_color_id: u8::unmarshal(rb)?,
            b_color_id: u8::unmarshal(rb)?,
            a_color_id: u8::unmarshal(rb)?,
        })
    }
}

impl Marshal for CharacterAttributeType {
    const MARSHAL_SIZE: usize = <i32 as Marshal>::MARSHAL_SIZE;

    fn marshal(&self, wb: &mut WriteBuffer) {
        i32::from(*self).marshal(wb);
    }
}

impl Unmarshal for CharacterAttributeType {
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let value = i32::unmarshal(rb)?;
        Self::try_from(value).map_err(|_| MarshalerError::InvalidRange {
            value: value.cast_unsigned().into(),
            min: 0,
            max: 4,
        })
    }
}

impl Marshal for GeneralCooldownType {
    const MARSHAL_SIZE: usize = <i32 as Marshal>::MARSHAL_SIZE;

    fn marshal(&self, wb: &mut WriteBuffer) {
        i32::from(*self).marshal(wb);
    }
}

impl Unmarshal for GeneralCooldownType {
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let value = i32::unmarshal(rb)?;
        Self::try_from(value).map_err(|_| MarshalerError::InvalidRange {
            value: value.cast_unsigned().into(),
            min: 0,
            max: 2,
        })
    }
}

impl Marshal for GatheringStatus {
    const MARSHAL_SIZE: usize = <i32 as Marshal>::MARSHAL_SIZE;

    fn marshal(&self, wb: &mut WriteBuffer) {
        i32::from(*self).marshal(wb);
    }
}

impl Unmarshal for GatheringStatus {
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let value = i32::unmarshal(rb)?;
        Self::try_from(value).map_err(|_| MarshalerError::InvalidRange {
            value: value.cast_unsigned().into(),
            min: 0,
            max: 3,
        })
    }
}

impl Marshal for PaperdollSlotAlias {
    const MARSHAL_SIZE: usize = <i32 as Marshal>::MARSHAL_SIZE;

    fn marshal(&self, wb: &mut WriteBuffer) {
        i32::from(*self).marshal(wb);
    }
}

impl Unmarshal for PaperdollSlotAlias {
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let value = i32::unmarshal(rb)?;
        Self::try_from(value).map_err(|_| MarshalerError::InvalidRange {
            value: value.cast_unsigned().into(),
            min: 0,
            max: 60,
        })
    }
}

impl Marshal for ReplicationCategory {
    const MARSHAL_SIZE: usize = <u8 as Marshal>::MARSHAL_SIZE;

    fn marshal(&self, wb: &mut WriteBuffer) {
        u8::from(*self).marshal(wb);
    }
}

impl Unmarshal for ReplicationCategory {
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let value = u8::unmarshal(rb)?;
        Self::try_from(value).map_err(|_| MarshalerError::InvalidRange {
            value: value.into(),
            min: 0,
            max: 6,
        })
    }
}

impl Marshal for EntityRef {
    fn marshal(&self, wb: &mut WriteBuffer) {
        match self {
            EntityRef::String(value) => {
                0u8.marshal(wb);
                value.marshal(wb);
            }
            EntityRef::Uuid { uuid, format_flags } => {
                (0x01 | (format_flags << 1)).marshal(wb);
                wb.write_bytes(uuid.as_bytes());
            }
        }
    }
}

impl Unmarshal for EntityRef {
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let flags = u8::unmarshal(rb)?;
        if (flags & 1) == 0 {
            return Ok(Self::String(String::unmarshal(rb)?));
        }

        let bytes = rb.read_bytes(16)?;
        Ok(Self::Uuid {
            uuid: uuid::Uuid::from_slice(bytes)?,
            format_flags: (flags >> 1) & 0x07,
        })
    }
}

impl Marshal for String {
    #[inline]
    fn marshal(&self, wb: &mut WriteBuffer) {
        let bytes = self.as_bytes();
        marshal_wire_count(wb, bytes.len());
        wb.write_bytes(bytes);
    }
}

impl Unmarshal for String {
    /// Length is bounded by [`WIRE_VEC_CAP`] on read; the wire ceiling
    /// applies the same way to `String` byte length as it does to `Vec<T>`
    #[inline]
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let len = VlqU32Marshaler.unmarshal(rb)? as usize;
        if len > WIRE_VEC_CAP {
            return Err(MarshalerError::ContainerOverflow {
                len,
                capacity: WIRE_VEC_CAP,
            });
        }
        let bytes = rb.read_bytes(len)?;
        Ok(std::str::from_utf8(bytes)?.to_string())
    }
}

/// Marker for primitive scalar types with direct marshaler support.
pub trait FundamentalMarshalType: Marshaler {}

impl FundamentalMarshalType for f32 {}
impl FundamentalMarshalType for f64 {}
impl FundamentalMarshalType for u8 {}
impl FundamentalMarshalType for u16 {}
impl FundamentalMarshalType for u32 {}
impl FundamentalMarshalType for u64 {}
impl FundamentalMarshalType for i8 {}
impl FundamentalMarshalType for i16 {}
impl FundamentalMarshalType for i32 {}
impl FundamentalMarshalType for i64 {}

pub struct IsFundamentalMarshalType<T>(PhantomData<fn() -> T>);

impl<T: FundamentalMarshalType> IsFundamentalMarshalType<T> {
    pub const VALUE: bool = true;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serialize::buffer::{CARRIER_ENDIAN, ReadBuffer, WriteBuffer};

    fn roundtrip<T: Marshaler + PartialEq + std::fmt::Debug>(value: &T) -> T {
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        value.marshal(&mut wb);
        let data = wb.into_vec();
        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &data);
        T::unmarshal(&mut rb).expect("unmarshal should succeed")
    }

    #[test]
    fn test_u8_roundtrip() {
        assert_eq!(roundtrip(&0u8), 0u8);
        assert_eq!(roundtrip(&127u8), 127u8);
        assert_eq!(roundtrip(&255u8), 255u8);
    }

    #[test]
    fn test_u16_roundtrip() {
        assert_eq!(roundtrip(&0u16), 0u16);
        assert_eq!(roundtrip(&1234u16), 1234u16);
        assert_eq!(roundtrip(&u16::MAX), u16::MAX);
    }

    #[test]
    fn test_u32_roundtrip() {
        assert_eq!(roundtrip(&0u32), 0u32);
        assert_eq!(roundtrip(&123_456_u32), 123_456_u32);
        assert_eq!(roundtrip(&u32::MAX), u32::MAX);
    }

    #[test]
    fn test_u64_roundtrip() {
        assert_eq!(roundtrip(&0u64), 0u64);
        assert_eq!(roundtrip(&123_456_789_012_345_u64), 123_456_789_012_345_u64);
        assert_eq!(roundtrip(&u64::MAX), u64::MAX);
    }

    #[test]
    fn test_f32_roundtrip() {
        assert_eq!(roundtrip(&0.0f32).to_bits(), 0.0f32.to_bits());
        assert_eq!(
            roundtrip(&std::f32::consts::PI).to_bits(),
            std::f32::consts::PI.to_bits()
        );
        assert_eq!(roundtrip(&-1.5f32).to_bits(), (-1.5f32).to_bits());
    }

    #[test]
    fn test_f64_roundtrip() {
        assert_eq!(roundtrip(&0.0f64).to_bits(), 0.0f64.to_bits());
        assert_eq!(
            roundtrip(&std::f64::consts::PI).to_bits(),
            std::f64::consts::PI.to_bits()
        );
        assert_eq!(roundtrip(&-1.5f64).to_bits(), (-1.5f64).to_bits());
        assert_eq!(roundtrip(&f64::MIN).to_bits(), f64::MIN.to_bits());
        assert_eq!(roundtrip(&f64::MAX).to_bits(), f64::MAX.to_bits());
    }

    /// Wire form of `f64` is the IEEE-754 64-bit bit pattern in carrier
    /// endian — i.e. the same 8 bytes that `u64::marshal` would emit for
    /// `f64::to_bits()`. Locks that in.
    #[test]
    fn test_f64_wire_matches_u64_bits() {
        let value = 1.5f64;
        let mut wb1 = WriteBuffer::new(CARRIER_ENDIAN);
        value.marshal(&mut wb1);
        let mut wb2 = WriteBuffer::new(CARRIER_ENDIAN);
        value.to_bits().marshal(&mut wb2);
        assert_eq!(wb1.into_vec(), wb2.into_vec());
    }

    #[test]
    fn test_bool_roundtrip() {
        assert!(roundtrip(&true));
        assert!(!roundtrip(&false));
    }

    #[test]
    fn test_string_roundtrip() {
        assert_eq!(roundtrip(&String::new()), String::new());
        assert_eq!(roundtrip(&"hello".to_string()), "hello".to_string());
        assert_eq!(
            roundtrip(&"Hello, World! 🌍".to_string()),
            "Hello, World! 🌍".to_string()
        );

        // Test longer string
        let long_string = "a".repeat(1000);
        assert_eq!(roundtrip(&long_string), long_string);
    }

    #[test]
    fn test_string_marshal_bytes() {
        // Verify the string bytes are actually written (not just length 0)
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        "test".to_string().marshal(&mut wb);
        let data = wb.into_vec();

        // Should be: 1 byte VLQ length (4) + 4 bytes content
        assert_eq!(data.len(), 5);
        assert_eq!(data[0], 4); // VLQ encoded length 4
        assert_eq!(&data[1..], b"test");
    }

    #[test]
    fn remote_server_gde_ref_field_codec_derives_target_id_from_actor_uuid() {
        let actor_id = AzUuid::from_bytes([
            0x67, 0xa3, 0x4a, 0xd4, 0x1e, 0x58, 0xb1, 0x95, 0, 0, 0, 0, 0, 0, 0, 0,
        ]);
        let value = RemoteServerGdeRef {
            remote_server_context: RemoteServerContextRef { actor_id },
            target_id: GdeId::new(0),
        };

        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        RemoteServerGdeRefMarshaler::marshal(&value, &mut wb);
        let bytes = wb.into_vec();
        assert_eq!(bytes, actor_id.as_bytes());

        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bytes);
        let decoded = RemoteServerGdeRefMarshaler::unmarshal(&mut rb).expect("decode");
        assert_eq!(decoded.remote_server_context.actor_id, actor_id);
        assert_eq!(decoded.target_id.value(), 0x95b1_581e_d44a_a367);
        assert!(rb.remaining().is_empty());
    }
}
