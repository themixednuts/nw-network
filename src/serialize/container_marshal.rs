//! Container codecs and count policies for protocol collections.
//!
//! Blanket [`Marshaler`] impls for Rust
//! containers (`Vec`, `String`, maps, sets, `ArrayVec`, and `ArrayString`)
//! write a VLQ32 element or byte count before the entries. The explicit
//! [`ContainerMarshaler`] and [`MapContainerMarshaler`] policy codecs instead
//! write a raw carrier-endian `u16` count.
//!
//! Fields choose the policy required by their wire format: use the blanket
//! impls for VLQ-counted collections and policy codecs for fixed `u16`
//! counted slots. Entries are serialized in iteration order; use [`IndexMap`]
//! or [`IndexSet`] when byte order needs to be deterministic.

use crate::serialize::marshaler::{Marshal, Unmarshal};

use super::{
    buffer::{ReadBuffer, WriteBuffer},
    error::MarshalerError,
    marshaler::{Codec, DefaultMarshaler},
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

fn unmarshal_wire_count(rb: &mut ReadBuffer, capacity: usize) -> Result<usize, MarshalerError> {
    let len = VlqU32Marshaler.unmarshal(rb)? as usize;
    if len > capacity {
        return Err(MarshalerError::ContainerOverflow { len, capacity });
    }
    Ok(len)
}

/// VLQ-counted sequence encoded through a field-local element codec.
#[derive(Debug, Clone, Copy, Default)]
pub struct SequenceCodec<M>(PhantomData<fn() -> M>);

impl<T, M: Codec<T>> Codec<Vec<T>> for SequenceCodec<M> {
    fn marshal(value: &Vec<T>, wb: &mut WriteBuffer) {
        marshal_wire_count(wb, value.len());
        for item in value {
            M::marshal(item, wb);
        }
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Vec<T>, MarshalerError> {
        let len = unmarshal_wire_count(rb, WIRE_VEC_CAP)?;
        let mut value = Vec::with_capacity(len);
        for _ in 0..len {
            value.push(M::unmarshal(rb)?);
        }
        Ok(value)
    }
}

impl<T, M: Codec<T>, const N: usize> Codec<ArrayVec<T, N>> for SequenceCodec<M> {
    fn marshal(value: &ArrayVec<T, N>, wb: &mut WriteBuffer) {
        marshal_wire_count(wb, value.len());
        for item in value {
            M::marshal(item, wb);
        }
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<ArrayVec<T, N>, MarshalerError> {
        let len = unmarshal_wire_count(rb, N)?;
        let mut value = ArrayVec::new();
        for _ in 0..len {
            value.push(M::unmarshal(rb)?);
        }
        Ok(value)
    }
}

/// VLQ-counted ordered map encoded through field-local key and value codecs.
#[derive(Debug, Clone, Copy, Default)]
pub struct MapSequenceCodec<KM, VM>(PhantomData<fn() -> (KM, VM)>);

impl<K, V, KM, VM> Codec<IndexMap<K, V>> for MapSequenceCodec<KM, VM>
where
    K: Eq + Hash,
    KM: Codec<K>,
    VM: Codec<V>,
{
    fn marshal(value: &IndexMap<K, V>, wb: &mut WriteBuffer) {
        marshal_wire_count(wb, value.len());
        for (key, value) in value {
            KM::marshal(key, wb);
            VM::marshal(value, wb);
        }
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<IndexMap<K, V>, MarshalerError> {
        let len = unmarshal_wire_count(rb, WIRE_VEC_CAP)?;
        let mut value = IndexMap::with_capacity(len);
        for _ in 0..len {
            value.insert(KM::unmarshal(rb)?, VM::unmarshal(rb)?);
        }
        Ok(value)
    }
}

/// Fixed-length array encoded through a field-local element codec.
#[derive(Debug, Clone, Copy, Default)]
pub struct ArrayCodec<M>(PhantomData<fn() -> M>);

impl<T, M: Codec<T>, const N: usize> Codec<[T; N]> for ArrayCodec<M> {
    const MARSHAL_SIZE: usize = if M::MARSHAL_SIZE == 0 {
        0
    } else {
        N * M::MARSHAL_SIZE
    };

    fn marshal(value: &[T; N], wb: &mut WriteBuffer) {
        for item in value {
            M::marshal(item, wb);
        }
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<[T; N], MarshalerError> {
        let mut value = ArrayVec::<T, N>::new();
        for _ in 0..N {
            value.push(M::unmarshal(rb)?);
        }
        value
            .into_inner()
            .map_err(|_| MarshalerError::ContainerOverflow {
                len: N + 1,
                capacity: N,
            })
    }
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
impl<T: Marshal> Marshal for Vec<T> {
    fn marshal(&self, wb: &mut WriteBuffer) {
        marshal_wire_count(wb, self.len());
        for item in self {
            item.marshal(wb);
        }
    }
}

impl<T: Unmarshal> Unmarshal for Vec<T> {
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let len = unmarshal_wire_count(rb, WIRE_VEC_CAP)?;
        let mut v = Vec::with_capacity(len);
        for _ in 0..len {
            v.push(T::unmarshal(rb)?);
        }
        Ok(v)
    }
}

macro_rules! impl_tuple_marshaler {
    ($(($($type:ident : $index:tt),+)),+ $(,)?) => {
        $(
            impl<$($type: Marshal),+> Marshal for ($($type,)+) {
                fn marshal(&self, wb: &mut WriteBuffer) {
                    $(self.$index.marshal(wb);)+
                }
            }

            impl<$($type: Unmarshal),+> Unmarshal for ($($type,)+) {

                fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
                    Ok(($($type::unmarshal(rb)?,)+))
                }
            }
        )+
    };
}

impl_tuple_marshaler!(
    (A: 0, B: 1),
    (A: 0, B: 1, C: 2),
    (A: 0, B: 1, C: 2, D: 3),
    (A: 0, B: 1, C: 2, D: 3, E: 4),
    (A: 0, B: 1, C: 2, D: 3, E: 4, F: 5),
    (A: 0, B: 1, C: 2, D: 3, E: 4, F: 5, G: 6),
    (A: 0, B: 1, C: 2, D: 3, E: 4, F: 5, G: 6, H: 7),
    (A: 0, B: 1, C: 2, D: 3, E: 4, F: 5, G: 6, H: 7, I: 8),
    (A: 0, B: 1, C: 2, D: 3, E: 4, F: 5, G: 6, H: 7, I: 8, J: 9),
    (A: 0, B: 1, C: 2, D: 3, E: 4, F: 5, G: 6, H: 7, I: 8, J: 9, K: 10),
    (A: 0, B: 1, C: 2, D: 3, E: 4, F: 5, G: 6, H: 7, I: 8, J: 9, K: 10, L: 11),
);

/// `ArrayVec<T, N>` encoded as VLQ32 length then `T` elements.
impl<T: Marshal, const N: usize> Marshal for ArrayVec<T, N> {
    fn marshal(&self, wb: &mut WriteBuffer) {
        marshal_wire_count(wb, self.len());
        for item in self {
            item.marshal(wb);
        }
    }
}

impl<T: Unmarshal, const N: usize> Unmarshal for ArrayVec<T, N> {
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let len = unmarshal_wire_count(rb, N)?;
        let mut v = ArrayVec::new();
        for _ in 0..len {
            v.push(T::unmarshal(rb)?);
        }
        Ok(v)
    }
}

impl<const N: usize> Marshal for ArrayString<N> {
    fn marshal(&self, wb: &mut WriteBuffer) {
        marshal_wire_count(wb, self.len());
        wb.write_bytes(self.as_bytes());
    }
}

impl<const N: usize> Unmarshal for ArrayString<N> {
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
impl<T: Marshal, const N: usize> Marshal for [T; N] {
    fn marshal(&self, wb: &mut WriteBuffer) {
        for item in self {
            item.marshal(wb);
        }
    }
}

impl<T: Unmarshal, const N: usize> Unmarshal for [T; N] {
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
/// `IndexSet` preserves insertion/wire order after unmarshal.
impl<T> Marshal for IndexSet<T>
where
    T: Marshal + Eq + Hash,
{
    fn marshal(&self, wb: &mut WriteBuffer) {
        marshal_wire_count(wb, self.len());
        for item in self {
            item.marshal(wb);
        }
    }
}

impl<T> Unmarshal for IndexSet<T>
where
    T: Unmarshal + Eq + Hash,
{
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
/// `IndexMap` preserves insertion/wire order after unmarshal.
impl<K, V> Marshal for IndexMap<K, V>
where
    K: Marshal + Eq + Hash,
    V: Marshal,
{
    fn marshal(&self, wb: &mut WriteBuffer) {
        marshal_wire_count(wb, self.len());
        for (k, v) in self {
            k.marshal(wb);
            v.marshal(wb);
        }
    }
}

impl<K, V> Unmarshal for IndexMap<K, V>
where
    K: Unmarshal + Eq + Hash,
    V: Unmarshal,
{
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
/// deterministic enough for byte-locked protocol fields. Use `IndexSet` or
/// `BTreeSet` when the wire order must be stable.
impl<T, S> Marshal for HashSet<T, S>
where
    T: Marshal + Eq + Hash,
    S: BuildHasher + Default,
{
    fn marshal(&self, wb: &mut WriteBuffer) {
        marshal_wire_count(wb, self.len());
        for item in self {
            item.marshal(wb);
        }
    }
}

impl<T, S> Unmarshal for HashSet<T, S>
where
    T: Unmarshal + Eq + Hash,
    S: BuildHasher + Default,
{
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
impl<T> Marshal for BTreeSet<T>
where
    T: Marshal + Ord,
{
    fn marshal(&self, wb: &mut WriteBuffer) {
        marshal_wire_count(wb, self.len());
        for item in self {
            item.marshal(wb);
        }
    }
}

impl<T> Unmarshal for BTreeSet<T>
where
    T: Unmarshal + Ord,
{
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
/// deterministic enough for byte-locked protocol fields. Use `IndexMap` or
/// `BTreeMap` when the wire order must be stable.
impl<K, V, S> Marshal for HashMap<K, V, S>
where
    K: Marshal + Eq + Hash,
    V: Marshal,
    S: BuildHasher + Default,
{
    fn marshal(&self, wb: &mut WriteBuffer) {
        marshal_wire_count(wb, self.len());
        for (k, v) in self {
            k.marshal(wb);
            v.marshal(wb);
        }
    }
}

impl<K, V, S> Unmarshal for HashMap<K, V, S>
where
    K: Unmarshal + Eq + Hash,
    V: Unmarshal,
    S: BuildHasher + Default,
{
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
impl<K, V> Marshal for std::collections::BTreeMap<K, V>
where
    K: Marshal + Ord,
    V: Marshal,
{
    fn marshal(&self, wb: &mut WriteBuffer) {
        marshal_wire_count(wb, self.len());
        for (k, v) in self {
            k.marshal(wb);
            v.marshal(wb);
        }
    }
}

impl<K, V> Unmarshal for std::collections::BTreeMap<K, V>
where
    K: Unmarshal + Ord,
    V: Unmarshal,
{
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

    fn read_len_only<T: Unmarshal>(len: usize) -> Result<T, MarshalerError> {
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
