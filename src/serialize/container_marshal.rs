//! Container codecs.
//!
//! Default Rust containers use VLQ32 lengths. The explicit
//! [`ContainerMarshaler`] and [`MapContainerMarshaler`] policies use raw
//! `u16` lengths. Elements are serialized in iteration order; use
//! [`IndexMap`] or [`IndexSet`] when byte order needs to be deterministic.

use super::{
    buffer::{ReadBuffer, WriteBuffer},
    error::MarshalerError,
    marshaler::{Codec, DefaultMarshaler, Marshaler},
    vlq::VlqU32Marshaler,
};
use arrayvec::{ArrayString, ArrayVec};
use indexmap::{IndexMap, IndexSet};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::hash::{BuildHasher, Hash};
use std::marker::PhantomData;

/// Universal upper bound on `Vec<T>` and `String` element/byte counts on the
/// wire: `0x2000000` (~33.5M).
///
/// This prevents malformed VLQ counts from triggering impractically large
/// allocations during unmarshal.
///
/// Per-field tighter caps are expressed at the type level with
/// `ArrayVec<T, N>` or `ArrayString<N>`. Those types keep the protocol limit in
/// the public API instead of allowing values to grow past the declared cap.
pub const WIRE_VEC_CAP: usize = 0x0200_0000;

pub(crate) fn marshal_wire_count(wb: &mut WriteBuffer, len: usize) {
    debug_assert!(
        len <= WIRE_VEC_CAP,
        "wire container count exceeds configured cap"
    );
    let len = u32::try_from(len).expect("wire container count exceeds u32");
    VlqU32Marshaler.marshal(wb, len);
}

/// Raw `u16` element count followed by each element through the selected inner
/// marshaler.
#[derive(Debug, Clone, Copy, Default)]
pub struct ContainerMarshaler<T, M = DefaultMarshaler<T>>(PhantomData<fn() -> (T, M)>);

impl<T, M> ContainerMarshaler<T, M>
where
    M: Codec<T>,
{
    fn marshal_len(wb: &mut WriteBuffer, len: usize) {
        let len = u16::try_from(len).expect("container count exceeds u16");
        len.marshal(wb);
    }

    fn unmarshal_len(rb: &mut ReadBuffer) -> Result<usize, MarshalerError> {
        Ok(u16::unmarshal(rb)? as usize)
    }
}

impl<T, M> Codec<Vec<T>> for ContainerMarshaler<T, M>
where
    M: Codec<T>,
{
    fn marshal(value: &Vec<T>, wb: &mut WriteBuffer) {
        Self::marshal_len(wb, value.len());
        for item in value {
            M::marshal(item, wb);
        }
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Vec<T>, MarshalerError> {
        let len = Self::unmarshal_len(rb)?;
        let mut value = Vec::with_capacity(len);
        for _ in 0..len {
            value.push(M::unmarshal(rb)?);
        }
        Ok(value)
    }
}

impl<T, M, const N: usize> Codec<ArrayVec<T, N>> for ContainerMarshaler<T, M>
where
    M: Codec<T>,
{
    fn marshal(value: &ArrayVec<T, N>, wb: &mut WriteBuffer) {
        Self::marshal_len(wb, value.len());
        for item in value {
            M::marshal(item, wb);
        }
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<ArrayVec<T, N>, MarshalerError> {
        let len = Self::unmarshal_len(rb)?;
        if len > N {
            return Err(MarshalerError::ContainerOverflow { len, capacity: N });
        }
        let mut value = ArrayVec::new();
        for _ in 0..len {
            value.push(M::unmarshal(rb)?);
        }
        Ok(value)
    }
}

impl Codec<String> for ContainerMarshaler<u8> {
    fn marshal(value: &String, wb: &mut WriteBuffer) {
        let bytes = value.as_bytes();
        Self::marshal_len(wb, bytes.len());
        wb.write_bytes(bytes);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<String, MarshalerError> {
        let len = Self::unmarshal_len(rb)?;
        let bytes = rb.read_bytes(len)?;
        Ok(std::str::from_utf8(bytes)?.to_string())
    }
}

impl<const N: usize> Codec<ArrayString<N>> for ContainerMarshaler<u8> {
    fn marshal(value: &ArrayString<N>, wb: &mut WriteBuffer) {
        Self::marshal_len(wb, value.len());
        wb.write_bytes(value.as_bytes());
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<ArrayString<N>, MarshalerError> {
        let len = Self::unmarshal_len(rb)?;
        if len > N {
            return Err(MarshalerError::StringOverflow { len, capacity: N });
        }
        let bytes = rb.read_bytes(len)?;
        let s = std::str::from_utf8(bytes)?;
        let mut value = ArrayString::new();
        value
            .try_push_str(s)
            .map_err(|_| MarshalerError::StringOverflow { len, capacity: N })?;
        Ok(value)
    }
}

impl<T, M> Codec<IndexSet<T>> for ContainerMarshaler<T, M>
where
    T: Eq + Hash,
    M: Codec<T>,
{
    fn marshal(value: &IndexSet<T>, wb: &mut WriteBuffer) {
        Self::marshal_len(wb, value.len());
        for item in value {
            M::marshal(item, wb);
        }
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<IndexSet<T>, MarshalerError> {
        let len = Self::unmarshal_len(rb)?;
        let mut value = IndexSet::with_capacity(len);
        for _ in 0..len {
            value.insert(M::unmarshal(rb)?);
        }
        Ok(value)
    }
}

impl<T, M, S> Codec<HashSet<T, S>> for ContainerMarshaler<T, M>
where
    T: Eq + Hash,
    M: Codec<T>,
    S: BuildHasher + Default,
{
    fn marshal(value: &HashSet<T, S>, wb: &mut WriteBuffer) {
        Self::marshal_len(wb, value.len());
        for item in value {
            M::marshal(item, wb);
        }
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<HashSet<T, S>, MarshalerError> {
        let len = Self::unmarshal_len(rb)?;
        let mut value = HashSet::with_capacity_and_hasher(len, S::default());
        for _ in 0..len {
            value.insert(M::unmarshal(rb)?);
        }
        Ok(value)
    }
}

impl<T, M> Codec<BTreeSet<T>> for ContainerMarshaler<T, M>
where
    T: Ord,
    M: Codec<T>,
{
    fn marshal(value: &BTreeSet<T>, wb: &mut WriteBuffer) {
        Self::marshal_len(wb, value.len());
        for item in value {
            M::marshal(item, wb);
        }
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<BTreeSet<T>, MarshalerError> {
        let len = Self::unmarshal_len(rb)?;
        let mut value = BTreeSet::new();
        for _ in 0..len {
            value.insert(M::unmarshal(rb)?);
        }
        Ok(value)
    }
}

impl<T, M, const N: usize> Codec<[T; N]> for ContainerMarshaler<T, M>
where
    M: Codec<T>,
{
    fn marshal(value: &[T; N], wb: &mut WriteBuffer) {
        Self::marshal_len(wb, N);
        for item in value {
            M::marshal(item, wb);
        }
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<[T; N], MarshalerError> {
        let len = Self::unmarshal_len(rb)?;
        if len != N {
            return Err(MarshalerError::ContainerOverflow { len, capacity: N });
        }
        let mut value = Vec::with_capacity(N);
        for _ in 0..N {
            value.push(M::unmarshal(rb)?);
        }
        value
            .try_into()
            .map_err(|value: Vec<T>| MarshalerError::ContainerOverflow {
                len: value.len(),
                capacity: N,
            })
    }
}

type MapContainerMarker<K, V, KM, VM> = fn() -> (K, V, KM, VM);

/// Encodes a raw `u16` entry count followed by key/value pairs through their
/// configured marshalers.
#[derive(Debug, Clone, Copy, Default)]
pub struct MapContainerMarshaler<K, V, KM = DefaultMarshaler<K>, VM = DefaultMarshaler<V>>(
    PhantomData<MapContainerMarker<K, V, KM, VM>>,
);

impl<K, V, KM, VM> MapContainerMarshaler<K, V, KM, VM>
where
    KM: Codec<K>,
    VM: Codec<V>,
{
    fn marshal_len(wb: &mut WriteBuffer, len: usize) {
        let len = u16::try_from(len).expect("map container count exceeds u16");
        len.marshal(wb);
    }

    fn unmarshal_len(rb: &mut ReadBuffer) -> Result<usize, MarshalerError> {
        Ok(u16::unmarshal(rb)? as usize)
    }
}

impl<K, V, KM, VM, S> Codec<HashMap<K, V, S>> for MapContainerMarshaler<K, V, KM, VM>
where
    K: Eq + Hash,
    KM: Codec<K>,
    VM: Codec<V>,
    S: BuildHasher + Default,
{
    fn marshal(value: &HashMap<K, V, S>, wb: &mut WriteBuffer) {
        Self::marshal_len(wb, value.len());
        for (key, item) in value {
            KM::marshal(key, wb);
            VM::marshal(item, wb);
        }
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<HashMap<K, V, S>, MarshalerError> {
        let len = Self::unmarshal_len(rb)?;
        let mut value = HashMap::with_capacity_and_hasher(len, S::default());
        for _ in 0..len {
            let key = KM::unmarshal(rb)?;
            let item = VM::unmarshal(rb)?;
            value.insert(key, item);
        }
        Ok(value)
    }
}

impl<K, V, KM, VM> Codec<BTreeMap<K, V>> for MapContainerMarshaler<K, V, KM, VM>
where
    K: Ord,
    KM: Codec<K>,
    VM: Codec<V>,
{
    fn marshal(value: &BTreeMap<K, V>, wb: &mut WriteBuffer) {
        Self::marshal_len(wb, value.len());
        for (key, item) in value {
            KM::marshal(key, wb);
            VM::marshal(item, wb);
        }
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<BTreeMap<K, V>, MarshalerError> {
        let len = Self::unmarshal_len(rb)?;
        let mut value = BTreeMap::new();
        for _ in 0..len {
            let key = KM::unmarshal(rb)?;
            let item = VM::unmarshal(rb)?;
            value.insert(key, item);
        }
        Ok(value)
    }
}

impl<K, V, KM, VM> Codec<IndexMap<K, V>> for MapContainerMarshaler<K, V, KM, VM>
where
    K: Eq + Hash,
    KM: Codec<K>,
    VM: Codec<V>,
{
    fn marshal(value: &IndexMap<K, V>, wb: &mut WriteBuffer) {
        Self::marshal_len(wb, value.len());
        for (key, item) in value {
            KM::marshal(key, wb);
            VM::marshal(item, wb);
        }
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<IndexMap<K, V>, MarshalerError> {
        let len = Self::unmarshal_len(rb)?;
        let mut value = IndexMap::with_capacity(len);
        for _ in 0..len {
            let key = KM::unmarshal(rb)?;
            let item = VM::unmarshal(rb)?;
            value.insert(key, item);
        }
        Ok(value)
    }
}

/// `Vec<T>` encoded as: `VLQ32` length, then `T` elements in order.
///
/// Length is bounded by [`WIRE_VEC_CAP`] on read; counts above it are
/// rejected to prevent oversized `VLQ` allocation attempts.
impl<T: Marshaler> Marshaler for Vec<T> {
    fn marshal(&self, wb: &mut WriteBuffer) {
        marshal_wire_count(wb, self.len());
        for item in self {
            item.marshal(wb);
        }
    }
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let len = VlqU32Marshaler.unmarshal(rb)? as usize;
        if len > WIRE_VEC_CAP {
            return Err(MarshalerError::ContainerOverflow {
                len,
                capacity: WIRE_VEC_CAP,
            });
        }
        let mut v = Vec::with_capacity(len);
        for _ in 0..len {
            v.push(T::unmarshal(rb)?);
        }
        Ok(v)
    }
}

/// `(A, B)` encoded as: `A` then `B`.
impl<A: Marshaler, B: Marshaler> Marshaler for (A, B) {
    fn marshal(&self, wb: &mut WriteBuffer) {
        self.0.marshal(wb);
        self.1.marshal(wb);
    }
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        Ok((A::unmarshal(rb)?, B::unmarshal(rb)?))
    }
}

/// `ArrayVec<T, N>` encoded as VLQ32 length then `T` elements.
impl<T: Marshaler, const N: usize> Marshaler for ArrayVec<T, N> {
    fn marshal(&self, wb: &mut WriteBuffer) {
        marshal_wire_count(wb, self.len());
        for item in self {
            item.marshal(wb);
        }
    }
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let len = VlqU32Marshaler.unmarshal(rb)? as usize;
        if len > N {
            return Err(MarshalerError::ContainerOverflow { len, capacity: N });
        }
        let mut v = ArrayVec::new();
        for _ in 0..len {
            v.push(T::unmarshal(rb)?);
        }
        Ok(v)
    }
}

impl<const N: usize> Marshaler for ArrayString<N> {
    fn marshal(&self, wb: &mut WriteBuffer) {
        marshal_wire_count(wb, self.len());
        wb.write_bytes(self.as_bytes());
    }
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let len = VlqU32Marshaler.unmarshal(rb)? as usize;
        if len > N {
            return Err(MarshalerError::StringOverflow { len, capacity: N });
        }
        let bytes = rb.read_bytes(len)?;
        let s = std::str::from_utf8(bytes)?;
        let mut value = ArrayString::new();
        value
            .try_push_str(s)
            .map_err(|_| MarshalerError::StringOverflow { len, capacity: N })?;
        Ok(value)
    }
}

/// `[T; N]` encoded as exactly `N` consecutive `T` elements (no length prefix).
impl<T: Marshaler, const N: usize> Marshaler for [T; N] {
    fn marshal(&self, wb: &mut WriteBuffer) {
        for item in self {
            item.marshal(wb);
        }
    }
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let mut tmp = ArrayVec::<T, N>::new();
        for _ in 0..N {
            tmp.push(T::unmarshal(rb)?);
        }
        tmp.into_inner()
            .map_err(|_| MarshalerError::ContainerOverflow {
                len: N + 1,
                capacity: N,
            })
    }
}

/// `IndexSet<T>` encoded as: VLQ32 length, then `T` elements in iteration order.
///
/// insertion/wire order after unmarshal.
impl<T> Marshaler for IndexSet<T>
where
    T: Marshaler + Eq + Hash,
{
    fn marshal(&self, wb: &mut WriteBuffer) {
        marshal_wire_count(wb, self.len());
        for item in self {
            item.marshal(wb);
        }
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let len = VlqU32Marshaler.unmarshal(rb)? as usize;
        if len > WIRE_VEC_CAP {
            return Err(MarshalerError::ContainerOverflow {
                len,
                capacity: WIRE_VEC_CAP,
            });
        }
        let mut set = IndexSet::with_capacity(len);
        for _ in 0..len {
            set.insert(T::unmarshal(rb)?);
        }
        Ok(set)
    }
}

/// `IndexMap<K, V>` encoded as: VLQ32 length, then pairs `K`, `V` in iteration order.
///
/// insertion/wire order after unmarshal.
impl<K, V> Marshaler for IndexMap<K, V>
where
    K: Marshaler + Eq + Hash,
    V: Marshaler,
{
    fn marshal(&self, wb: &mut WriteBuffer) {
        marshal_wire_count(wb, self.len());
        for (k, v) in self {
            k.marshal(wb);
            v.marshal(wb);
        }
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let len = VlqU32Marshaler.unmarshal(rb)? as usize;
        if len > WIRE_VEC_CAP {
            return Err(MarshalerError::ContainerOverflow {
                len,
                capacity: WIRE_VEC_CAP,
            });
        }
        let mut map = IndexMap::with_capacity(len);
        for _ in 0..len {
            let k = K::unmarshal(rb)?;
            let v = V::unmarshal(rb)?;
            map.insert(k, v);
        }
        Ok(map)
    }
}

/// `HashSet<T>` encoded as: VLQ32 length, then `T` elements in iteration order.
///
/// This matches the generic count-plus-entry byte shape, but it is not
/// protocol fields.
impl<T, S> Marshaler for HashSet<T, S>
where
    T: Marshaler + Eq + Hash,
    S: BuildHasher + Default,
{
    fn marshal(&self, wb: &mut WriteBuffer) {
        marshal_wire_count(wb, self.len());
        for item in self {
            item.marshal(wb);
        }
    }
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let len = VlqU32Marshaler.unmarshal(rb)? as usize;
        if len > WIRE_VEC_CAP {
            return Err(MarshalerError::ContainerOverflow {
                len,
                capacity: WIRE_VEC_CAP,
            });
        }
        let mut set = HashSet::with_capacity_and_hasher(len, S::default());
        for _ in 0..len {
            set.insert(T::unmarshal(rb)?);
        }
        Ok(set)
    }
}

/// `BTreeSet<T>` encoded as: VLQ32 length, then `T` elements in sorted order.
impl<T> Marshaler for BTreeSet<T>
where
    T: Marshaler + Ord,
{
    fn marshal(&self, wb: &mut WriteBuffer) {
        marshal_wire_count(wb, self.len());
        for item in self {
            item.marshal(wb);
        }
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let len = VlqU32Marshaler.unmarshal(rb)? as usize;
        if len > WIRE_VEC_CAP {
            return Err(MarshalerError::ContainerOverflow {
                len,
                capacity: WIRE_VEC_CAP,
            });
        }
        let mut set = BTreeSet::new();
        for _ in 0..len {
            set.insert(T::unmarshal(rb)?);
        }
        Ok(set)
    }
}

/// `HashMap<K, V>` encoded as: VLQ32 length, then pairs `K`, `V` in iteration order.
///
/// This matches the generic count-plus-entry byte shape, but it is not
/// protocol fields.
impl<K, V, S> Marshaler for HashMap<K, V, S>
where
    K: Marshaler + Eq + Hash,
    V: Marshaler,
    S: BuildHasher + Default,
{
    fn marshal(&self, wb: &mut WriteBuffer) {
        marshal_wire_count(wb, self.len());
        for (k, v) in self {
            k.marshal(wb);
            v.marshal(wb);
        }
    }
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let len = VlqU32Marshaler.unmarshal(rb)? as usize;
        if len > WIRE_VEC_CAP {
            return Err(MarshalerError::ContainerOverflow {
                len,
                capacity: WIRE_VEC_CAP,
            });
        }
        let mut map = HashMap::with_capacity_and_hasher(len, S::default());
        for _ in 0..len {
            let k = K::unmarshal(rb)?;
            let v = V::unmarshal(rb)?;
            map.insert(k, v);
        }
        Ok(map)
    }
}

/// `BTreeMap<K, V>` encoded as: VLQ32 length, then pairs `K`, `V` in key order.
/// Unlike `HashMap`, iteration order is deterministic (sorted by key).
impl<K, V> Marshaler for std::collections::BTreeMap<K, V>
where
    K: Marshaler + Ord,
    V: Marshaler,
{
    fn marshal(&self, wb: &mut WriteBuffer) {
        marshal_wire_count(wb, self.len());
        for (k, v) in self {
            k.marshal(wb);
            v.marshal(wb);
        }
    }
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let len = VlqU32Marshaler.unmarshal(rb)? as usize;
        if len > WIRE_VEC_CAP {
            return Err(MarshalerError::ContainerOverflow {
                len,
                capacity: WIRE_VEC_CAP,
            });
        }
        let mut map = std::collections::BTreeMap::new();
        for _ in 0..len {
            let k = K::unmarshal(rb)?;
            let v = V::unmarshal(rb)?;
            map.insert(k, v);
        }
        Ok(map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serialize::buffer::CARRIER_ENDIAN;

    fn read_len_only<T: Marshaler>(len: usize) -> Result<T, MarshalerError> {
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        VlqU32Marshaler.marshal(&mut wb, u32::try_from(len).unwrap());
        let bytes = wb.into_vec();
        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bytes);
        T::unmarshal(&mut rb)
    }

    #[test]
    fn index_map_preserves_stream_order() {
        let mut value = IndexMap::new();
        value.insert(2u8, 20u16);
        value.insert(1, 10);
        value.insert(3, 30);
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        value.marshal(&mut wb);
        let bytes = wb.into_vec();

        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bytes);
        let decoded = IndexMap::<u8, u16>::unmarshal(&mut rb).unwrap();

        assert_eq!(
            decoded.into_iter().collect::<Vec<_>>(),
            vec![(2, 20), (1, 10), (3, 30)]
        );
        assert_eq!(rb.left(), 0);
    }

    #[test]
    fn index_set_preserves_stream_order() {
        let mut value = IndexSet::new();
        value.insert(3u8);
        value.insert(1);
        value.insert(2);
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        value.marshal(&mut wb);
        let bytes = wb.into_vec();

        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bytes);
        let decoded = IndexSet::<u8>::unmarshal(&mut rb).unwrap();

        assert_eq!(decoded.into_iter().collect::<Vec<_>>(), vec![3, 1, 2]);
        assert_eq!(rb.left(), 0);
    }

    #[test]
    fn rust_maps_reject_oversized_wire_counts() {
        let len = WIRE_VEC_CAP + 1;
        for result in [
            read_len_only::<HashSet<u8>>(len).map(|_| ()),
            read_len_only::<HashMap<u8, u8>>(len).map(|_| ()),
            read_len_only::<IndexSet<u8>>(len).map(|_| ()),
            read_len_only::<IndexMap<u8, u8>>(len).map(|_| ()),
            read_len_only::<std::collections::BTreeMap<u8, u8>>(len).map(|_| ()),
        ] {
            assert!(matches!(
                result,
                Err(MarshalerError::ContainerOverflow { len: got, capacity })
                    if got == len && capacity == WIRE_VEC_CAP
            ));
        }
    }

    #[test]
    fn u16_counted_container_marshaler_uses_u16_count() {
        let value = vec![0x11u8, 0x22, 0x33];
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        ContainerMarshaler::<u8>::marshal(&value, &mut wb);
        let bytes = wb.into_vec();

        assert_eq!(&bytes[..2], &3u16.to_be_bytes());
        assert_eq!(&bytes[2..], &[0x11, 0x22, 0x33]);

        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bytes);
        let decoded: Vec<u8> = ContainerMarshaler::<u8>::unmarshal(&mut rb).unwrap();
        assert_eq!(decoded, value);
        assert_eq!(rb.left(), 0);
    }

    #[test]
    fn u16_counted_container_marshaler_handles_string_bytes() {
        let value = String::from("mix");
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        ContainerMarshaler::<u8>::marshal(&value, &mut wb);
        let bytes = wb.into_vec();

        assert_eq!(&bytes[..2], &3u16.to_be_bytes());
        assert_eq!(&bytes[2..], b"mix");

        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bytes);
        let decoded = <ContainerMarshaler<u8> as Codec<String>>::unmarshal(&mut rb).unwrap();
        assert_eq!(decoded, value);
        assert_eq!(rb.left(), 0);
    }

    #[test]
    fn u16_counted_map_container_marshaler_uses_u16_count() {
        let mut value = IndexMap::new();
        value.insert(7u8, 70u16);
        value.insert(8, 80);

        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        MapContainerMarshaler::<u8, u16>::marshal(&value, &mut wb);
        let bytes = wb.into_vec();

        assert_eq!(&bytes[..2], &2u16.to_be_bytes());

        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bytes);
        let decoded: IndexMap<u8, u16> =
            MapContainerMarshaler::<u8, u16>::unmarshal(&mut rb).unwrap();
        assert_eq!(
            decoded.into_iter().collect::<Vec<_>>(),
            vec![(7, 70), (8, 80)]
        );
        assert_eq!(rb.left(), 0);
    }
}
