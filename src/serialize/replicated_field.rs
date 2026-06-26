//! Replicated field handlers and field-local compression policies.
//!
//! A field handler stores the optional current value, optional default value,
//! last-modified sequence, and network-data flag used by replicated-state
//! merge. The owning state decides field presence; the handler writes only
//! its value bytes through a field-local codec.

use super::{
    buffer::{ReadBuffer, WriteBuffer},
    compression_marshal::Float16Marshaler,
    error::MarshalerError,
    marshaler::{Codec, DefaultMarshaler, Marshaler},
    quantize::{f32_to_u8, f32_to_u32, u32_to_f32},
};
use crate::hub::SequenceNumber;
use glam::Vec3;
use std::any::Any;
use std::marker::PhantomData;

pub type ReplicatedFieldEquals<T> = fn(&T, &T) -> bool;

pub fn default_replicated_field_equals<T: PartialEq>(left: &T, right: &T) -> bool {
    left == right
}

#[macro_export]
macro_rules! unmarshal_replicated_fields {
    ($rb:expr, $($field:expr),+ $(,)?) => {{
        let mut bitmask = 0u8;
        let mut field_idx = 0usize;
        $(
            if field_idx % 8 == 0 {
                bitmask = $rb.read_u8()?;
            }
            if (bitmask & (1 << (field_idx % 8))) != 0 {
                $field = <_ as $crate::serialize::Marshaler>::unmarshal($rb)?;
            }
            field_idx += 1;
        )+
        let _ = field_idx;

        Ok::<(), $crate::serialize::MarshalerError>(())
    }};
}

#[macro_export]
macro_rules! marshal_replicated_fields {
    ($wb:expr, $($field:expr),+ $(,)?) => {{
        use $crate::serialize::Marshaler as _;

        let dirty = [$($field.has_field_payload()),+];
        let mut group_start = 0usize;
        while group_start < dirty.len() {
            let batch = (dirty.len() - group_start).min(8);
            let mut bitmask = 0u8;
            for bit in 0..batch {
                if dirty[group_start + bit] {
                    bitmask |= 1 << bit;
                }
            }
            $wb.write_u8(bitmask);

            let mut field_idx = 0usize;
            $(
                if field_idx >= group_start && field_idx < group_start + batch && dirty[field_idx] {
                    $field.marshal($wb);
                }
                field_idx += 1;
            )+
            let _ = field_idx;

            group_start += batch;
        }
    }};
}

pub trait ReplicatedFieldHandlerBase: Any {
    fn is_default_value(&self) -> bool;
    fn set_current_value_as_default(&mut self) {}
    fn is_dirty(&self, baseline: SequenceNumber) -> bool;
    fn has_value(&self) -> bool;
    fn marshal_field(&self, wb: &mut WriteBuffer);
    fn marshal_field_since(&self, wb: &mut WriteBuffer, _baseline: SequenceNumber) {
        self.marshal_field(wb);
    }
    /// Decode this field's value through its field-local codec.
    ///
    /// # Errors
    ///
    /// Returns the first error reported by the field codec.
    fn unmarshal_field(&mut self, rb: &mut ReadBuffer) -> Result<(), MarshalerError>;
    fn merge_and_update_sequence(
        &mut self,
        old_value: &dyn ReplicatedFieldHandlerBase,
        new_value: &mut dyn ReplicatedFieldHandlerBase,
        seq: SequenceNumber,
        inherit_previous_network_data_status: bool,
    ) -> bool;
    fn last_modified(&self) -> SequenceNumber;
    fn set_last_modified(&mut self, seq: SequenceNumber);
    fn is_field_valid(&self) -> bool {
        self.last_modified().is_valid()
    }
    fn reset_has_new_network_data(&mut self);
    fn has_new_network_data(&self) -> bool;
}

impl dyn ReplicatedFieldHandlerBase + '_ {
    #[must_use]
    pub fn as_any(&self) -> &dyn Any {
        self
    }

    pub fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }

    #[must_use]
    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        self.as_any().downcast_ref::<T>()
    }

    #[must_use]
    pub fn downcast_mut<T: Any>(&mut self) -> Option<&mut T> {
        self.as_any_mut().downcast_mut::<T>()
    }
}

/// Optional replicated field with default suppression, sequence bookkeeping,
/// and a field-local wire codec.
///
/// `M` is a [`Codec<T>`] policy. The default uses `T`'s own
/// [`Marshaler`] impl through [`DefaultMarshaler`].
/// Custom wire shapes such as half-floats, VLQs, and quantized positions supply a
/// zero-sized policy type implementing `Codec<T>`.
#[derive(Debug, Clone)]
pub struct ReplicatedFieldHandler<T: Default, M: Codec<T> = DefaultMarshaler<T>> {
    value: Option<T>,
    /// Invalid initially; transitions to `ValidNonSequence` after direct
    /// value changes or inbound unmarshalling.
    last_modified: SequenceNumber,
    /// A value equal to this default is treated as suppressible.
    default_value: Option<T>,
    /// Set when inbound merge/unmarshal produced network-visible data.
    /// Local value changes deliberately leave this false.
    has_new_network_data: bool,
    equals: Option<ReplicatedFieldEquals<T>>,
    marshaler: PhantomData<fn() -> M>,
}

impl<T: Default, M: Codec<T>> Default for ReplicatedFieldHandler<T, M> {
    fn default() -> Self {
        Self {
            value: None,
            last_modified: SequenceNumber::Invalid,
            default_value: None,
            has_new_network_data: false,
            equals: None,
            marshaler: PhantomData,
        }
    }
}

impl<T: Default, M: Codec<T>> ReplicatedFieldHandler<T, M> {
    /// Construct from an explicit optional value.
    /// `last_modified` follows the value's optionality:
    /// `Some(_)` ⇒ [`SequenceNumber::ValidNonSequence`],
    /// `None` ⇒ [`SequenceNumber::Invalid`].
    #[must_use]
    pub const fn new(value: Option<T>) -> Self {
        let last_modified = if value.is_some() {
            SequenceNumber::ValidNonSequence
        } else {
            SequenceNumber::Invalid
        };
        Self {
            value,
            last_modified,
            default_value: None,
            has_new_network_data: false,
            equals: None,
            marshaler: PhantomData,
        }
    }

    #[must_use]
    pub const fn with_equals(equals: ReplicatedFieldEquals<T>) -> Self {
        Self {
            value: None,
            last_modified: SequenceNumber::Invalid,
            default_value: None,
            has_new_network_data: false,
            equals: Some(equals),
            marshaler: PhantomData,
        }
    }

    #[must_use]
    pub const fn new_with_equals(value: Option<T>, equals: ReplicatedFieldEquals<T>) -> Self {
        let last_modified = if value.is_some() {
            SequenceNumber::ValidNonSequence
        } else {
            SequenceNumber::Invalid
        };
        Self {
            value,
            last_modified,
            default_value: None,
            has_new_network_data: false,
            equals: Some(equals),
            marshaler: PhantomData,
        }
    }

    /// Construct a present-valued handler. Shorthand for
    /// fixtures; production code generally goes through
    /// [`set_value`](Self::set_value).
    #[must_use]
    pub const fn some(value: T) -> Self {
        Self::new(Some(value))
    }

    /// returns `Option<&T>` instead of asserting `HasValue()` —
    /// callers express intent at the unwrap site.
    #[must_use]
    pub fn value(&self) -> Option<&T> {
        self.value.as_ref()
    }

    #[must_use]
    pub fn is_field_valid(&self) -> bool {
        self.last_modified.is_valid()
    }

    #[must_use]
    pub fn last_modified(&self) -> SequenceNumber {
        self.last_modified
    }

    /// machinery (e.g. inside `merge_and_update_sequence`) to stamp the
    /// merged value with the merge sequence; production `set_value`
    /// already does this for the local case.
    pub fn set_last_modified(&mut self, seq: impl Into<SequenceNumber>) {
        self.last_modified = seq.into();
    }

    #[must_use]
    pub fn has_new_network_data(&self) -> bool {
        self.has_new_network_data
    }

    /// after a successful marshal pass so subsequent passes don't
    /// re-emit the same value.
    pub fn reset_has_new_network_data(&mut self) {
        self.has_new_network_data = false;
    }

    pub fn clear_value(&mut self) {
        self.value = None;
        self.last_modified = SequenceNumber::Invalid;
        self.has_new_network_data = false;
    }

    /// Returns whether this field was modified after the supplied baseline.
    #[must_use]
    pub fn is_dirty_since(&self, baseline: impl Into<SequenceNumber>) -> bool {
        let baseline = baseline.into();
        self.last_modified.is_valid() && baseline < self.last_modified
    }

    /// Replicated-state bitmask signal: `true` when this field has bytes to
    /// write in the current field group. Distinct from
    /// [`is_dirty_since`](Self::is_dirty_since), which gates on a baseline
    /// sequence.
    #[must_use]
    pub fn has_field_payload(&self) -> bool {
        self.value.is_some()
    }

    /// Borrow the default value, if any.
    #[must_use]
    pub fn default_value(&self) -> Option<&T> {
        self.default_value.as_ref()
    }

    #[must_use]
    pub fn equals_policy(&self) -> Option<ReplicatedFieldEquals<T>> {
        self.equals
    }

    pub fn set_equals_policy(&mut self, equals: ReplicatedFieldEquals<T>) {
        self.equals = Some(equals);
    }

    /// Sets the default value while the field is still invalid.
    ///
    /// This is only meaningful before any `set_value` call; subsequent
    /// invocations are silently ignored after the field becomes valid.
    pub fn set_default_value(&mut self, default: T)
    where
        T: Clone,
    {
        if !self.last_modified.is_valid() {
            self.value = Some(default.clone());
            self.default_value = Some(default);
        }
    }
}

impl<T: Default + PartialEq + Clone, M: Codec<T>> ReplicatedFieldHandler<T, M> {
    fn values_equal(&self, left: &T, right: &T) -> bool {
        self.equals.unwrap_or(default_replicated_field_equals::<T>)(left, right)
    }

    pub fn has_value(&self) -> bool {
        self.is_field_valid() || self.is_default_value()
    }

    /// Sets the current value and updates replication bookkeeping.
    ///
    /// If `new_value` equals the default value, the value is stored but the
    /// field is marked invalid so replication can suppress it. Otherwise the
    /// field is marked `ValidNonSequence`.
    pub fn set_value(&mut self, new_value: T) {
        if matches!(self.default_value.as_ref(), Some(d) if self.values_equal(d, &new_value)) {
            // Equals default — keep the value but suppress replication.
            self.value = Some(new_value);
            self.last_modified = SequenceNumber::Invalid;
        } else {
            self.value = Some(new_value);
            self.last_modified = SequenceNumber::ValidNonSequence;
        }
    }

    pub fn set_optional_value(&mut self, new_value: Option<T>) {
        match new_value {
            Some(value) => self.set_value(value),
            None => self.clear_value(),
        }
    }

    /// Mutates the stored value in place and marks it locally modified when
    /// the callback returns `true`.
    pub fn access<F>(&mut self, cb: F)
    where
        F: FnOnce(&mut T) -> bool,
    {
        if let Some(value) = self.value.as_mut() {
            if cb(value) {
                self.last_modified = SequenceNumber::ValidNonSequence;
            }
        } else {
            let mut value = T::default();
            if cb(&mut value) {
                self.value = Some(value);
                self.last_modified = SequenceNumber::ValidNonSequence;
            }
        }
    }

    /// Returns true when the current value is present and equals the stored
    /// default value.
    #[must_use]
    pub fn is_default_value(&self) -> bool {
        match (self.default_value.as_ref(), self.value.as_ref()) {
            (Some(d), Some(v)) => self.values_equal(d, v),
            _ => false,
        }
    }

    /// Returns true when the current value is present and equals `rhs`.
    #[must_use]
    pub fn is_field_equal(&self, rhs: &T) -> bool {
        self.has_value() && matches!(self.value.as_ref(), Some(v) if self.values_equal(v, rhs))
    }

    /// Keeps the current value as the default and marks the field invalid so
    /// ordinary replication suppresses it until it changes away from that
    /// default.
    pub fn set_current_value_as_default(&mut self) {
        let value = self.value.clone().unwrap_or_default();
        self.value = Some(value.clone());
        self.default_value = Some(value);
        self.last_modified = SequenceNumber::Invalid;
    }

    /// The caller supplies the previous merged value (`old_value`), the new
    /// incoming value, the sequence stamp to apply when a real change is
    /// detected, and whether previous network-data status should be inherited.
    ///
    /// Returns true when this merge observes new data relative to `old_value`.
    #[must_use]
    pub fn merge_and_update_sequence(
        &mut self,
        old_value: &Self,
        new_value: &Self,
        seq: impl Into<SequenceNumber>,
        inherit_previous_network_data_status: bool,
    ) -> bool {
        let seq = seq.into();
        let old_seq = old_value.last_modified();
        self.has_new_network_data = false;
        let mut detected_new_data = true;

        if std::ptr::eq(old_value, new_value) {
            if new_value.last_modified.is_valid() {
                self.last_modified = seq;
                self.value.clone_from(&new_value.value);
                self.has_new_network_data = true;
            } else if new_value.default_value.is_some() {
                self.value.clone_from(&new_value.value);
            }
            return detected_new_data;
        }

        self.default_value = old_value
            .default_value
            .clone()
            .or_else(|| new_value.default_value.clone());

        if new_value.is_default_value() {
            let old_effective = old_value.effective_value();
            let new_effective = new_value.effective_value();
            let old_equals_new = self.values_equal(&new_effective, &old_effective);

            if !old_value.last_modified.is_valid() && old_equals_new {
                self.last_modified = SequenceNumber::Invalid;
                if inherit_previous_network_data_status {
                    self.has_new_network_data = old_value.has_new_network_data();
                }
                detected_new_data = false;
            } else if old_equals_new {
                self.last_modified = old_seq;
                self.has_new_network_data = old_value.has_new_network_data();
                detected_new_data = false;
            } else {
                self.last_modified = seq;
                self.has_new_network_data = true;
            }

            self.value = Some(new_effective);
            return detected_new_data;
        }

        if new_value.last_modified.is_valid() {
            let old_effective = old_value.effective_value();
            let new_effective = new_value.effective_value();
            if old_seq.is_valid() && self.values_equal(&new_effective, &old_effective) {
                self.last_modified = old_seq;
                self.value.clone_from(&old_value.value);
                if inherit_previous_network_data_status {
                    self.has_new_network_data = old_value.has_new_network_data();
                }
                detected_new_data = false;
            } else {
                self.last_modified = seq;
                self.value.clone_from(&new_value.value);
                self.has_new_network_data = true;
            }
        } else {
            self.last_modified = old_seq;
            self.value.clone_from(&old_value.value);
            if inherit_previous_network_data_status {
                self.has_new_network_data = old_value.has_new_network_data();
            }
            detected_new_data = false;
        }

        detected_new_data
    }

    fn effective_value(&self) -> T {
        self.value.clone().unwrap_or_default()
    }
}

impl<T: Default + PartialEq + Clone, M: Codec<T>> PartialEq for ReplicatedFieldHandler<T, M> {
    fn eq(&self, rhs: &Self) -> bool {
        self.has_value() == rhs.has_value()
            && (!self.has_value() || self.value.as_ref() == rhs.value.as_ref())
    }
}

impl<T: Default + Eq + Clone, M: Codec<T>> Eq for ReplicatedFieldHandler<T, M> {}

impl<T: Default + PartialEq + Clone + 'static, M: Codec<T> + 'static> ReplicatedFieldHandlerBase
    for ReplicatedFieldHandler<T, M>
{
    fn is_default_value(&self) -> bool {
        ReplicatedFieldHandler::is_default_value(self)
    }

    fn set_current_value_as_default(&mut self) {
        ReplicatedFieldHandler::set_current_value_as_default(self);
    }

    fn is_dirty(&self, baseline: SequenceNumber) -> bool {
        self.is_dirty_since(baseline)
    }

    fn has_value(&self) -> bool {
        ReplicatedFieldHandler::has_value(self)
    }

    fn marshal_field(&self, wb: &mut WriteBuffer) {
        self.marshal(wb);
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
            debug_assert!(false, "old replicated field handler type mismatch");
            return false;
        };
        let Some(new_value) = new_value.downcast_mut::<Self>() else {
            debug_assert!(false, "new replicated field handler type mismatch");
            return false;
        };
        ReplicatedFieldHandler::merge_and_update_sequence(
            self,
            old_value,
            &*new_value,
            seq,
            inherit_previous_network_data_status,
        )
    }

    fn last_modified(&self) -> SequenceNumber {
        self.last_modified()
    }

    fn set_last_modified(&mut self, seq: SequenceNumber) {
        ReplicatedFieldHandler::set_last_modified(self, seq);
    }

    fn reset_has_new_network_data(&mut self) {
        ReplicatedFieldHandler::reset_has_new_network_data(self);
    }

    fn has_new_network_data(&self) -> bool {
        self.has_new_network_data()
    }
}

impl<T: Default, M: Codec<T>> Marshaler for ReplicatedFieldHandler<T, M> {
    fn marshal(&self, wb: &mut WriteBuffer) {
        if let Some(ref v) = self.value {
            M::marshal(v, wb);
        }
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        Ok(Self {
            value: Some(M::unmarshal(rb)?),
            last_modified: SequenceNumber::ValidNonSequence,
            default_value: None,
            has_new_network_data: true,
            equals: None,
            marshaler: PhantomData,
        })
    }
}

// ---------------------------------------------------------------------------
// Per-field marshaler policies
// ---------------------------------------------------------------------------
//
// Specialized wire formats live as zero-sized codec policy types instead of
// duplicating `ReplicatedFieldHandler`.

/// `f32` as f16 BE — half-precision scalar wire form.
///
/// Use as `ReplicatedFieldHandler<f32, HalfF32Marshaler>`.
/// The wrapper struct [`HalfF32`](super::utility_marshal::HalfF32) covers the
/// `#[marshal(as = "HalfF32")]` field-attribute case; this policy covers the
/// `ReplicatedFieldHandler` slot.
#[derive(Debug, Clone, Copy, Default)]
pub struct HalfF32Marshaler;

impl Codec<f32> for HalfF32Marshaler {
    fn marshal(value: &f32, wb: &mut WriteBuffer) {
        wb.write_u16(half::f16::from_f32(*value).to_bits());
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<f32, MarshalerError> {
        let bits = rb.read_u16()?;
        Ok(half::f16::from_bits(bits).to_f32())
    }
}

/// `(f32, f32, f32)` as 3 × f16 BE — used for velocity, offset vectors,
/// magnet root offsets, and similar half-precision vector quantities.
///
/// Use as `ReplicatedFieldHandler<(f32, f32, f32), HalfVec3Marshaler>`.
#[derive(Debug, Clone, Copy, Default)]
pub struct HalfVec3Marshaler;

impl Codec<(f32, f32, f32)> for HalfVec3Marshaler {
    fn marshal(value: &(f32, f32, f32), wb: &mut WriteBuffer) {
        wb.write_u16(half::f16::from_f32(value.0).to_bits());
        wb.write_u16(half::f16::from_f32(value.1).to_bits());
        wb.write_u16(half::f16::from_f32(value.2).to_bits());
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<(f32, f32, f32), MarshalerError> {
        let x = half::f16::from_bits(rb.read_u16()?).to_f32();
        let y = half::f16::from_bits(rb.read_u16()?).to_f32();
        let z = half::f16::from_bits(rb.read_u16()?).to_f32();
        Ok((x, y, z))
    }
}

/// Custom world-position layout. Full precision in the horizontal plane,
/// bounded 16-bit quantization in height.
///
/// Use as `ReplicatedFieldHandler<(f32, f32, f32), PositionAnchorMarshaler>`.
#[derive(Debug, Clone, Copy, Default)]
pub struct PositionAnchorMarshaler;

impl PositionAnchorMarshaler {
    const HEIGHT_MIN: f32 = -100.0;
    const HEIGHT_MAX: f32 = 2000.0;

    #[inline]
    fn marshal_height(wb: &mut WriteBuffer, value: f32) {
        Float16Marshaler::new(Self::HEIGHT_MIN, Self::HEIGHT_MAX).marshal(wb, value);
    }

    #[inline]
    fn unmarshal_height(rb: &mut ReadBuffer) -> Result<f32, MarshalerError> {
        Float16Marshaler::new(Self::HEIGHT_MIN, Self::HEIGHT_MAX).unmarshal(rb)
    }
}

impl Codec<(f32, f32, f32)> for PositionAnchorMarshaler {
    const MARSHAL_SIZE: usize = 10;

    fn marshal(value: &(f32, f32, f32), wb: &mut WriteBuffer) {
        let (x, y, h) = *value;
        x.marshal(wb);
        y.marshal(wb);
        Self::marshal_height(wb, h);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<(f32, f32, f32), MarshalerError> {
        let x = f32::unmarshal(rb)?;
        let y = f32::unmarshal(rb)?;
        let h = Self::unmarshal_height(rb)?;
        Ok((x, y, h))
    }
}

// ---------------------------------------------------------------------------
// Per-field marshaler policies (continued)
// ---------------------------------------------------------------------------

/// **emits the high byte only** and reads it back
/// shifted left into the high half (low byte stays zero on
/// unmarshal).
///
/// Used as the absolute-portion codec for
/// [`DeltaCompressedCounterHandler<u16>`], where the lower byte is
/// reconstructed entirely from the delta-portion's signed-modulo-256
/// counter rather than carried separately.
///
/// Specialized for `u16` counters.
#[derive(Debug, Clone, Copy, Default)]
pub struct IntegerOmitLowerByteMarshaler;

pub const BITS_PER_BYTE: u32 = 8;

impl Codec<u16> for IntegerOmitLowerByteMarshaler {
    fn marshal(value: &u16, wb: &mut WriteBuffer) {
        let byte = u8::try_from(*value >> BITS_PER_BYTE).expect("u16 high byte fits in u8");
        wb.write_u8(byte);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<u16, MarshalerError> {
        let byte = rb.read_u8()?;
        Ok(u16::from(byte) << BITS_PER_BYTE)
    }
}

/// Stores a delta-counter relative portion as one raw byte: truncating cast on
/// send, zero-extending on receive.
///
/// Functionally equivalent to writing a `u8` directly, but named so callers
/// can choose the right policy at a glance.
#[derive(Debug, Clone, Copy, Default)]
pub struct DeltaIntegerMarshaler;

impl Codec<u16> for DeltaIntegerMarshaler {
    fn marshal(value: &u16, wb: &mut WriteBuffer) {
        wb.write_u8(byte_from_u16(*value));
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<u16, MarshalerError> {
        Ok(u16::from(rb.read_u8()?))
    }
}

impl Codec<u32> for DeltaIntegerMarshaler {
    fn marshal(value: &u32, wb: &mut WriteBuffer) {
        wb.write_u8(byte_from_u32(*value));
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<u32, MarshalerError> {
        Ok(u32::from(rb.read_u8()?))
    }
}

pub const VALUES_IN_BYTE: u16 = 255;

fn quantize_delta<const DELTA_RANGE: u32>(value: f32) -> u8 {
    let delta_range = u32_to_f32(DELTA_RANGE);
    let quantized = (value + delta_range) * 255.0 / (2.0 * delta_range);
    f32_to_u8(quantized.clamp(0.0, 255.0))
}

fn unquantize_delta<const DELTA_RANGE: u32>(quantized: u8) -> f32 {
    let delta_range = u32_to_f32(DELTA_RANGE);
    2.0 * delta_range * f32::from(quantized) / 255.0 - delta_range
}

fn byte_from_u16(value: u16) -> u8 {
    u8::try_from(value & 0x00ff).expect("masked u16 byte fits in u8")
}

fn byte_from_u32(value: u32) -> u8 {
    u8::try_from(value & 0x0000_00ff).expect("masked u32 byte fits in u8")
}

#[derive(Debug, Clone, Copy, Default)]
pub struct DeltaMarshaler<const DELTA_RANGE: u32, T>(PhantomData<fn() -> T>);

impl<const DELTA_RANGE: u32> DeltaMarshaler<DELTA_RANGE, f32> {
    #[must_use]
    pub fn quantized(value: f32) -> u8 {
        quantize_delta::<DELTA_RANGE>(value)
    }

    #[must_use]
    pub fn unquantized(quantized: u8) -> f32 {
        unquantize_delta::<DELTA_RANGE>(quantized)
    }
}

impl<const DELTA_RANGE: u32> DeltaMarshaler<DELTA_RANGE, Vec3> {
    #[must_use]
    pub fn quantized_component(value: f32) -> u8 {
        quantize_delta::<DELTA_RANGE>(value)
    }

    #[must_use]
    pub fn unquantized_component(quantized: u8) -> f32 {
        unquantize_delta::<DELTA_RANGE>(quantized)
    }
}

impl<const DELTA_RANGE: u32> Codec<f32> for DeltaMarshaler<DELTA_RANGE, f32> {
    fn marshal(value: &f32, wb: &mut WriteBuffer) {
        wb.write_u8(quantize_delta::<DELTA_RANGE>(*value));
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<f32, MarshalerError> {
        Ok(unquantize_delta::<DELTA_RANGE>(rb.read_u8()?))
    }
}

impl<const DELTA_RANGE: u32> Codec<Vec3> for DeltaMarshaler<DELTA_RANGE, Vec3> {
    fn marshal(value: &Vec3, wb: &mut WriteBuffer) {
        wb.write_u8(quantize_delta::<DELTA_RANGE>(value.x));
        wb.write_u8(quantize_delta::<DELTA_RANGE>(value.y));
        wb.write_u8(quantize_delta::<DELTA_RANGE>(value.z));
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Vec3, MarshalerError> {
        Ok(Vec3::new(
            unquantize_delta::<DELTA_RANGE>(rb.read_u8()?),
            unquantize_delta::<DELTA_RANGE>(rb.read_u8()?),
            unquantize_delta::<DELTA_RANGE>(rb.read_u8()?),
        ))
    }
}

pub trait DeltaRangeValue:
    Copy + Default + PartialEq + std::ops::Add<Output = Self> + std::ops::Sub<Output = Self>
{
    fn is_within_delta(base: Self, other: Self, delta_range: u32) -> bool;
}

impl DeltaRangeValue for f32 {
    fn is_within_delta(base: Self, other: Self, delta_range: u32) -> bool {
        (base - other).abs() < u32_to_f32(delta_range)
    }
}

impl DeltaRangeValue for Vec3 {
    fn is_within_delta(base: Self, other: Self, delta_range: u32) -> bool {
        let abs_diff = (base - other).abs();
        let delta_range = u32_to_f32(delta_range);
        abs_diff.x < delta_range && abs_diff.y < delta_range && abs_diff.z < delta_range
    }
}

/// Delta-compressed field split into absolute and relative portions.
///
/// This preserves the wire-level absolute/relative split and set logic. The
/// timestamp accessor is intentionally left out because this field model does
/// not track per-field network timestamps yet.
///
/// The combined value is **always** computed on demand from the two
/// portions — there is no cached field. Mutate parts through their
/// handlers so sequence and dirty-state bookkeeping stays local to each slot.
#[derive(Debug, Clone)]
pub struct DeltaCompressedReplicatedFieldHandler<
    T: DeltaRangeValue,
    const DELTA_RANGE: u32,
    AbsoluteM: Codec<T> = DefaultMarshaler<T>,
    RelativeM: Codec<T> = DeltaMarshaler<DELTA_RANGE, T>,
> {
    pub absolute_portion: ReplicatedFieldHandler<T, AbsoluteM>,
    pub relative_portion: ReplicatedFieldHandler<T, RelativeM>,
}

impl<T: DeltaRangeValue, const DELTA_RANGE: u32, AbsoluteM: Codec<T>, RelativeM: Codec<T>> Default
    for DeltaCompressedReplicatedFieldHandler<T, DELTA_RANGE, AbsoluteM, RelativeM>
{
    fn default() -> Self {
        Self {
            absolute_portion: ReplicatedFieldHandler::default(),
            relative_portion: ReplicatedFieldHandler::default(),
        }
    }
}

impl<T: DeltaRangeValue, const DELTA_RANGE: u32, AbsoluteM: Codec<T>, RelativeM: Codec<T>>
    DeltaCompressedReplicatedFieldHandler<T, DELTA_RANGE, AbsoluteM, RelativeM>
{
    #[must_use]
    pub fn new(initial: T) -> Self {
        Self {
            absolute_portion: ReplicatedFieldHandler::some(initial),
            relative_portion: ReplicatedFieldHandler::some(T::default()),
        }
    }

    #[must_use]
    pub fn is_absolute_valid(&self) -> bool {
        self.absolute_portion.is_field_valid()
    }

    #[must_use]
    pub fn is_relative_valid(&self) -> bool {
        self.relative_portion.is_field_valid()
    }

    #[must_use]
    pub fn is_field_valid(&self) -> bool {
        self.is_absolute_valid()
    }

    /// Set the combined value, recomputing the absolute/relative split
    /// against the current absolute portion.
    pub fn set_value(&mut self, value: T) {
        if !self.is_absolute_valid() {
            self.absolute_portion.set_value(value);
            self.relative_portion.set_value(T::default());
            return;
        }

        let absolute = self.absolute_portion.value.unwrap_or_default();
        if T::is_within_delta(absolute, value, DELTA_RANGE) {
            self.relative_portion.set_value(value - absolute);
        } else {
            self.absolute_portion.set_value(value);
            self.relative_portion.set_value(T::default());
        }
    }

    /// Combined value — `absolute + relative`, computed on demand from
    /// whatever the parts currently hold. Returns `T::default()` when both
    /// portions are absent.
    #[must_use]
    pub fn value(&self) -> T {
        self.absolute_portion.value.unwrap_or_default()
            + self.relative_portion.value.unwrap_or_default()
    }
}

/// Delta-compressed counter for integer types.
///
/// Wire shape: two `ReplicatedFieldHandler<T>` slots — absolute
/// (high byte, via [`IntegerOmitLowerByteMarshaler`]) plus relative
/// (low byte cast, via [`DeltaIntegerMarshaler`]). Reconstructed
/// value is `absolute + relative` with modulo-256 wrap on the
/// relative portion.
///
/// `set_value` updates only the relative portion while the value remains
/// inside the current absolute window. Otherwise it re-anchors by storing
/// `new_value % 256` as the relative portion and the remaining high portion
/// as the absolute value.
#[derive(Debug, Clone, Default)]
pub struct DeltaCompressedCounterHandler {
    pub absolute_portion: ReplicatedFieldHandler<u16, IntegerOmitLowerByteMarshaler>,
    pub relative_portion: ReplicatedFieldHandler<u16, DeltaIntegerMarshaler>,
}

impl DeltaCompressedCounterHandler {
    #[must_use]
    pub fn is_absolute_valid(&self) -> bool {
        self.absolute_portion.is_field_valid()
    }

    /// `[absolute, absolute + 255]`.
    #[must_use]
    pub fn is_within_delta(&self, new_value: u16) -> bool {
        let abs = self.absolute_portion.value.unwrap_or(0);
        abs <= new_value && (new_value - abs) <= VALUES_IN_BYTE
    }

    /// the absolute portion when the new value would exceed the
    /// relative range or no absolute has been set yet.
    pub fn set_value(&mut self, new_value: u16) {
        if !self.is_absolute_valid() || !self.is_within_delta(new_value) {
            let rel = new_value % (VALUES_IN_BYTE + 1);
            self.absolute_portion.set_value(new_value - rel);
            self.relative_portion.set_value(rel);
        } else {
            let abs = self.absolute_portion.value.unwrap_or(0);
            self.relative_portion.set_value(new_value - abs);
        }
    }

    /// Reconstructed combined value. Returns `0` when no absolute has
    /// default-construct return.
    #[must_use]
    pub fn value(&self) -> u16 {
        self.absolute_portion.value.unwrap_or(0) + self.relative_portion.value.unwrap_or(0)
    }
}

/// The `[255, 255, 255]` sentinel indicates "no relative
/// portion present" (i.e. the absolute portion alone carries the value);
/// any other triple is read as three quantized `u8` axis deltas.
///
/// Used by [`DynamicDeltaReplicatedFieldHandler`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QuantizedRelativePosition {
    pub quantized_values: [u8; 3],
}

impl QuantizedRelativePosition {
    pub const ZERO_SENTINEL: Self = Self {
        quantized_values: [255, 255, 255],
    };

    #[must_use]
    pub const fn new(quantized_values: [u8; 3]) -> Self {
        Self { quantized_values }
    }

    /// signaling "no relative portion." Default-constructed positions
    /// are zero.
    #[must_use]
    pub fn is_zero(&self) -> bool {
        self.quantized_values == [255, 255, 255]
    }
}

impl Default for QuantizedRelativePosition {
    fn default() -> Self {
        Self::ZERO_SENTINEL
    }
}

impl From<[u8; 3]> for QuantizedRelativePosition {
    fn from(value: [u8; 3]) -> Self {
        Self::new(value)
    }
}

impl From<QuantizedRelativePosition> for [u8; 3] {
    fn from(value: QuantizedRelativePosition) -> Self {
        value.quantized_values
    }
}

#[must_use]
pub fn quantize_with_range(value: f32, delta_range: f32) -> u8 {
    let quantized = (value + delta_range) * 255.0 / (2.0 * delta_range);
    f32_to_u8(quantized.clamp(0.0, 255.0))
}

#[must_use]
pub fn unquantize_with_range(quantized: u8, delta_range: f32) -> f32 {
    2.0 * delta_range * f32::from(quantized) / 255.0 - delta_range
}

impl Marshaler for QuantizedRelativePosition {
    fn marshal(&self, wb: &mut WriteBuffer) {
        wb.write_u8(self.quantized_values[0]);
        wb.write_u8(self.quantized_values[1]);
        wb.write_u8(self.quantized_values[2]);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        Ok(Self {
            quantized_values: [rb.read_u8()?, rb.read_u8()?, rb.read_u8()?],
        })
    }
}

/// Dynamic delta-compressed vector field with a runtime-supplied quantization
/// scale.
///
/// Wire shape: three slots —
/// - `absolute_portion: ReplicatedFieldHandler<Vec3, M>` (default
///   `M = Vec3`, three raw f32 components);
/// - `quantized_relative_portion: ReplicatedFieldHandler<QuantizedRelativePosition>`
///   (3 bytes, see [`QuantizedRelativePosition`]);
/// - `quantization: ReplicatedFieldHandler<f32>` (single f32 scale).
///
/// `set_value(new_value, quantization)`:
/// - If absolute is unset, or `quantization` differs, or the new
///   value falls outside the per-component `< quantization` range:
///   re-anchor `absolute = new_value`, relative goes to the sentinel,
///   quantization slot stores the new scale.
/// - Otherwise the relative portion stores the quantized delta and
///   the absolute / quantization slots stay put.
#[derive(Debug, Clone, Default)]
pub struct DynamicDeltaReplicatedFieldHandler<AbsoluteM: Codec<Vec3> = DefaultMarshaler<Vec3>> {
    pub absolute_portion: ReplicatedFieldHandler<Vec3, AbsoluteM>,
    pub quantized_relative_portion: ReplicatedFieldHandler<QuantizedRelativePosition>,
    pub quantization: ReplicatedFieldHandler<f32>,
}

impl<AbsoluteM: Codec<Vec3>> DynamicDeltaReplicatedFieldHandler<AbsoluteM> {
    fn is_absolute_valid(&self) -> bool {
        self.absolute_portion.is_field_valid()
    }

    fn is_relative_zero(&self) -> bool {
        self.quantized_relative_portion
            .value
            .is_none_or(|v| v.is_zero())
    }

    fn is_within_delta(&self, new_value: Vec3, quantization: f32) -> bool {
        let abs = self.absolute_portion.value.unwrap_or(Vec3::ZERO);
        let abs_diff = (abs - new_value).abs();
        // for the sentinel.
        abs_diff.x < quantization && abs_diff.y < quantization && abs_diff.z < quantization
    }

    fn quantize(value: f32, quantization: f32) -> u8 {
        quantize_with_range(value, quantization)
    }

    fn unquantize(q: u8, quantization: f32) -> f32 {
        unquantize_with_range(q, quantization)
    }

    /// Reconstructed combined value. When the relative portion is the
    /// sentinel (`is_zero()`) only the absolute is used.
    #[must_use]
    pub fn value(&self) -> Vec3 {
        let Some(abs) = self.absolute_portion.value else {
            return Vec3::ZERO;
        };
        if self.is_relative_zero() || !self.quantization.has_value() {
            return abs;
        }
        let quantization = self.quantization.value.unwrap_or(0.0);
        let rel = self
            .quantized_relative_portion
            .value
            .unwrap_or_default()
            .quantized_values;
        Vec3::new(
            abs.x + Self::unquantize(rel[0], quantization),
            abs.y + Self::unquantize(rel[1], quantization),
            abs.z + Self::unquantize(rel[2], quantization),
        )
    }

    #[must_use]
    pub fn is_field_valid(&self) -> bool {
        self.is_absolute_valid()
    }

    pub fn set_value(&mut self, new_value: Vec3, quantization: f32) {
        let needs_anchor = !self.is_absolute_valid()
            || self.quantization.value != Some(quantization)
            || !self.is_within_delta(new_value, quantization);

        if needs_anchor {
            self.absolute_portion.set_value(new_value);
            self.quantized_relative_portion
                .set_value(QuantizedRelativePosition::default());
            self.quantization.set_value(quantization);
            return;
        }

        let abs = self.absolute_portion.value.unwrap_or(Vec3::ZERO);
        let diff = new_value - abs;
        let quantized = QuantizedRelativePosition {
            quantized_values: [
                Self::quantize(diff.x, quantization),
                Self::quantize(diff.y, quantization),
                Self::quantize(diff.z, quantization),
            ],
        };
        self.quantized_relative_portion.set_value(quantized);
    }
}

/// Delta-encoded float timer split into four independent
/// `ReplicatedFieldHandler<u8>` byte slots.
///
/// `set_value(seconds)`:
/// - Quantize: `q = (seconds % ROLLOVER_THRESHOLD) / (1 / QUANTIZATION)`
///   stored as `u32`.
/// - Diff against the previous quantized value and only set the bytes
///   that changed.
///
/// `value()` reassembles the four bytes into a u32 and reverses the
/// quantization.
///
/// Carries the four bytes as flat `ReplicatedFieldHandler<u8>` fields while
/// exposing them as one timer value.
#[derive(Debug, Clone)]
pub struct FloatTimerDeltaReplicatedField<
    const QUANTIZATION: u32 = 30,
    const ROLLOVER_THRESHOLD: u32 = 65535,
> {
    pub data: [ReplicatedFieldHandler<u8>; 4],
}

impl<const QUANTIZATION: u32, const ROLLOVER_THRESHOLD: u32> Default
    for FloatTimerDeltaReplicatedField<QUANTIZATION, ROLLOVER_THRESHOLD>
{
    fn default() -> Self {
        Self {
            data: std::array::from_fn(|_| ReplicatedFieldHandler::default()),
        }
    }
}

impl<const QUANTIZATION: u32, const ROLLOVER_THRESHOLD: u32>
    FloatTimerDeltaReplicatedField<QUANTIZATION, ROLLOVER_THRESHOLD>
{
    fn quantization_ratio() -> f32 {
        1.0 / u32_to_f32(QUANTIZATION)
    }

    #[must_use]
    fn rollover_divisor() -> u32 {
        if ROLLOVER_THRESHOLD > 0 {
            ROLLOVER_THRESHOLD
        } else {
            1
        }
    }

    fn quantize_value(value_in: f32) -> u32 {
        let divisor = Self::rollover_divisor();
        let divisions = f32_to_u32(value_in) / divisor;
        let clamped_time = value_in - u32_to_f32(divisions) * u32_to_f32(ROLLOVER_THRESHOLD);
        f32_to_u32(clamped_time / Self::quantization_ratio())
    }

    fn normalize_quantized_value(q: u32) -> f32 {
        u32_to_f32(q) * Self::quantization_ratio()
    }

    fn raw_quantized_value(&self) -> u32 {
        let b0 = u32::from(self.data[0].value.unwrap_or(0));
        let b1 = u32::from(self.data[1].value.unwrap_or(0));
        let b2 = u32::from(self.data[2].value.unwrap_or(0));
        let b3 = u32::from(self.data[3].value.unwrap_or(0));
        (b3 << 24) | (b2 << 16) | (b1 << 8) | b0
    }

    fn bytes_required(new_value: u32, old_value: u32) -> usize {
        let diff = new_value ^ old_value;
        if (diff & 0xff00_0000) != 0 {
            4
        } else if (diff & 0xffff_0000) != 0 {
            3
        } else if (diff & 0x0000_ff00) != 0 {
            2
        } else {
            1
        }
    }

    #[must_use]
    pub fn is_field_valid(&self) -> bool {
        self.data.iter().all(ReplicatedFieldHandler::is_field_valid)
    }

    #[must_use]
    pub fn value(&self) -> f32 {
        Self::normalize_quantized_value(self.raw_quantized_value())
    }

    pub fn set_value(&mut self, new_value: f32) {
        let quantized_new = Self::quantize_value(new_value);
        let bytes_needed = if self.is_field_valid() {
            let quantized_old = self.raw_quantized_value();
            Self::bytes_required(quantized_new, quantized_old)
        } else {
            4
        };

        if bytes_needed >= 4 {
            self.data[3].set_value(byte_from_u32(quantized_new >> 24));
        }
        if bytes_needed >= 3 {
            self.data[2].set_value(byte_from_u32(quantized_new >> 16));
        }
        if bytes_needed >= 2 {
            self.data[1].set_value(byte_from_u32(quantized_new >> 8));
        }
        self.data[0].set_value(byte_from_u32(quantized_new));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serialize::buffer::CARRIER_ENDIAN;

    fn roundtrip_field<T, M>(value: T) -> T
    where
        T: Default,
        M: Codec<T>,
    {
        let field = ReplicatedFieldHandler::<T, M> {
            value: Some(value),
            ..Default::default()
        };

        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        field.marshal(&mut wb);
        let bytes = wb.into_vec();

        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bytes);
        let decoded = ReplicatedFieldHandler::<T, M>::unmarshal(&mut rb).expect("unmarshal");
        assert_eq!(rb.left(), 0, "all bytes consumed");
        decoded.value.expect("decoded value")
    }

    #[test]
    fn test_delta_marshaler_f32_endpoint_roundtrip() {
        let min = roundtrip_field::<f32, DeltaMarshaler<10, f32>>(-10.0);
        let max = roundtrip_field::<f32, DeltaMarshaler<10, f32>>(10.0);
        assert!((min + 10.0).abs() < f32::EPSILON);
        assert!((max - 10.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_delta_marshaler_vec3_roundtrip() {
        let decoded = roundtrip_field::<Vec3, DeltaMarshaler<4, Vec3>>(Vec3::new(-4.0, 0.0, 4.0));
        assert!((decoded.x + 4.0).abs() < f32::EPSILON);
        assert!((decoded.y - (-0.015_686_035)).abs() < 0.0001);
        assert!((decoded.z - 4.0).abs() < f32::EPSILON);
    }

    #[test]
    fn replicated_field_set_value_is_local_dirty_not_network_data() {
        let mut field = ReplicatedFieldHandler::<u32>::default();
        field.set_value(42);

        assert_eq!(field.value, Some(42));
        assert_eq!(field.last_modified(), SequenceNumber::ValidNonSequence);
        assert!(field.is_dirty_since(SequenceNumber::Invalid));
        assert!(!field.has_new_network_data());
    }

    #[test]
    fn replicated_field_unmarshal_marks_new_network_data() {
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        42u32.marshal(&mut wb);
        let bytes = wb.into_vec();

        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bytes);
        let field = ReplicatedFieldHandler::<u32>::unmarshal(&mut rb).expect("unmarshal");

        assert_eq!(field.value, Some(42));
        assert_eq!(field.last_modified(), SequenceNumber::ValidNonSequence);
        assert!(field.has_new_network_data());
    }

    #[test]
    fn replicated_field_access_false_keeps_empty_field_absent() {
        let mut field = ReplicatedFieldHandler::<Vec<u32>>::default();
        field.access(|_| false);

        assert_eq!(field.value, None);
        assert_eq!(field.last_modified(), SequenceNumber::Invalid);
        assert!(!field.has_field_payload());
    }

    #[test]
    fn replicated_field_access_marks_modified_without_default_suppression() {
        let mut field = ReplicatedFieldHandler::<u32>::default();
        field.set_default_value(0);
        field.set_value(1);

        field.access(|value| {
            *value = 0;
            true
        });

        assert_eq!(field.value, Some(0));
        assert_eq!(
            field.last_modified(),
            SequenceNumber::ValidNonSequence,
            "access stamps a mutation; set_value owns default suppression"
        );
        assert!(field.is_dirty_since(SequenceNumber::Invalid));
    }

    #[test]
    fn replicated_field_merge_stamps_changed_value() {
        let mut old = ReplicatedFieldHandler::<u32>::default();
        old.set_value(1);
        old.set_last_modified(7);

        let mut new = ReplicatedFieldHandler::<u32>::default();
        new.set_value(2);

        let mut merged = ReplicatedFieldHandler::<u32>::default();
        let detected = merged.merge_and_update_sequence(&old, &new, 9, false);

        assert!(detected);
        assert_eq!(merged.value, Some(2));
        assert_eq!(merged.last_modified(), SequenceNumber::Seq(9));
        assert!(merged.has_new_network_data());
    }

    #[test]
    fn replicated_field_merge_preserves_equal_old_sequence() {
        let mut old = ReplicatedFieldHandler::<u32>::default();
        old.set_value(1);
        old.set_last_modified(7);

        let mut new = ReplicatedFieldHandler::<u32>::default();
        new.set_value(1);

        let mut merged = ReplicatedFieldHandler::<u32>::default();
        let detected = merged.merge_and_update_sequence(&old, &new, 9, false);

        assert!(!detected);
        assert_eq!(merged.value, Some(1));
        assert_eq!(merged.last_modified(), SequenceNumber::Seq(7));
        assert!(!merged.has_new_network_data());
    }

    #[test]
    fn test_delta_compressed_field_handler_set_within_delta() {
        let mut field = DeltaCompressedReplicatedFieldHandler::<f32, 5>::new(10.0);
        field.set_value(13.0);
        assert_eq!(field.absolute_portion.value, Some(10.0));
        assert_eq!(field.relative_portion.value, Some(3.0));
        assert!((field.value() - 13.0).abs() < f32::EPSILON);
        assert_eq!(
            field.relative_portion.last_modified(),
            SequenceNumber::ValidNonSequence
        );
        assert!(!field.relative_portion.has_new_network_data());
    }

    #[test]
    fn test_delta_compressed_field_handler_set_outside_delta() {
        let mut field = DeltaCompressedReplicatedFieldHandler::<f32, 5>::new(10.0);
        field.set_value(20.0);
        assert_eq!(field.absolute_portion.value, Some(20.0));
        assert_eq!(field.relative_portion.value, Some(0.0));
        assert!((field.value() - 20.0).abs() < f32::EPSILON);
        assert_eq!(
            field.absolute_portion.last_modified(),
            SequenceNumber::ValidNonSequence
        );
        assert_eq!(
            field.relative_portion.last_modified(),
            SequenceNumber::ValidNonSequence
        );
    }

    #[test]
    fn test_delta_compressed_field_handler_set_initializes_empty_absolute() {
        let mut field = DeltaCompressedReplicatedFieldHandler::<f32, 5>::default();
        field.set_value(13.0);

        assert_eq!(field.absolute_portion.value, Some(13.0));
        assert_eq!(field.relative_portion.value, Some(0.0));
        assert_eq!(
            field.absolute_portion.last_modified(),
            SequenceNumber::ValidNonSequence
        );
        assert_eq!(
            field.relative_portion.last_modified(),
            SequenceNumber::ValidNonSequence
        );
    }

    /// Confirms that mutating the parts directly is reflected by `get()`
    /// without any explicit "sync" step — the prior `combined_value`
    /// used to verify; now the read-through structure makes the bug
    /// unrepresentable.
    #[test]
    fn test_delta_compressed_field_handler_get_reads_through_parts() {
        let mut field = DeltaCompressedReplicatedFieldHandler::<Vec3, 8>::default();
        field
            .absolute_portion
            .set_value(Vec3::new(100.0, 200.0, 300.0));
        field.relative_portion.set_value(Vec3::new(1.0, -2.0, 3.0));
        assert_eq!(field.value(), Vec3::new(101.0, 198.0, 303.0));
    }

    #[test]
    fn dynamic_delta_quantization_matches_wire_range_mapping() {
        let mut handler = DynamicDeltaReplicatedFieldHandler::<DefaultMarshaler<Vec3>>::default();
        handler.set_value(Vec3::new(10.0, 20.0, 30.0), 4.0);

        handler.set_value(Vec3::new(8.0, 20.0, 32.0), 4.0);
        assert_eq!(
            handler.quantized_relative_portion.value.unwrap(),
            QuantizedRelativePosition {
                quantized_values: [63, 127, 191],
            }
        );

        let decoded = handler.value();
        assert!((decoded.x - 7.976_470_5).abs() < 0.001, "{decoded:?}");
        assert!((decoded.y - 19.984_314).abs() < 0.001, "{decoded:?}");
        assert!((decoded.z - 31.992_157).abs() < 0.001, "{decoded:?}");
    }
}
