//! Replicated containers keep the ergonomic Rust spelling
//! `ReplicatedContainer<Vec<T>>`, `ReplicatedContainer<HashMap<K, V>>`, or
//! `ReplicatedContainer<IndexMap<K, V>>`. The type owns replication metadata
//! beside the ordinary Rust container so callers mutate through domain methods
//! instead of manually coordinating flags and journals.
//!
//! A zero change count means a snapshot body follows, and a non-zero count
//! means that many delta changes follow.
//! Snapshot bodies carry `SequenceNumber + VLQ count + entries`; delta bodies
//! carry live-mask batches of changed entries. A delta
//! `SequenceNumber::ValidNonSequence` repeats the previous sequence.

use std::collections::HashMap;
use std::hash::{BuildHasher, Hash};
use std::marker::PhantomData;

use arrayvec::ArrayVec;
use indexmap::IndexMap;

use crate::hub::SequenceNumber;
use crate::serialize::replicated_field::ReplicatedFieldHandlerBase;

use super::{
    buffer::{ReadBuffer, WriteBuffer},
    container_marshal::{WIRE_VEC_CAP, marshal_wire_count},
    error::MarshalerError,
    live_mask::{read_live_mask_batches, write_live_mask_batches},
    marshaler::{Codec, DefaultMarshaler, Marshaler},
    quantize::usize_to_f32,
    vlq::{VlqU32Marshaler, VlqU64},
};

pub const REPLICATED_CONTAINER_FIXED_JOURNAL_SIZE: usize = 10;

pub type ReplicatedVec<T, const CAP: usize = WIRE_VEC_CAP> = ReplicatedContainer<Vec<T>, CAP>;

pub type ReplicatedMap<K, V, const CAP: usize = WIRE_VEC_CAP> =
    ReplicatedContainer<HashMap<K, V>, CAP>;

pub type ReplicatedIndexMap<K, V, const CAP: usize = WIRE_VEC_CAP> =
    ReplicatedContainer<IndexMap<K, V>, CAP>;

/// The wire format encodes `Add` and `Update` identically: both set the live
/// bit and may carry a value. Delta unmarshal therefore yields `Update`; vector
/// storage can still treat an update at the current length as an append.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ChangeOp {
    Add,
    Update,
    #[default]
    Remove,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change<K, V> {
    Add {
        key: K,
        value: Option<V>,
        sequence: SequenceNumber,
    },
    Update {
        key: K,
        value: Option<V>,
        sequence: SequenceNumber,
    },
    Remove {
        key: K,
        sequence: SequenceNumber,
    },
}

impl<K: Default, V> Default for Change<K, V> {
    fn default() -> Self {
        Self::Remove {
            key: K::default(),
            sequence: SequenceNumber::Invalid,
        }
    }
}

impl<K, V> Change<K, V> {
    #[must_use]
    pub fn add(key: K, value: V, sequence: impl Into<SequenceNumber>) -> Self {
        Self::Add {
            key,
            value: Some(value),
            sequence: sequence.into(),
        }
    }

    #[must_use]
    pub fn update(key: K, value: V, sequence: impl Into<SequenceNumber>) -> Self {
        Self::Update {
            key,
            value: Some(value),
            sequence: sequence.into(),
        }
    }

    #[must_use]
    pub fn remove(key: K, sequence: impl Into<SequenceNumber>) -> Self {
        Self::Remove {
            key,
            sequence: sequence.into(),
        }
    }

    #[must_use]
    pub fn delta(key: K, value: Option<V>, sequence: impl Into<SequenceNumber>) -> Self {
        let sequence = sequence.into();
        match value {
            Some(value) => Self::update(key, value, sequence),
            None => Self::remove(key, sequence),
        }
    }

    #[must_use]
    pub fn add_key(key: K, sequence: impl Into<SequenceNumber>) -> Self {
        Self::Add {
            key,
            value: None,
            sequence: sequence.into(),
        }
    }

    #[must_use]
    pub fn update_key(key: K, sequence: impl Into<SequenceNumber>) -> Self {
        Self::Update {
            key,
            value: None,
            sequence: sequence.into(),
        }
    }

    #[must_use]
    pub const fn op(&self) -> ChangeOp {
        match self {
            Self::Add { .. } => ChangeOp::Add,
            Self::Update { .. } => ChangeOp::Update,
            Self::Remove { .. } => ChangeOp::Remove,
        }
    }

    #[must_use]
    pub const fn key(&self) -> &K {
        match self {
            Self::Add { key, .. } | Self::Update { key, .. } | Self::Remove { key, .. } => key,
        }
    }

    #[must_use]
    pub const fn value(&self) -> Option<&V> {
        match self {
            Self::Add { value, .. } | Self::Update { value, .. } => value.as_ref(),
            Self::Remove { .. } => None,
        }
    }

    #[must_use]
    pub const fn sequence(&self) -> SequenceNumber {
        match self {
            Self::Add { sequence, .. }
            | Self::Update { sequence, .. }
            | Self::Remove { sequence, .. } => *sequence,
        }
    }

    #[must_use]
    pub const fn is_live(&self) -> bool {
        matches!(self, Self::Add { .. } | Self::Update { .. })
    }

    pub fn fill_value(&mut self, next_value: V) -> bool {
        match self {
            Self::Add { value, .. } | Self::Update { value, .. } if value.is_none() => {
                *value = Some(next_value);
                true
            }
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChangeSet<K, V> {
    pub sequence: SequenceNumber,
    pub changes: Vec<Change<K, V>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ContainerFlags(u8);

impl ContainerFlags {
    const INITIALIZE_CHANGE: u8 = 1 << 0;
    const PENDING_DELTA_CHANGES: u8 = 1 << 1;
    const DEFAULT_VALUE: u8 = 1 << 2;
    const NEW_NETWORK_DATA: u8 = 1 << 3;

    const fn delta() -> Self {
        Self(Self::PENDING_DELTA_CHANGES)
    }

    const fn contains(self, flag: u8) -> bool {
        (self.0 & flag) != 0
    }

    fn set(&mut self, flag: u8, value: bool) {
        if value {
            self.0 |= flag;
        } else {
            self.0 &= !flag;
        }
    }

    const fn initialize_change(self) -> bool {
        self.contains(Self::INITIALIZE_CHANGE)
    }

    fn set_initialize_change(&mut self, value: bool) {
        self.set(Self::INITIALIZE_CHANGE, value);
    }

    const fn pending_delta_changes(self) -> bool {
        self.contains(Self::PENDING_DELTA_CHANGES)
    }

    fn set_pending_delta_changes(&mut self, value: bool) {
        self.set(Self::PENDING_DELTA_CHANGES, value);
    }

    const fn default_value(self) -> bool {
        self.contains(Self::DEFAULT_VALUE)
    }

    fn set_default_value(&mut self, value: bool) {
        self.set(Self::DEFAULT_VALUE, value);
    }

    const fn new_network_data(self) -> bool {
        self.contains(Self::NEW_NETWORK_DATA)
    }

    fn set_new_network_data(&mut self, value: bool) {
        self.set(Self::NEW_NETWORK_DATA, value);
    }
}

impl Default for ContainerFlags {
    fn default() -> Self {
        Self(Self::INITIALIZE_CHANGE)
    }
}

///
/// `KMarshaller` and `VMarshaller` policy parameters; defaults use each
/// key/value type's own [`Marshaler`] impl.
#[derive(Debug)]
pub struct ReplicatedContainer<
    C,
    const CAP: usize = WIRE_VEC_CAP,
    KM = DefaultMarshaler<<C as ReplicatedContainerStorage>::Key>,
    VM = DefaultMarshaler<<C as ReplicatedContainerStorage>::Value>,
> where
    C: ReplicatedContainerStorage,
    KM: Codec<C::Key>,
    VM: Codec<C::Value>,
{
    last_modified: SequenceNumber,
    last_modified_marshal_full: SequenceNumber,
    /// Snapshot/current value store used by snapshot-mode marshaling.
    values: C,
    current_changes: Vec<Change<C::Key, C::Value>>,
    journal: ArrayVec<ChangeSet<C::Key, C::Value>, REPLICATED_CONTAINER_FIXED_JOURNAL_SIZE>,
    flags: ContainerFlags,
    seq_of_last_processed_change: SequenceNumber,
    marshaler: PhantomData<fn() -> (KM, VM)>,
}

impl<C, const CAP: usize, KM, VM> Clone for ReplicatedContainer<C, CAP, KM, VM>
where
    C: ReplicatedContainerStorage + Clone,
    C::Key: Clone,
    C::Value: Clone,
    KM: Codec<C::Key>,
    VM: Codec<C::Value>,
{
    fn clone(&self) -> Self {
        Self {
            last_modified: self.last_modified,
            last_modified_marshal_full: self.last_modified_marshal_full,
            values: self.values.clone(),
            current_changes: self.current_changes.clone(),
            journal: self.journal.clone(),
            flags: self.flags,
            seq_of_last_processed_change: self.seq_of_last_processed_change,
            marshaler: PhantomData,
        }
    }
}

impl<C, const CAP: usize, KM, VM> PartialEq for ReplicatedContainer<C, CAP, KM, VM>
where
    C: ReplicatedContainerStorage + PartialEq,
    C::Key: PartialEq,
    C::Value: PartialEq,
    KM: Codec<C::Key>,
    VM: Codec<C::Value>,
{
    fn eq(&self, rhs: &Self) -> bool {
        self.last_modified == rhs.last_modified
            && self.last_modified_marshal_full == rhs.last_modified_marshal_full
            && self.values == rhs.values
            && self.current_changes == rhs.current_changes
            && self.journal == rhs.journal
            && self.flags == rhs.flags
            && self.seq_of_last_processed_change == rhs.seq_of_last_processed_change
    }
}

impl<C, const CAP: usize, KM, VM> Eq for ReplicatedContainer<C, CAP, KM, VM>
where
    C: ReplicatedContainerStorage + Eq,
    C::Key: Eq,
    C::Value: Eq,
    KM: Codec<C::Key>,
    VM: Codec<C::Value>,
{
}

impl<C, const CAP: usize, KM, VM> Default for ReplicatedContainer<C, CAP, KM, VM>
where
    C: ReplicatedContainerStorage,
    KM: Codec<C::Key>,
    VM: Codec<C::Value>,
{
    fn default() -> Self {
        Self {
            last_modified: SequenceNumber::Invalid,
            last_modified_marshal_full: SequenceNumber::Invalid,
            values: C::empty(),
            current_changes: Vec::new(),
            journal: ArrayVec::new(),
            flags: ContainerFlags::default(),
            seq_of_last_processed_change: SequenceNumber::Invalid,
            marshaler: PhantomData,
        }
    }
}

impl<C, const CAP: usize, KM, VM> ReplicatedContainer<C, CAP, KM, VM>
where
    C: ReplicatedContainerStorage,
    KM: Codec<C::Key>,
    VM: Codec<C::Value>,
{
    /// Build a snapshot-mode replicated container.
    pub fn new(last_modified: impl Into<SequenceNumber>, values: C) -> Self {
        Self {
            last_modified: last_modified.into(),
            last_modified_marshal_full: SequenceNumber::Invalid,
            values,
            current_changes: Vec::new(),
            journal: ArrayVec::new(),
            flags: ContainerFlags::default(),
            seq_of_last_processed_change: SequenceNumber::Invalid,
            marshaler: PhantomData,
        }
    }

    #[must_use]
    pub fn delta(current_changes: Vec<Change<C::Key, C::Value>>) -> Self {
        Self {
            current_changes,
            flags: ContainerFlags::delta(),
            last_modified: SequenceNumber::ValidNonSequence,
            ..Self::default()
        }
    }

    #[inline]
    #[must_use]
    pub const fn values(&self) -> &C {
        &self.values
    }

    #[must_use]
    pub fn into_values(self) -> C {
        self.values
    }

    #[inline]
    #[must_use]
    pub fn is_delta(&self) -> bool {
        !self.current_changes.is_empty()
    }

    #[inline]
    #[must_use]
    pub fn has_value(&self) -> bool {
        self.last_modified.is_valid()
            || self.flags.default_value()
            || self.values.len() > 0
            || !self.current_changes.is_empty()
    }

    #[inline]
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.has_value()
    }

    #[inline]
    #[must_use]
    pub const fn last_modified(&self) -> SequenceNumber {
        self.last_modified
    }

    #[inline]
    #[must_use]
    pub const fn last_full_marshal_sequence(&self) -> SequenceNumber {
        self.last_modified_marshal_full
    }

    #[inline]
    #[must_use]
    pub const fn last_processed_change_sequence(&self) -> SequenceNumber {
        self.seq_of_last_processed_change
    }

    #[inline]
    pub fn set_last_modified(&mut self, sequence: impl Into<SequenceNumber>) {
        self.last_modified = sequence.into();
    }

    #[inline]
    #[must_use]
    pub fn journal(&self) -> &[ChangeSet<C::Key, C::Value>] {
        &self.journal
    }

    pub fn push_journal(&mut self, change_set: ChangeSet<C::Key, C::Value>) {
        if self.journal.len() == REPLICATED_CONTAINER_FIXED_JOURNAL_SIZE {
            self.journal.remove(0);
        }
        self.journal.push(change_set);
    }

    #[must_use]
    pub fn has_change_set(&self, sequence: impl Into<SequenceNumber>) -> bool {
        let sequence = sequence.into();
        self.journal
            .iter()
            .any(|change_set| change_set.sequence == sequence)
    }

    #[must_use]
    pub fn begin_change_set_sequence_number(&self) -> SequenceNumber {
        self.journal
            .first()
            .map_or(SequenceNumber::Invalid, |change_set| change_set.sequence)
    }

    #[must_use]
    pub fn end_change_set_sequence_number(&self) -> SequenceNumber {
        self.journal
            .last()
            .map_or(SequenceNumber::Invalid, |change_set| change_set.sequence)
    }

    #[must_use]
    pub fn journal_size(&self) -> usize {
        self.journal.len()
    }

    #[must_use]
    pub fn is_initialize_change(&self) -> bool {
        self.flags.initialize_change()
    }

    #[must_use]
    pub const fn has_pending_delta_changes(&self) -> bool {
        self.flags.pending_delta_changes()
    }

    #[must_use]
    pub const fn is_default_value(&self) -> bool {
        self.flags.default_value()
    }

    #[must_use]
    pub const fn has_new_network_data(&self) -> bool {
        self.flags.new_network_data()
    }

    pub fn clear_current_changes(&mut self) {
        self.current_changes.clear();
        self.flags.set_initialize_change(false);
    }

    pub fn clear_journal(&mut self) {
        self.journal.clear();
    }

    pub fn clear_all_change_sets(&mut self) {
        self.journal.clear();
        self.clear_current_changes();
    }

    #[inline]
    #[must_use]
    pub fn current_changes(&self) -> &[Change<C::Key, C::Value>] {
        &self.current_changes
    }

    pub fn current_value_changes(
        &self,
    ) -> impl Iterator<Item = (&C::Key, &C::Value, SequenceNumber)> + '_ {
        self.current_changes.iter().filter_map(|change| {
            change
                .value()
                .map(|value| (change.key(), value, change.sequence()))
        })
    }

    #[must_use]
    pub fn values_with_current_changes(&self) -> C
    where
        C: Clone,
        C::Key: Clone,
        C::Value: Clone,
    {
        let mut values = self.values.clone();
        values.apply_changes(&self.current_changes);
        values
    }

    fn push_initialize_change(&mut self) {
        self.last_modified = SequenceNumber::ValidNonSequence;
        self.flags.set_default_value(false);
        if !self.flags.initialize_change() {
            self.flags.set_initialize_change(true);
            self.current_changes.clear();
        }
        self.flags.set_pending_delta_changes(false);
    }

    pub fn push_add(&mut self, key: C::Key, value: C::Value)
    where
        C::Key: Clone,
        C::Value: Clone,
    {
        self.values.apply_change(&Change::add(
            key.clone(),
            value.clone(),
            SequenceNumber::ValidNonSequence,
        ));
        self.push_change(Change::add_key(key, SequenceNumber::Invalid));
    }

    pub fn push_update(&mut self, key: C::Key, value: C::Value)
    where
        C::Key: Clone,
        C::Value: Clone,
    {
        self.values.apply_change(&Change::update(
            key.clone(),
            value.clone(),
            SequenceNumber::ValidNonSequence,
        ));
        self.push_change(Change::update_key(key, SequenceNumber::Invalid));
    }

    pub fn push_remove(&mut self, key: C::Key)
    where
        C::Key: Clone,
        C::Value: Clone,
    {
        self.values.apply_change(&Change::remove(
            key.clone(),
            SequenceNumber::ValidNonSequence,
        ));
        self.push_change(Change::remove(key, SequenceNumber::Invalid));
    }

    fn push_change(&mut self, change: Change<C::Key, C::Value>) {
        debug_assert!(
            !self.flags.pending_delta_changes() || self.current_changes.is_empty(),
            "delta-encoded changes must be merged before local edits"
        );

        self.last_modified = SequenceNumber::ValidNonSequence;
        self.flags.set_default_value(false);
        if !self.flags.initialize_change() {
            if self.current_changes.len() > self.values.len() {
                self.push_initialize_change();
            } else {
                self.current_changes.push(change);
            }
        }
        self.flags.set_pending_delta_changes(false);
    }

    pub fn client_update_sequence_of_last_processed_change(
        &mut self,
        sequence: impl Into<SequenceNumber>,
    ) {
        let sequence = sequence.into();
        if self.seq_of_last_processed_change < sequence {
            self.seq_of_last_processed_change = sequence;
        }
    }

    fn copy_container_from(&mut self, incoming: &Self, seq_of_last_processed_change: SequenceNumber)
    where
        C: Clone,
    {
        self.values = incoming.values.clone();
        self.seq_of_last_processed_change = seq_of_last_processed_change;
    }

    fn apply_change_set(&mut self, sequence: SequenceNumber, changes: &[Change<C::Key, C::Value>])
    where
        C::Key: Clone,
        C::Value: Clone,
    {
        self.values.apply_changes(changes);
        self.push_journal(ChangeSet {
            sequence,
            changes: changes.to_vec(),
        });
    }

    pub fn merge_change_sets(&mut self, incoming: &Self, sequence: impl Into<SequenceNumber>)
    where
        C::Key: Clone,
        C::Value: Clone,
    {
        let sequence = sequence.into();
        let end_sequence = self.end_change_set_sequence_number();
        for change_set in &incoming.journal {
            debug_assert!(
                end_sequence < change_set.sequence || self.has_change_set(change_set.sequence),
                "replicated container merge saw an unaccounted older change set"
            );
            if end_sequence < change_set.sequence {
                self.apply_change_set(change_set.sequence, &change_set.changes);
            }
        }

        if !incoming.current_changes.is_empty() {
            self.apply_change_set(sequence, &incoming.current_changes);
        }
    }

    pub fn summarize_changes(
        &self,
        baseline: impl Into<SequenceNumber>,
        efficiency: f32,
    ) -> Vec<Change<C::Key, C::Value>>
    where
        C::Key: Clone,
        C::Value: Clone,
    {
        let baseline = baseline.into();
        if self.flags.initialize_change() {
            return Vec::new();
        }

        let mut changes = Vec::new();
        for change_set in &self.journal {
            if baseline < change_set.sequence {
                changes.extend(change_set.changes.iter().cloned());
            }
        }
        changes.extend(self.current_changes.iter().cloned());
        for change in &mut changes {
            self.values.populate_change_value(change);
        }

        if !changes.is_empty()
            && usize_to_f32(changes.len()) > (usize_to_f32(self.values.len()) * efficiency)
        {
            Vec::new()
        } else {
            changes
        }
    }

    fn marshal_full(&self, wb: &mut WriteBuffer) {
        VlqU32Marshaler.marshal(wb, 0);
        self.last_modified.marshal(wb);
        marshal_wire_count(wb, self.values.len());
        self.values.marshal_entries::<KM, VM>(wb);
    }

    fn marshal_changes(&self, wb: &mut WriteBuffer, changes: &[Change<C::Key, C::Value>])
    where
        C::Value: Default + Clone,
    {
        marshal_wire_count(wb, changes.len());
        let mut previous_sequence = SequenceNumber::Invalid;
        write_live_mask_batches(wb, changes, Change::is_live, |wb, change, live| {
            let sequence = change.sequence();
            C::marshal_key::<KM>(change.key(), wb);
            if sequence == previous_sequence {
                SequenceNumber::ValidNonSequence.marshal(wb);
            } else {
                sequence.marshal(wb);
                previous_sequence = sequence;
            }
            if live {
                if let Some(value) = change.value() {
                    VM::marshal(value, wb);
                } else if let Some(value) = self.values.change_value(change.key()) {
                    VM::marshal(&value, wb);
                } else {
                    VM::marshal(&C::Value::default(), wb);
                }
            }
        });
    }

    pub fn marshal_since(&self, wb: &mut WriteBuffer, baseline: impl Into<SequenceNumber>)
    where
        C::Key: Clone,
        C::Value: Clone + Default,
    {
        const VLQ_TWO_BYTE_CHANGE_LIMIT: usize = 1 << 14;

        let baseline = baseline.into();
        if !baseline.is_valid() {
            self.marshal_full(wb);
            return;
        }

        let changes = self.summarize_changes(baseline, 0.8);
        debug_assert!(
            changes.len() < VLQ_TWO_BYTE_CHANGE_LIMIT,
            "too many replicated-container changes for one sync"
        );

        if changes.is_empty() || changes.len() >= VLQ_TWO_BYTE_CHANGE_LIMIT {
            self.marshal_full(wb);
        } else {
            self.marshal_changes(wb, &changes);
        }
    }
}

/// Storage behavior supplied by ordinary Rust containers.
pub trait ReplicatedContainerStorage {
    type Key;
    type Value;

    fn empty() -> Self;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
    fn marshal_entries<KM, VM>(&self, wb: &mut WriteBuffer)
    where
        KM: Codec<Self::Key>,
        VM: Codec<Self::Value>;
    /// Decode a snapshot body containing `count` entries.
    ///
    /// # Errors
    ///
    /// Returns the first error reported by a key or value codec.
    fn unmarshal_entries<KM, VM>(rb: &mut ReadBuffer, count: usize) -> Result<Self, MarshalerError>
    where
        Self: Sized,
        KM: Codec<Self::Key>,
        VM: Codec<Self::Value>;
    /// Decode one delta key.
    ///
    /// # Errors
    ///
    /// Returns the first error reported by the key codec.
    fn unmarshal_key<KM>(rb: &mut ReadBuffer) -> Result<Self::Key, MarshalerError>
    where
        KM: Codec<Self::Key>;
    fn marshal_key<KM>(key: &Self::Key, wb: &mut WriteBuffer)
    where
        KM: Codec<Self::Key>;
    fn change_value(&self, key: &Self::Key) -> Option<Self::Value>
    where
        Self::Value: Clone;
    fn populate_change_value(&self, change: &mut Change<Self::Key, Self::Value>)
    where
        Self::Value: Clone,
    {
        if !change.is_live() || change.value().is_some() {
            return;
        }
        if let Some(value) = self.change_value(change.key()) {
            change.fill_value(value);
        }
    }
    fn apply_change(&mut self, change: &Change<Self::Key, Self::Value>)
    where
        Self::Key: Clone,
        Self::Value: Clone;

    fn apply_changes(&mut self, changes: &[Change<Self::Key, Self::Value>])
    where
        Self::Key: Clone,
        Self::Value: Clone,
    {
        for change in changes {
            self.apply_change(change);
        }
    }
}

impl<T> ReplicatedContainerStorage for Vec<T> {
    type Key = VlqU64;
    type Value = T;

    fn empty() -> Self {
        Vec::new()
    }

    fn len(&self) -> usize {
        Vec::len(self)
    }

    fn marshal_entries<KM, VM>(&self, wb: &mut WriteBuffer)
    where
        KM: Codec<Self::Key>,
        VM: Codec<Self::Value>,
    {
        for value in self {
            VM::marshal(value, wb);
        }
    }

    fn unmarshal_entries<KM, VM>(rb: &mut ReadBuffer, count: usize) -> Result<Self, MarshalerError>
    where
        KM: Codec<Self::Key>,
        VM: Codec<Self::Value>,
    {
        let mut values = Vec::with_capacity(count);
        for _ in 0..count {
            values.push(VM::unmarshal(rb)?);
        }
        Ok(values)
    }

    fn unmarshal_key<KM>(rb: &mut ReadBuffer) -> Result<Self::Key, MarshalerError>
    where
        KM: Codec<Self::Key>,
    {
        KM::unmarshal(rb)
    }

    fn marshal_key<KM>(key: &Self::Key, wb: &mut WriteBuffer)
    where
        KM: Codec<Self::Key>,
    {
        KM::marshal(key, wb);
    }

    fn change_value(&self, key: &Self::Key) -> Option<Self::Value>
    where
        Self::Value: Clone,
    {
        usize::try_from(key.get())
            .ok()
            .and_then(|index| self.get(index))
            .cloned()
    }

    fn apply_change(&mut self, change: &Change<Self::Key, Self::Value>)
    where
        Self::Key: Clone,
        Self::Value: Clone,
    {
        let Ok(index) = usize::try_from(change.key().get()) else {
            debug_assert!(false, "replicated vector index does not fit in usize");
            return;
        };
        match (change.op(), change.value()) {
            (ChangeOp::Add | ChangeOp::Update, Some(value)) => match index.cmp(&self.len()) {
                std::cmp::Ordering::Less => {
                    self[index] = value.clone();
                }
                std::cmp::Ordering::Equal => {
                    self.push(value.clone());
                }
                std::cmp::Ordering::Greater => {
                    debug_assert!(false, "replicated vector update index is out of bounds");
                }
            },
            (ChangeOp::Add | ChangeOp::Update, None) if index < self.len() => {}
            (ChangeOp::Add | ChangeOp::Update, None) => {
                debug_assert!(false, "replicated vector live change is missing a value");
            }
            (ChangeOp::Remove, _) if index < self.len() => {
                self.remove(index);
            }
            (ChangeOp::Remove, _) => {
                debug_assert!(false, "replicated vector remove index is out of bounds");
            }
        }
    }
}

impl<K, V, S> ReplicatedContainerStorage for HashMap<K, V, S>
where
    K: Eq + Hash,
    S: BuildHasher + Default,
{
    type Key = K;
    type Value = V;

    fn empty() -> Self {
        HashMap::with_hasher(S::default())
    }

    fn len(&self) -> usize {
        HashMap::len(self)
    }

    fn marshal_entries<KM, VM>(&self, wb: &mut WriteBuffer)
    where
        KM: Codec<Self::Key>,
        VM: Codec<Self::Value>,
    {
        for (key, value) in self {
            KM::marshal(key, wb);
            VM::marshal(value, wb);
        }
    }

    fn unmarshal_entries<KM, VM>(rb: &mut ReadBuffer, count: usize) -> Result<Self, MarshalerError>
    where
        KM: Codec<Self::Key>,
        VM: Codec<Self::Value>,
    {
        let mut values = HashMap::with_capacity_and_hasher(count, S::default());
        for _ in 0..count {
            values.insert(KM::unmarshal(rb)?, VM::unmarshal(rb)?);
        }
        Ok(values)
    }

    fn unmarshal_key<KM>(rb: &mut ReadBuffer) -> Result<Self::Key, MarshalerError>
    where
        KM: Codec<Self::Key>,
    {
        KM::unmarshal(rb)
    }

    fn marshal_key<KM>(key: &Self::Key, wb: &mut WriteBuffer)
    where
        KM: Codec<Self::Key>,
    {
        KM::marshal(key, wb);
    }

    fn change_value(&self, key: &Self::Key) -> Option<Self::Value>
    where
        Self::Value: Clone,
    {
        self.get(key).cloned()
    }

    fn apply_change(&mut self, change: &Change<Self::Key, Self::Value>)
    where
        Self::Key: Clone,
        Self::Value: Clone,
    {
        match (change.op(), change.value()) {
            (ChangeOp::Add | ChangeOp::Update, Some(value)) => {
                self.insert(change.key().clone(), value.clone());
            }
            (ChangeOp::Add | ChangeOp::Update, None) => {
                debug_assert!(false, "replicated map live change is missing a value");
            }
            (ChangeOp::Remove, _) => {
                self.remove(change.key());
            }
        }
    }
}

impl<K, V> ReplicatedContainerStorage for IndexMap<K, V>
where
    K: Eq + Hash,
{
    type Key = K;
    type Value = V;

    fn empty() -> Self {
        IndexMap::new()
    }

    fn len(&self) -> usize {
        IndexMap::len(self)
    }

    fn marshal_entries<KM, VM>(&self, wb: &mut WriteBuffer)
    where
        KM: Codec<Self::Key>,
        VM: Codec<Self::Value>,
    {
        for (key, value) in self {
            KM::marshal(key, wb);
            VM::marshal(value, wb);
        }
    }

    fn unmarshal_entries<KM, VM>(rb: &mut ReadBuffer, count: usize) -> Result<Self, MarshalerError>
    where
        KM: Codec<Self::Key>,
        VM: Codec<Self::Value>,
    {
        let mut values = IndexMap::with_capacity(count);
        for _ in 0..count {
            values.insert(KM::unmarshal(rb)?, VM::unmarshal(rb)?);
        }
        Ok(values)
    }

    fn unmarshal_key<KM>(rb: &mut ReadBuffer) -> Result<Self::Key, MarshalerError>
    where
        KM: Codec<Self::Key>,
    {
        KM::unmarshal(rb)
    }

    fn marshal_key<KM>(key: &Self::Key, wb: &mut WriteBuffer)
    where
        KM: Codec<Self::Key>,
    {
        KM::marshal(key, wb);
    }

    fn change_value(&self, key: &Self::Key) -> Option<Self::Value>
    where
        Self::Value: Clone,
    {
        self.get(key).cloned()
    }

    fn apply_change(&mut self, change: &Change<Self::Key, Self::Value>)
    where
        Self::Key: Clone,
        Self::Value: Clone,
    {
        match (change.op(), change.value()) {
            (ChangeOp::Add | ChangeOp::Update, Some(value)) => {
                self.insert(change.key().clone(), value.clone());
            }
            (ChangeOp::Add | ChangeOp::Update, None) => {
                debug_assert!(false, "replicated index map live change is missing a value");
            }
            (ChangeOp::Remove, _) => {
                self.shift_remove(change.key());
            }
        }
    }
}

impl<C, const CAP: usize, KM, VM> Marshaler for ReplicatedContainer<C, CAP, KM, VM>
where
    C: ReplicatedContainerStorage,
    C::Value: Default + Clone,
    KM: Codec<C::Key>,
    VM: Codec<C::Value>,
{
    fn marshal(&self, wb: &mut WriteBuffer) {
        if self.current_changes.is_empty() {
            self.marshal_full(wb);
            return;
        }

        self.marshal_changes(wb, &self.current_changes);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let mode = usize::try_from(VlqU32Marshaler.unmarshal(rb)?).map_err(|_| {
            MarshalerError::ContainerOverflow {
                len: usize::MAX,
                capacity: CAP,
            }
        })?;
        if mode == 0 {
            let last_modified_marshal_full = SequenceNumber::unmarshal(rb)?;
            let count = usize::try_from(VlqU32Marshaler.unmarshal(rb)?).map_err(|_| {
                MarshalerError::ContainerOverflow {
                    len: usize::MAX,
                    capacity: CAP,
                }
            })?;
            check_count(count, CAP)?;
            let mut flags = ContainerFlags::default();
            flags.set_new_network_data(true);
            let mut value = Self {
                last_modified_marshal_full,
                values: C::unmarshal_entries::<KM, VM>(rb, count)?,
                seq_of_last_processed_change: last_modified_marshal_full,
                flags,
                ..Self::default()
            };
            value.push_initialize_change();
            return Ok(value);
        }

        check_count(mode, CAP)?;
        let mut previous_sequence = SequenceNumber::Invalid;
        let current_changes = read_live_mask_batches(rb, mode, |rb, live| {
            let key = C::unmarshal_key::<KM>(rb)?;
            let raw_sequence = SequenceNumber::unmarshal(rb)?;
            let sequence = if raw_sequence == SequenceNumber::ValidNonSequence {
                previous_sequence
            } else {
                previous_sequence = raw_sequence;
                raw_sequence
            };
            let value = if live { Some(VM::unmarshal(rb)?) } else { None };
            Ok(Change::delta(key, value, sequence))
        })?;

        let mut flags = ContainerFlags::delta();
        flags.set_new_network_data(true);
        Ok(Self {
            last_modified: SequenceNumber::ValidNonSequence,
            values: C::empty(),
            current_changes,
            flags,
            ..Self::default()
        })
    }
}

impl<C, const CAP: usize, KM, VM> ReplicatedFieldHandlerBase for ReplicatedContainer<C, CAP, KM, VM>
where
    C: ReplicatedContainerStorage + Clone + PartialEq + 'static,
    C::Key: Clone + PartialEq + 'static,
    C::Value: Clone + Default + PartialEq + 'static,
    KM: Codec<C::Key> + 'static,
    VM: Codec<C::Value> + 'static,
{
    fn is_default_value(&self) -> bool {
        self.flags.default_value()
    }

    fn set_current_value_as_default(&mut self) {
        self.flags.set_default_value(true);
        self.last_modified = SequenceNumber::Invalid;
        self.current_changes.clear();
    }

    fn is_dirty(&self, baseline: SequenceNumber) -> bool {
        baseline < self.last_modified
    }

    fn has_value(&self) -> bool {
        ReplicatedContainer::has_value(self)
    }

    fn marshal_field(&self, wb: &mut WriteBuffer) {
        self.marshal(wb);
    }

    fn marshal_field_since(&self, wb: &mut WriteBuffer, baseline: SequenceNumber) {
        self.marshal_since(wb, baseline);
    }

    fn unmarshal_field(&mut self, rb: &mut ReadBuffer) -> Result<(), MarshalerError> {
        *self = Self::unmarshal(rb)?;
        Ok(())
    }

    fn merge_and_update_sequence(
        &mut self,
        old_value: &dyn ReplicatedFieldHandlerBase,
        new_value: &mut dyn ReplicatedFieldHandlerBase,
        seq: SequenceNumber,
        inherit_previous_network_data_status: bool,
    ) -> bool {
        let Some(old_value) = old_value.downcast_ref::<Self>() else {
            debug_assert!(false, "old replicated container type mismatch");
            return false;
        };
        let Some(new_value) = new_value.downcast_mut::<Self>() else {
            debug_assert!(false, "new replicated container type mismatch");
            return false;
        };

        self.clear_all_change_sets();
        self.flags.set_new_network_data(false);
        let mut detected_new_data = true;
        self.seq_of_last_processed_change = old_value.seq_of_last_processed_change;

        if std::ptr::eq(old_value, new_value) {
            if new_value.last_modified.is_valid() {
                self.last_modified = seq;
                self.copy_container_from(new_value, new_value.seq_of_last_processed_change);
                if !new_value.flags.initialize_change() {
                    self.journal.clone_from(&new_value.journal);
                }
                self.push_journal(ChangeSet {
                    sequence: seq,
                    changes: new_value.current_changes.clone(),
                });
                self.flags.set_new_network_data(true);
            }
            new_value.clear_current_changes();
            return detected_new_data;
        }

        let last_modified = old_value.last_modified();
        let old_seq = seq.min(last_modified);
        let mut new_current_changes = !new_value.current_changes.is_empty();
        let mut new_initialization = new_value.flags.initialize_change();

        if old_value.seq_of_last_processed_change.is_valid() {
            if let Some(last_change) = new_value.current_changes.last() {
                new_current_changes = new_current_changes
                    && old_value.seq_of_last_processed_change < last_change.sequence();
            }
            new_initialization = new_initialization
                && old_value.seq_of_last_processed_change < new_value.last_modified_marshal_full;
        }

        let new_change_set =
            old_value.end_change_set_sequence_number() < new_value.end_change_set_sequence_number();
        let has_new_value = new_current_changes || new_initialization || new_change_set;

        if new_value.last_modified.is_valid() && has_new_value {
            self.last_modified = seq;
            self.flags.set_new_network_data(true);

            if new_value.flags.initialize_change() {
                self.copy_container_from(new_value, new_value.seq_of_last_processed_change);
                self.push_journal(ChangeSet {
                    sequence: seq,
                    changes: new_value.current_changes.clone(),
                });
            } else if !new_value.flags.pending_delta_changes() {
                self.copy_container_from(new_value, new_value.seq_of_last_processed_change);
                if new_value.current_changes.is_empty() {
                    self.journal.clone_from(&new_value.journal);
                } else {
                    self.journal.clone_from(&old_value.journal);
                    self.push_journal(ChangeSet {
                        sequence: seq,
                        changes: new_value.current_changes.clone(),
                    });
                }
            } else {
                self.copy_container_from(old_value, old_value.seq_of_last_processed_change);
                self.journal.clone_from(&old_value.journal);
                self.merge_change_sets(new_value, seq);
            }
        } else {
            self.last_modified = old_seq;
            self.copy_container_from(old_value, old_value.seq_of_last_processed_change);
            self.journal.clone_from(&old_value.journal);
            if inherit_previous_network_data_status {
                self.flags
                    .set_new_network_data(old_value.has_new_network_data());
            }
            detected_new_data = false;
        }

        new_value.clear_current_changes();
        detected_new_data
    }

    fn last_modified(&self) -> SequenceNumber {
        self.last_modified
    }

    fn set_last_modified(&mut self, seq: SequenceNumber) {
        self.last_modified = seq;
    }

    fn reset_has_new_network_data(&mut self) {
        self.flags.set_new_network_data(false);
    }

    fn has_new_network_data(&self) -> bool {
        self.flags.new_network_data()
    }
}

fn check_count(count: usize, capacity: usize) -> Result<(), MarshalerError> {
    if count > capacity {
        return Err(MarshalerError::ContainerOverflow {
            len: count,
            capacity,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serialize::buffer::CARRIER_ENDIAN;

    #[derive(Debug, Clone, Copy, Default)]
    struct PlusOneU16;

    impl Codec<u16> for PlusOneU16 {
        fn marshal(value: &u16, wb: &mut WriteBuffer) {
            (*value + 1).marshal(wb);
        }

        fn unmarshal(rb: &mut ReadBuffer) -> Result<u16, MarshalerError> {
            Ok(u16::unmarshal(rb)? - 1)
        }
    }

    #[test]
    fn members_default_to_encoded_values() {
        let value = ReplicatedContainer::<Vec<u32>>::default();

        assert_eq!(value.last_modified, SequenceNumber::Invalid);
        assert!(value.values.is_empty());
        assert!(value.current_changes.is_empty());
        assert!(value.journal.is_empty());
        assert_eq!(value.last_modified_marshal_full, SequenceNumber::Invalid);
        assert!(value.is_initialize_change());
        assert!(!value.has_pending_delta_changes());
        assert!(!value.is_default_value());
        assert!(!value.has_new_network_data());
        assert_eq!(value.seq_of_last_processed_change, SequenceNumber::Invalid);
    }

    #[test]
    fn vec_snapshot_round_trips() {
        let value = ReplicatedContainer::<Vec<u32>>::new(SequenceNumber::Seq(7), vec![10, 20, 30]);

        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        value.marshal(&mut wb);
        let bytes = wb.into_vec();

        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bytes);
        let decoded = ReplicatedContainer::<Vec<u32>>::unmarshal(&mut rb).unwrap();
        assert_eq!(rb.left(), 0);
        assert_eq!(decoded.values, value.values);
        assert_eq!(decoded.last_modified, SequenceNumber::ValidNonSequence);
        assert_eq!(decoded.last_modified_marshal_full, SequenceNumber::Seq(7));
        assert_eq!(decoded.seq_of_last_processed_change, SequenceNumber::Seq(7));
        assert!(decoded.is_initialize_change());
        assert!(!decoded.has_pending_delta_changes());
        assert!(decoded.has_new_network_data());
    }

    #[test]
    fn snapshot_uses_zero_mode_then_sequence_count_and_entries() {
        let value = ReplicatedContainer::<Vec<u8>>::new(SequenceNumber::Seq(7), vec![10, 20]);

        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        value.marshal(&mut wb);

        assert_eq!(wb.as_slice(), &[0, 1, 7, 2, 10, 20]);
    }

    #[test]
    fn index_map_snapshot_round_trips() {
        let mut values = IndexMap::new();
        values.insert(42u32, 0x1122_3344_5566_7788u64);
        let value: ReplicatedContainer<IndexMap<u32, u64>> =
            ReplicatedContainer::new(SequenceNumber::Seq(7), values);

        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        value.marshal(&mut wb);
        let bytes = wb.into_vec();

        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bytes);
        let decoded = ReplicatedContainer::<IndexMap<u32, u64>>::unmarshal(&mut rb).unwrap();
        assert_eq!(rb.left(), 0);
        assert_eq!(decoded.values, value.values);
        assert_eq!(decoded.last_modified, SequenceNumber::ValidNonSequence);
        assert_eq!(decoded.last_modified_marshal_full, SequenceNumber::Seq(7));
        assert_eq!(decoded.seq_of_last_processed_change, SequenceNumber::Seq(7));
        assert!(decoded.is_initialize_change());
        assert!(decoded.has_new_network_data());
    }

    #[test]
    fn hash_map_snapshot_round_trips() {
        let mut values = HashMap::new();
        values.insert(42u32, 9u8);
        let value: ReplicatedContainer<HashMap<u32, u8>> =
            ReplicatedContainer::new(SequenceNumber::Seq(7), values);

        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        value.marshal(&mut wb);
        let bytes = wb.into_vec();

        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bytes);
        let decoded = ReplicatedContainer::<HashMap<u32, u8>>::unmarshal(&mut rb).unwrap();
        assert_eq!(rb.left(), 0);
        assert_eq!(decoded.values, value.values);
        assert_eq!(decoded.last_modified, SequenceNumber::ValidNonSequence);
        assert_eq!(decoded.last_modified_marshal_full, SequenceNumber::Seq(7));
        assert_eq!(decoded.seq_of_last_processed_change, SequenceNumber::Seq(7));
        assert!(decoded.is_initialize_change());
        assert!(decoded.has_new_network_data());
    }

    #[test]
    fn map_delta_round_trips_and_reuses_repeated_sequences() {
        let mut value = ReplicatedContainer::<IndexMap<u32, u8>>::default();
        value
            .current_changes
            .push(Change::update(10, 2, SequenceNumber::Seq(5)));
        value
            .current_changes
            .push(Change::remove(11, SequenceNumber::Seq(5)));

        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        value.marshal(&mut wb);
        let bytes = wb.into_vec();

        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bytes);
        let decoded = ReplicatedContainer::<IndexMap<u32, u8>>::unmarshal(&mut rb).unwrap();
        assert_eq!(rb.left(), 0);
        assert_eq!(decoded.current_changes, value.current_changes);
        assert_eq!(decoded.last_modified, SequenceNumber::ValidNonSequence);
        assert!(!decoded.is_initialize_change());
        assert!(decoded.has_pending_delta_changes());
        assert!(decoded.has_new_network_data());
    }

    #[test]
    fn delta_uses_mode_count_live_masks_keys_and_wire_sequences() {
        let mut value = ReplicatedContainer::<IndexMap<u32, u8>>::default();
        value
            .current_changes
            .push(Change::update(10, 2, SequenceNumber::Seq(5)));
        value
            .current_changes
            .push(Change::remove(11, SequenceNumber::Seq(5)));

        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        value.marshal(&mut wb);

        assert_eq!(
            wb.as_slice(),
            &[
                2, // mode/change count
                1, // live mask: first change has a value, second is remove
                0, 0, 0, 10, // key
                1, 5, // sequence
                2, // value
                0, 0, 0, 11, // key
                1, 0, // ValidNonSequence repeats previous sequence
            ]
        );
    }

    #[test]
    fn vec_delta_round_trips_and_reuses_repeated_sequences() {
        let mut value = ReplicatedContainer::<Vec<u32>>::default();
        value
            .current_changes
            .push(Change::update(VlqU64::new(0), 42, SequenceNumber::Seq(5)));
        value
            .current_changes
            .push(Change::remove(VlqU64::new(3), SequenceNumber::Seq(5)));

        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        value.marshal(&mut wb);
        let bytes = wb.into_vec();

        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bytes);
        let decoded = ReplicatedContainer::<Vec<u32>>::unmarshal(&mut rb).unwrap();
        assert_eq!(rb.left(), 0);
        assert_eq!(decoded.current_changes, value.current_changes);
    }

    #[test]
    fn explicit_key_and_value_codecs_are_used() {
        type CustomContainer =
            ReplicatedContainer<IndexMap<u16, u16>, WIRE_VEC_CAP, PlusOneU16, PlusOneU16>;

        let mut values = IndexMap::new();
        values.insert(4, 9);
        let value = CustomContainer::new(SequenceNumber::Seq(7), values);

        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        value.marshal(&mut wb);
        let bytes = wb.into_vec();
        assert!(bytes.ends_with(&[0, 5, 0, 10]));

        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bytes);
        let decoded = CustomContainer::unmarshal(&mut rb).unwrap();
        assert_eq!(rb.left(), 0);
        assert_eq!(decoded.values, value.values);
        assert_eq!(decoded.last_modified, SequenceNumber::ValidNonSequence);
        assert_eq!(decoded.last_modified_marshal_full, SequenceNumber::Seq(7));
    }

    #[test]
    fn implements_replicated_field_handler_base() {
        let mut value = ReplicatedContainer::<Vec<u32>>::new(SequenceNumber::Seq(7), vec![1]);
        let field: &mut dyn ReplicatedFieldHandlerBase = &mut value;

        assert!(field.has_value());
        assert!(field.is_dirty(SequenceNumber::Invalid));
        assert!(!field.has_new_network_data());
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        field.marshal_field(&mut wb);
        field
            .unmarshal_field(&mut ReadBuffer::new(CARRIER_ENDIAN, wb.as_slice()))
            .unwrap();
        assert!(field.has_new_network_data());
        field.reset_has_new_network_data();
        assert!(!field.has_new_network_data());
        field.set_current_value_as_default();
        assert!(field.is_default_value());
        assert_eq!(field.last_modified(), SequenceNumber::Invalid);
    }

    #[test]
    fn merge_clears_incoming_current_changes() {
        let mut old_values = IndexMap::new();
        old_values.insert(1u32, 5u8);
        let old = ReplicatedContainer::<IndexMap<u32, u8>>::new(SequenceNumber::Seq(1), old_values);
        let mut new = ReplicatedContainer::<IndexMap<u32, u8>>::delta(vec![Change::update(
            2,
            7,
            SequenceNumber::Seq(2),
        )]);
        let mut merged = ReplicatedContainer::<IndexMap<u32, u8>>::default();

        let detected = ReplicatedFieldHandlerBase::merge_and_update_sequence(
            &mut merged,
            &old,
            &mut new,
            SequenceNumber::Seq(3),
            false,
        );

        assert!(detected);
        assert!(new.current_changes.is_empty());
        assert!(!new.is_initialize_change());
        assert_eq!(merged.values.get(&1), Some(&5));
        assert_eq!(merged.values.get(&2), Some(&7));
    }
}
