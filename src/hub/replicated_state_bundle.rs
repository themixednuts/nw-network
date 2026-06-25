//!
//!
//! ```text
//! ReplicatedStateBundle:
//!   SequenceNumber seq
//!   u8             client_context_instance_id
//!   u8             bandwidth_mode
//!   bool           is_unreliable
//!   bool           includes_replication_control
//!   ReplicationControlData?  // only when the bool above is true
//!   VLQ32 bundle_buffer_len
//!   u8[bundle_buffer_len] bundle_buffer
//!
//! StateRecord:
//!   VLQ32 interest_id
//!   u8    fragment_count
//!   StateFragment[fragment_count]
//!
//! StateFragment:
//!   VLQ32 fragment_key
//!   FragmentTypeInfo
//! ```
//!
//! Fragment bodies are not length-prefixed. Readers must decode each body with
//! the resolved fragment descriptor before they can reach the next fragment.

use std::{cell::Cell, ops::AddAssign};

use arrayvec::ArrayVec;
use uuid::Uuid;

use super::sequence_number::SequenceNumber;
use super::{
    BandwidthMode, ClientContextId, DynFragment, Fragment, FragmentKey, FragmentRegistration,
    InterestId, MarshalContext, TypeIndex, fragment_registration_by_type_index,
    fragment_registration_by_uuid, fragment_type_index_by_uuid,
};
use crate::serialize::buffer::{CARRIER_ENDIAN, ReadBuffer, WriteBuffer};
use crate::serialize::error::MarshalerError;
use crate::serialize::marshaler::Marshaler;
use crate::serialize::vlq::VlqU32Marshaler;
use crate::types::{AzRtti, TypeRegistryEntry};

pub const MAX_REPLICATED_STATE_BUNDLE_BUFFER_SIZE: usize = 250 * 1024;
pub const MAX_REPLICATION_CONTROL_IDS: usize = 100;
pub const MAX_REPLICATION_CONTROL_MESSAGE_IDS: usize = 0x200;

fn capped_len_u32(len: usize) -> u32 {
    u32::try_from(len).expect("wire length cap fits in u32")
}

#[derive(
    nw_network_derive::Marshaler,
    Debug,
    Clone,
    Default,
    PartialEq,
    Eq,
    nw_network_derive::AzRtti,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("64CEE0FB-B878-47B3-8377-B45A9C9BA884")]
#[type_registry(18)]
pub struct ReplicationPerformanceData {
    pub collection_period_ms: u16,
    pub count_bundles: u32,
    pub count_throttled: u32,
    pub count_overflows: u32,
    pub count_characters_intended: u32,
    pub count_characters_actual: u32,
    pub characters_missed_due_to_rate_limiter: u32,
    pub objects_missed_due_to_rate_limiter: u32,
    pub characters_missed_due_to_overflow: u32,
    pub objects_missed_due_to_overflow: u32,
    pub characters_added_during_period: u32,
    pub objects_added_during_period: u32,
}

impl ReplicationPerformanceData {
    pub fn reset(&mut self) {
        let collection_period_ms = self.collection_period_ms;
        *self = Self {
            collection_period_ms,
            ..Self::default()
        };
    }

    pub fn add_counts(&mut self, rhs: &Self) {
        *self += rhs;
    }
}

impl AddAssign<&Self> for ReplicationPerformanceData {
    fn add_assign(&mut self, rhs: &Self) {
        self.collection_period_ms = rhs.collection_period_ms;
        self.count_bundles = self.count_bundles.wrapping_add(rhs.count_bundles);
        self.count_throttled = self.count_throttled.wrapping_add(rhs.count_throttled);
        self.count_overflows = self.count_overflows.wrapping_add(rhs.count_overflows);
        self.count_characters_intended = self
            .count_characters_intended
            .wrapping_add(rhs.count_characters_intended);
        self.count_characters_actual = self
            .count_characters_actual
            .wrapping_add(rhs.count_characters_actual);
        self.characters_missed_due_to_rate_limiter = self
            .characters_missed_due_to_rate_limiter
            .wrapping_add(rhs.characters_missed_due_to_rate_limiter);
        self.objects_missed_due_to_rate_limiter = self
            .objects_missed_due_to_rate_limiter
            .wrapping_add(rhs.objects_missed_due_to_rate_limiter);
        self.characters_missed_due_to_overflow = self
            .characters_missed_due_to_overflow
            .wrapping_add(rhs.characters_missed_due_to_overflow);
        self.objects_missed_due_to_overflow = self
            .objects_missed_due_to_overflow
            .wrapping_add(rhs.objects_missed_due_to_overflow);
        self.characters_added_during_period = self
            .characters_added_during_period
            .wrapping_add(rhs.characters_added_during_period);
        self.objects_added_during_period = self
            .objects_added_during_period
            .wrapping_add(rhs.objects_added_during_period);
    }
}

#[derive(
    Debug, Clone, Default, PartialEq, Eq, nw_network_derive::AzRtti, nw_network_derive::TypeRegistry,
)]
#[az_rtti(
    uuid = "FE59B513-CEB7-4BC2-80E6-545A8C492591",
    name = "Amazon::Hub::ReplicationControl"
)]
#[type_registry(9)]
pub struct ReplicationControl {
    pub seq: SequenceNumber,
    pub client_context_instance_id: u8,
    pub pause_start_idx: u32,
    pub interest_ids: ArrayVec<InterestId, MAX_REPLICATION_CONTROL_MESSAGE_IDS>,
}

impl ReplicationControl {
    #[must_use]
    pub const fn new(seq: SequenceNumber, client_context_instance_id: u8) -> Self {
        Self {
            seq,
            client_context_instance_id,
            pause_start_idx: 0,
            interest_ids: ArrayVec::new_const(),
        }
    }

    /// Build control ids from stop ids followed by pause ids.
    ///
    /// # Errors
    ///
    /// Returns [`MarshalerError::ContainerOverflow`] when the combined id count exceeds
    /// [`MAX_REPLICATION_CONTROL_MESSAGE_IDS`].
    pub fn from_ids<S, P>(
        seq: impl Into<SequenceNumber>,
        client_context_instance_id: u8,
        stop_replication_ids: &[S],
        pause_replication_ids: &[P],
    ) -> Result<Self, MarshalerError>
    where
        S: Copy + Into<InterestId>,
        P: Copy + Into<InterestId>,
    {
        let len = stop_replication_ids.len() + pause_replication_ids.len();
        if len > MAX_REPLICATION_CONTROL_MESSAGE_IDS {
            return Err(MarshalerError::ContainerOverflow {
                len,
                capacity: MAX_REPLICATION_CONTROL_MESSAGE_IDS,
            });
        }

        let mut interest_ids = ArrayVec::new();
        interest_ids.extend(stop_replication_ids.iter().copied().map(Into::into));
        interest_ids.extend(pause_replication_ids.iter().copied().map(Into::into));

        let pause_start_idx = u32::try_from(stop_replication_ids.len()).map_err(|_| {
            MarshalerError::ContainerOverflow {
                len: stop_replication_ids.len(),
                capacity: MAX_REPLICATION_CONTROL_MESSAGE_IDS,
            }
        })?;

        Ok(Self {
            seq: seq.into(),
            client_context_instance_id,
            interest_ids,
            pause_start_idx,
        })
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.interest_ids.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.interest_ids.is_empty()
    }

    #[must_use]
    pub fn stop_replication_ids(&self) -> &[InterestId] {
        let split = self.pause_start();
        &self.interest_ids[..split]
    }

    #[must_use]
    pub fn pause_replication_ids(&self) -> &[InterestId] {
        let split = self.pause_start();
        &self.interest_ids[split..]
    }

    fn pause_start(&self) -> usize {
        usize::try_from(self.pause_start_idx)
            .unwrap_or(usize::MAX)
            .min(self.interest_ids.len())
    }
}

impl Marshaler for ReplicationControl {
    fn marshal(&self, wb: &mut WriteBuffer) {
        self.seq.marshal(wb);
        self.client_context_instance_id.marshal(wb);
        VlqU32Marshaler.marshal(wb, self.pause_start_idx);
        VlqU32Marshaler.marshal(wb, capped_len_u32(self.interest_ids.len()));
        for interest_id in &self.interest_ids {
            interest_id.get().marshal(wb);
        }
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let seq = SequenceNumber::unmarshal(rb)?;
        let client_context_instance_id = u8::unmarshal(rb)?;
        let pause_start_idx = VlqU32Marshaler.unmarshal(rb)?;
        let len = usize::try_from(VlqU32Marshaler.unmarshal(rb)?).map_err(|_| {
            MarshalerError::ContainerOverflow {
                len: usize::MAX,
                capacity: MAX_REPLICATION_CONTROL_MESSAGE_IDS,
            }
        })?;
        if len > MAX_REPLICATION_CONTROL_MESSAGE_IDS {
            return Err(MarshalerError::ContainerOverflow {
                len,
                capacity: MAX_REPLICATION_CONTROL_MESSAGE_IDS,
            });
        }

        let mut interest_ids = ArrayVec::new();
        for _ in 0..len {
            interest_ids.push(InterestId::new(u16::unmarshal(rb)?));
        }

        Ok(Self {
            seq,
            client_context_instance_id,
            pause_start_idx,
            interest_ids,
        })
    }
}

/// Replication-control ids split into stop ids followed by pause ids.
///
/// The wire payload carries the total id count, the pause partition start, and
/// then the ordered id list.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReplicationControlData {
    pause_start_idx: u32,
    interest_ids: ArrayVec<InterestId, MAX_REPLICATION_CONTROL_IDS>,
}

impl ReplicationControlData {
    /// Build replication-control data from stop ids followed by pause ids.
    ///
    /// # Errors
    ///
    /// Returns [`MarshalerError::ContainerOverflow`] when the combined id count exceeds
    /// [`MAX_REPLICATION_CONTROL_IDS`].
    pub fn from_ids<S, P>(
        stop_replication_ids: &[S],
        pause_replication_ids: &[P],
    ) -> Result<Self, MarshalerError>
    where
        S: Copy + Into<InterestId>,
        P: Copy + Into<InterestId>,
    {
        let len = stop_replication_ids.len() + pause_replication_ids.len();
        if len > MAX_REPLICATION_CONTROL_IDS {
            return Err(MarshalerError::ContainerOverflow {
                len,
                capacity: MAX_REPLICATION_CONTROL_IDS,
            });
        }

        let mut interest_ids = ArrayVec::new();
        interest_ids.extend(stop_replication_ids.iter().copied().map(Into::into));
        interest_ids.extend(pause_replication_ids.iter().copied().map(Into::into));
        Ok(Self {
            pause_start_idx: capped_len_u32(stop_replication_ids.len()),
            interest_ids,
        })
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.interest_ids.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.interest_ids.is_empty()
    }

    #[must_use]
    pub const fn pause_start_idx(&self) -> u32 {
        self.pause_start_idx
    }

    #[must_use]
    pub const fn stop_replication_count(&self) -> u32 {
        self.pause_start_idx
    }

    #[must_use]
    pub fn pause_replication_count(&self) -> u32 {
        capped_len_u32(self.interest_ids.len()) - self.pause_start_idx
    }

    #[must_use]
    pub fn stop_replication_ids(&self) -> &[InterestId] {
        &self.interest_ids[..self.pause_start()]
    }

    #[must_use]
    pub fn pause_replication_ids(&self) -> &[InterestId] {
        &self.interest_ids[self.pause_start()..]
    }

    /// Add an id to the stop-replication partition.
    ///
    /// # Errors
    ///
    /// Returns [`MarshalerError::ContainerOverflow`] when the fixed control-id cap is full.
    pub fn push_stop_replication_id(
        &mut self,
        interest_id: impl Into<InterestId>,
    ) -> Result<(), MarshalerError> {
        if self.interest_ids.len() == MAX_REPLICATION_CONTROL_IDS {
            return Err(MarshalerError::ContainerOverflow {
                len: self.interest_ids.len() + 1,
                capacity: MAX_REPLICATION_CONTROL_IDS,
            });
        }
        let interest_id = interest_id.into();
        self.interest_ids.insert(self.pause_start(), interest_id);
        self.pause_start_idx += 1;
        Ok(())
    }

    /// Add an id to the pause-replication partition.
    ///
    /// # Errors
    ///
    /// Returns [`MarshalerError::ContainerOverflow`] when the fixed control-id cap is full.
    pub fn push_pause_replication_id(
        &mut self,
        interest_id: impl Into<InterestId>,
    ) -> Result<(), MarshalerError> {
        if self.interest_ids.len() == MAX_REPLICATION_CONTROL_IDS {
            return Err(MarshalerError::ContainerOverflow {
                len: self.interest_ids.len() + 1,
                capacity: MAX_REPLICATION_CONTROL_IDS,
            });
        }
        let interest_id = interest_id.into();
        self.interest_ids.push(interest_id);
        Ok(())
    }

    pub fn reset(&mut self) {
        self.interest_ids.clear();
        self.pause_start_idx = 0;
    }

    pub fn remove_pending_stop_id(&mut self, interest_id: impl Into<InterestId>) -> bool {
        let interest_id = interest_id.into();
        let Some(idx) = self
            .stop_replication_ids()
            .iter()
            .position(|id| *id == interest_id)
        else {
            return false;
        };
        self.interest_ids.remove(idx);
        self.pause_start_idx -= 1;
        true
    }

    fn pause_start(&self) -> usize {
        usize::try_from(self.pause_start_idx).expect("pause index fits in usize")
    }
}

impl Marshaler for ReplicationControlData {
    fn marshal(&self, wb: &mut WriteBuffer) {
        debug_assert!(
            self.pause_start() <= self.interest_ids.len(),
            "ReplicationControlData split must be within the id list"
        );
        wb.write_u8(u8::try_from(self.interest_ids.len()).expect("control id cap fits in u8"));
        wb.write_u8(u8::try_from(self.pause_start_idx).expect("control id cap fits in u8"));
        for interest_id in &self.interest_ids {
            interest_id.get().marshal(wb);
        }
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let len = usize::from(rb.read_u8()?);
        if len > MAX_REPLICATION_CONTROL_IDS {
            return Err(MarshalerError::ContainerOverflow {
                len,
                capacity: MAX_REPLICATION_CONTROL_IDS,
            });
        }
        let pause_start_idx = rb.read_u8()?;
        if usize::from(pause_start_idx) > len {
            return Err(MarshalerError::InvalidRange {
                value: u64::from(pause_start_idx),
                min: 0,
                max: u64::try_from(len).expect("control id count fits in u64"),
            });
        }
        let mut interest_ids = ArrayVec::new();
        for _ in 0..len {
            interest_ids.push(InterestId::new(u16::unmarshal(rb)?));
        }

        Ok(Self {
            pause_start_idx: u32::from(pause_start_idx),
            interest_ids,
        })
    }
}

#[derive(
    Debug, Clone, Default, PartialEq, Eq, nw_network_derive::AzRtti, nw_network_derive::TypeRegistry,
)]
#[az_rtti("8A40AEC2-AE07-4F92-9BF3-78FC0CC94FDF")]
#[type_registry(8)]
pub struct ReplicatedStateBundle {
    pub seq: SequenceNumber,
    pub client_context_instance_id: u8,
    pub bandwidth_mode: u8,
    pub is_unreliable: bool,
    pub replication_control: Option<ReplicationControlData>,
    pub bundle_buffer: Vec<u8>,
}

impl ReplicatedStateBundle {
    #[must_use]
    pub fn new(
        client_context_instance_id: impl Into<ClientContextId>,
        bandwidth_mode: impl Into<BandwidthMode>,
    ) -> Self {
        Self {
            client_context_instance_id: client_context_instance_id.into().get(),
            bandwidth_mode: bandwidth_mode.into().get(),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn with_seq(
        seq: impl Into<SequenceNumber>,
        client_context_instance_id: impl Into<ClientContextId>,
        bandwidth_mode: impl Into<BandwidthMode>,
    ) -> Self {
        Self {
            seq: seq.into(),
            client_context_instance_id: client_context_instance_id.into().get(),
            bandwidth_mode: bandwidth_mode.into().get(),
            ..Self::default()
        }
    }

    #[must_use]
    pub fn with_bundle_buffer(bundle_buffer: Vec<u8>) -> Self {
        Self {
            bundle_buffer,
            ..Self::default()
        }
    }

    #[must_use]
    pub const fn seq(&self) -> SequenceNumber {
        self.seq
    }

    pub fn set_seq(&mut self, seq: impl Into<SequenceNumber>) {
        self.seq = seq.into();
    }

    #[must_use]
    pub const fn client_context_instance_id(&self) -> u8 {
        self.client_context_instance_id
    }

    #[must_use]
    pub const fn client_context_id(&self) -> ClientContextId {
        ClientContextId::new(self.client_context_instance_id)
    }

    #[must_use]
    pub const fn bandwidth_mode(&self) -> u8 {
        self.bandwidth_mode
    }

    #[must_use]
    pub const fn bandwidth(&self) -> BandwidthMode {
        BandwidthMode::new(self.bandwidth_mode)
    }

    #[must_use]
    pub const fn is_unreliable(&self) -> bool {
        self.is_unreliable
    }

    pub fn set_unreliable(&mut self) {
        self.is_unreliable = true;
    }

    #[must_use]
    pub const fn includes_replication_control(&self) -> bool {
        self.replication_control.is_some()
    }

    pub fn include_replication_control(&mut self) {
        self.ensure_replication_control();
    }

    #[must_use]
    pub const fn has_replication_control(&self) -> bool {
        self.replication_control.is_some()
    }

    #[must_use]
    pub fn replication_control_count(&self) -> usize {
        self.replication_control
            .as_ref()
            .map_or(0, ReplicationControlData::len)
    }

    #[must_use]
    pub const fn has_state_records_payload(&self) -> bool {
        !self.bundle_buffer.is_empty()
    }

    /// Add a stop-replication id, creating the control block if needed.
    ///
    /// # Errors
    ///
    /// Returns [`MarshalerError::ContainerOverflow`] when the control-id cap is full.
    pub fn add_stop_id(
        &mut self,
        interest_id: impl Into<InterestId>,
    ) -> Result<(), MarshalerError> {
        self.ensure_replication_control()
            .push_stop_replication_id(interest_id)
    }

    /// Add a pause-replication id, creating the control block if needed.
    ///
    /// # Errors
    ///
    /// Returns [`MarshalerError::ContainerOverflow`] when the control-id cap is full.
    pub fn add_pause_id(
        &mut self,
        interest_id: impl Into<InterestId>,
    ) -> Result<(), MarshalerError> {
        self.ensure_replication_control()
            .push_pause_replication_id(interest_id)
    }

    #[must_use]
    pub fn stop_replication_ids(&self) -> &[InterestId] {
        self.replication_control
            .as_ref()
            .map_or(&[], ReplicationControlData::stop_replication_ids)
    }

    #[must_use]
    pub fn pause_replication_ids(&self) -> &[InterestId] {
        self.replication_control
            .as_ref()
            .map_or(&[], ReplicationControlData::pause_replication_ids)
    }

    pub fn reset_replication_control(&mut self) {
        if let Some(replication_control) = &mut self.replication_control {
            replication_control.reset();
        }
    }

    #[must_use]
    pub fn remove_pending_stop_id(&mut self, interest_id: impl Into<InterestId>) -> bool {
        let interest_id = interest_id.into();
        self.replication_control
            .as_mut()
            .is_some_and(|replication_control| {
                replication_control.remove_pending_stop_id(interest_id)
            })
    }

    #[must_use]
    pub fn written_size(&self) -> usize {
        self.bundle_buffer.len()
    }

    #[must_use]
    pub fn total_bundle_size(&self) -> usize {
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        self.marshal(&mut wb);
        wb.len()
    }

    /// Append one state record to the bundle buffer.
    ///
    /// # Errors
    ///
    /// Returns the first error reported by the record writer.
    pub fn write_record<R>(
        &mut self,
        interest_id: impl Into<InterestId>,
        write: impl FnOnce(&mut StateRecordWriter<'_>) -> Result<R, MarshalerError>,
    ) -> Result<R, MarshalerError> {
        let mut wb = WriteBuffer::from_vec(CARRIER_ENDIAN, std::mem::take(&mut self.bundle_buffer));
        let result = write_state_record(&mut wb, interest_id, write);
        self.bundle_buffer = wb.into_vec();
        result
    }

    /// Visit decoded fragment headers with a body cursor.
    ///
    /// # Errors
    ///
    /// Returns the first error reported by header decoding or the visitor.
    pub fn visit_fragments<F>(&self, visit: F) -> Result<(), MarshalerError>
    where
        F: FnMut(
            StateRecordHeader,
            StateFragmentHeaderSpan,
            &mut ReadBuffer<'_>,
        ) -> Result<(), MarshalerError>,
    {
        ReplicatedStateBundleView {
            seq: self.seq,
            client_context_instance_id: self.client_context_instance_id,
            bandwidth_mode: self.bandwidth_mode,
            is_unreliable: self.is_unreliable,
            replication_control: self.replication_control.clone(),
            bundle_buffer: &self.bundle_buffer,
            total_bundle_size: self.total_bundle_size(),
        }
        .visit_fragments(visit)
    }

    fn ensure_replication_control(&mut self) -> &mut ReplicationControlData {
        self.replication_control
            .get_or_insert_with(Default::default)
    }
}

impl Marshaler for ReplicatedStateBundle {
    fn marshal(&self, wb: &mut WriteBuffer) {
        self.seq.marshal(wb);
        self.client_context_instance_id.marshal(wb);
        self.bandwidth_mode.marshal(wb);
        self.is_unreliable.marshal(wb);
        self.replication_control.is_some().marshal(wb);
        if let Some(replication_control) = &self.replication_control {
            replication_control.marshal(wb);
        }
        marshal_bundle_buffer(&self.bundle_buffer, wb);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        ReplicatedStateBundleView::read_from(rb).map(ReplicatedStateBundleView::into_owned)
    }
}

/// Borrowed receive-side view. `bundle_buffer` points into the carrier message
/// body instead of allocating a second `Vec<u8>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplicatedStateBundleView<'a> {
    pub seq: SequenceNumber,
    pub client_context_instance_id: u8,
    pub bandwidth_mode: u8,
    pub is_unreliable: bool,
    pub replication_control: Option<ReplicationControlData>,
    pub bundle_buffer: &'a [u8],
    pub total_bundle_size: usize,
}

impl<'a> ReplicatedStateBundleView<'a> {
    /// Read a borrowed bundle view from a carrier message body.
    ///
    /// # Errors
    ///
    /// Returns an error when the header, replication-control data, or payload length is malformed.
    pub fn read_from(rb: &mut ReadBuffer<'a>) -> Result<Self, MarshalerError> {
        let start = rb.position();
        let seq = SequenceNumber::unmarshal(rb)?;
        let client_context_instance_id = u8::unmarshal(rb)?;
        let bandwidth_mode = u8::unmarshal(rb)?;
        let is_unreliable = bool::unmarshal(rb)?;
        let includes_replication_control = bool::unmarshal(rb)?;
        let replication_control = if includes_replication_control {
            Some(ReplicationControlData::unmarshal(rb)?)
        } else {
            None
        };
        let bundle_buffer = read_bundle_buffer(rb)?;
        let total_bundle_size = rb.position() - start;
        Ok(Self {
            seq,
            client_context_instance_id,
            bandwidth_mode,
            is_unreliable,
            replication_control,
            bundle_buffer,
            total_bundle_size,
        })
    }

    #[must_use]
    pub fn into_owned(self) -> ReplicatedStateBundle {
        ReplicatedStateBundle {
            seq: self.seq,
            client_context_instance_id: self.client_context_instance_id,
            bandwidth_mode: self.bandwidth_mode,
            is_unreliable: self.is_unreliable,
            replication_control: self.replication_control,
            bundle_buffer: self.bundle_buffer.to_vec(),
        }
    }

    #[must_use]
    pub const fn has_replication_control(&self) -> bool {
        self.replication_control.is_some()
    }

    #[must_use]
    pub fn replication_control_count(&self) -> usize {
        self.replication_control
            .as_ref()
            .map_or(0, ReplicationControlData::len)
    }

    #[must_use]
    pub const fn total_bundle_size(&self) -> usize {
        self.total_bundle_size
    }

    #[must_use]
    pub const fn has_state_records_payload(&self) -> bool {
        !self.bundle_buffer.is_empty()
    }

    #[must_use]
    pub fn stop_replication_ids(&self) -> &[InterestId] {
        self.replication_control
            .as_ref()
            .map_or(&[], ReplicationControlData::stop_replication_ids)
    }

    #[must_use]
    pub fn pause_replication_ids(&self) -> &[InterestId] {
        self.replication_control
            .as_ref()
            .map_or(&[], ReplicationControlData::pause_replication_ids)
    }

    /// Visit decoded fragment headers with a body cursor.
    ///
    /// # Errors
    ///
    /// Returns the first error reported by header decoding or the visitor.
    pub fn visit_fragments<F>(&self, mut visit: F) -> Result<(), MarshalerError>
    where
        F: FnMut(
            StateRecordHeader,
            StateFragmentHeaderSpan,
            &mut ReadBuffer<'a>,
        ) -> Result<(), MarshalerError>,
    {
        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, self.bundle_buffer);
        while !rb.is_empty() {
            let record_header = read_state_record_header(&mut rb)?;
            for _ in 0..record_header.fragment_count {
                let fragment_header = read_state_fragment_header(&mut rb)?;
                visit(record_header, fragment_header, &mut rb)?;
            }
        }
        Ok(())
    }
}

pub fn marshal_bundle_buffer(bundle_buffer: &[u8], wb: &mut WriteBuffer) {
    debug_assert!(
        bundle_buffer.len() <= MAX_REPLICATED_STATE_BUNDLE_BUFFER_SIZE,
        "ReplicatedStateBundle buffer exceeds bundle buffer cap"
    );
    VlqU32Marshaler.marshal(wb, capped_len_u32(bundle_buffer.len()));
    wb.write_bytes(bundle_buffer);
}

/// Read a length-prefixed bundle payload as a borrowed byte slice.
///
/// # Errors
///
/// Returns an error when the declared length exceeds the bundle cap or the buffer is truncated.
pub fn read_bundle_buffer<'a>(rb: &mut ReadBuffer<'a>) -> Result<&'a [u8], MarshalerError> {
    let len = usize::try_from(VlqU32Marshaler.unmarshal(rb)?).map_err(|_| {
        MarshalerError::ContainerOverflow {
            len: usize::MAX,
            capacity: MAX_REPLICATED_STATE_BUNDLE_BUFFER_SIZE,
        }
    })?;
    if len > MAX_REPLICATED_STATE_BUNDLE_BUFFER_SIZE {
        return Err(MarshalerError::ContainerOverflow {
            len,
            capacity: MAX_REPLICATED_STATE_BUNDLE_BUFFER_SIZE,
        });
    }
    rb.read_bytes(len)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StateRecordHeader {
    pub interest_id: InterestId,
    pub fragment_count: usize,
    pub start: usize,
    pub header_end: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StateFragmentHeaderSpan {
    pub fragment_key: FragmentKey,
    pub type_info: FragmentTypeInfo,
    pub start: usize,
    pub header_end: usize,
    pub body_start: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FragmentTypeInfo {
    TypeIndex(u32),
    RawUuid(Uuid),
}

impl Default for FragmentTypeInfo {
    fn default() -> Self {
        Self::RawUuid(Uuid::nil())
    }
}

impl FragmentTypeInfo {
    #[must_use]
    pub fn registered<C>() -> Self
    where
        C: TypeRegistryEntry,
    {
        Self::TypeIndex(C::TYPE_INDEX)
    }

    #[must_use]
    pub fn raw<C>() -> Self
    where
        C: AzRtti,
    {
        Self::RawUuid(C::TYPE_ID)
    }

    #[must_use]
    pub fn uuid(self) -> Option<Uuid> {
        match self {
            Self::TypeIndex(type_index) => {
                fragment_registration_by_type_index(type_index).map(|entry| (entry.uuid)())
            }
            Self::RawUuid(uuid) => Some(uuid),
        }
    }

    #[must_use]
    pub fn type_index(self) -> Option<u32> {
        match self {
            Self::TypeIndex(type_index) => Some(type_index),
            Self::RawUuid(uuid) => fragment_type_index_by_uuid(uuid),
        }
    }

    #[must_use]
    pub fn compact_type_index(self) -> Option<TypeIndex> {
        self.type_index().map(TypeIndex::new)
    }

    #[must_use]
    pub const fn raw_uuid(self) -> Option<Uuid> {
        match self {
            Self::TypeIndex(_) => None,
            Self::RawUuid(uuid) => Some(uuid),
        }
    }

    /// Read fragment type info.
    ///
    /// # Errors
    ///
    /// Returns an error when the VLQ type index or raw UUID bytes are truncated.
    pub fn read_from(rb: &mut ReadBuffer<'_>) -> Result<Self, MarshalerError> {
        read_fragment_type_info(rb)
    }

    pub fn write_to(self, wb: &mut WriteBuffer) {
        write_fragment_type_info(wb, self);
    }

    /// Resolve this type info against the registered fragment table.
    ///
    /// # Errors
    ///
    /// Returns an error when no registered fragment matches the compact type
    /// index or raw UUID.
    pub fn registration(self) -> Result<&'static FragmentRegistration, MarshalerError> {
        match self {
            Self::TypeIndex(type_index) => fragment_registration_by_type_index(type_index)
                .ok_or(MarshalerError::UnknownTypeIndex { type_index }),
            Self::RawUuid(uuid) => {
                fragment_registration_by_uuid(uuid).ok_or(MarshalerError::UnknownClassUuid)
            }
        }
    }

    /// Decode fragment contents using the registration selected by this type info.
    ///
    /// # Errors
    ///
    /// Returns an error when the type info is unknown or the contents decoder fails.
    pub fn decode_contents(
        self,
        rb: &mut ReadBuffer<'_>,
    ) -> Result<Box<dyn Fragment>, MarshalerError> {
        let registration = self.registration()?;
        (registration.decode_contents)(rb)
    }

    /// Consume fragment contents using the registration selected by this type info.
    ///
    /// # Errors
    ///
    /// Returns an error when the type info is unknown or the contents decoder fails.
    pub fn consume_contents(self, rb: &mut ReadBuffer<'_>) -> Result<(), MarshalerError> {
        let registration = self.registration()?;
        (registration.consume_contents)(rb)
    }

    /// Decode a full fragment using the registration selected by this type info.
    ///
    /// # Errors
    ///
    /// Returns an error when the type info is unknown or any full-fragment decoder fails.
    pub fn decode_full(self, rb: &mut ReadBuffer<'_>) -> Result<Box<dyn Fragment>, MarshalerError> {
        let registration = self.registration()?;
        (registration.decode_full)(rb)
    }

    /// Consume a full fragment using the registration selected by this type info.
    ///
    /// # Errors
    ///
    /// Returns an error when the type info is unknown or any full-fragment decoder fails.
    pub fn consume_full(self, rb: &mut ReadBuffer<'_>) -> Result<(), MarshalerError> {
        let registration = self.registration()?;
        (registration.consume_full)(rb)
    }
}

/// Borrowed baselineable fragment payload selected by a preceding [`FragmentTypeInfo`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BaselineableFragmentRef<'a> {
    pub type_info: FragmentTypeInfo,
    pub body: &'a [u8],
}

impl<'a> BaselineableFragmentRef<'a> {
    /// Read type info and borrow the exact body range consumed by its registration.
    ///
    /// # Errors
    ///
    /// Returns an error when the type info is unknown or any body section decoder fails.
    pub fn read_from(rb: &mut ReadBuffer<'a>) -> Result<Self, MarshalerError> {
        let type_info = FragmentTypeInfo::read_from(rb)?;
        let body_start = rb.position();
        type_info.consume_full(rb)?;
        let body_end = rb.position();
        Ok(Self {
            type_info,
            body: rb.range(body_start..body_end)?,
        })
    }

    /// Decode the borrowed body as a registered fragment.
    ///
    /// # Errors
    ///
    /// Returns an error when any body section decoder fails.
    pub fn decode(self) -> Result<Box<dyn Fragment>, MarshalerError> {
        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, self.body);
        self.type_info.decode_full(&mut rb)
    }

    #[must_use]
    pub fn into_owned(self) -> BaselineableFragment {
        BaselineableFragment {
            type_info: self.type_info,
            body: self.body.to_vec(),
        }
    }
}

/// Owned baselineable fragment payload selected by a preceding [`FragmentTypeInfo`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BaselineableFragment {
    pub type_info: FragmentTypeInfo,
    pub body: Vec<u8>,
}

impl BaselineableFragment {
    #[must_use]
    pub fn new(type_info: FragmentTypeInfo, body: impl Into<Vec<u8>>) -> Self {
        Self {
            type_info,
            body: body.into(),
        }
    }

    #[must_use]
    pub fn as_ref(&self) -> BaselineableFragmentRef<'_> {
        BaselineableFragmentRef {
            type_info: self.type_info,
            body: &self.body,
        }
    }

    /// Encode a registered fragment as a full body.
    #[must_use]
    pub fn encode<C>(fragment: &C, marshal_context: &MarshalContext<'_>) -> Self
    where
        C: DynFragment + TypeRegistryEntry,
    {
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        fragment.full_marshal(marshal_context, &mut wb);
        Self {
            type_info: FragmentTypeInfo::registered::<C>(),
            body: wb.into_vec(),
        }
    }

    /// Decode the owned body as a registered fragment.
    ///
    /// # Errors
    ///
    /// Returns an error when any body section decoder fails.
    pub fn decode(&self) -> Result<Box<dyn Fragment>, MarshalerError> {
        self.as_ref().decode()
    }
}

impl From<BaselineableFragmentRef<'_>> for BaselineableFragment {
    fn from(value: BaselineableFragmentRef<'_>) -> Self {
        value.into_owned()
    }
}

impl Marshaler for BaselineableFragment {
    fn marshal(&self, wb: &mut WriteBuffer) {
        self.type_info.write_to(wb);
        wb.write_bytes(&self.body);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        BaselineableFragmentRef::read_from(rb).map(Into::into)
    }
}

/// Read a state-record header.
///
/// # Errors
///
/// Returns an error when the interest id VLQ is malformed or exceeds the supported id range.
pub fn read_state_record_header(
    rb: &mut ReadBuffer<'_>,
) -> Result<StateRecordHeader, MarshalerError> {
    let start = rb.position();
    let interest_id = VlqU32Marshaler.unmarshal(rb)?;
    if interest_id > u32::from(u16::MAX) {
        return Err(MarshalerError::InvalidRange {
            value: u64::from(interest_id),
            min: 0,
            max: u64::from(u16::MAX),
        });
    }
    let interest_id = u16::try_from(interest_id).map_err(|_| MarshalerError::InvalidRange {
        value: u64::from(interest_id),
        min: 0,
        max: u64::from(u16::MAX),
    })?;
    let fragment_count = usize::from(rb.read_u8()?);
    Ok(StateRecordHeader {
        interest_id: InterestId::new(interest_id),
        fragment_count,
        start,
        header_end: rb.position(),
    })
}

/// Read a state-fragment header.
///
/// # Errors
///
/// Returns an error when the fragment key, type info, or raw UUID bytes are malformed.
pub fn read_state_fragment_header(
    rb: &mut ReadBuffer<'_>,
) -> Result<StateFragmentHeaderSpan, MarshalerError> {
    let start = rb.position();
    let fragment_key = FragmentKey::new(VlqU32Marshaler.unmarshal(rb)?);
    let type_info = read_fragment_type_info(rb)?;
    let header_end = rb.position();
    Ok(StateFragmentHeaderSpan {
        fragment_key,
        type_info,
        start,
        header_end,
        body_start: header_end,
    })
}

/// Read fragment type info.
///
/// # Errors
///
/// Returns an error when the VLQ type index or raw UUID bytes are truncated.
pub fn read_fragment_type_info(
    rb: &mut ReadBuffer<'_>,
) -> Result<FragmentTypeInfo, MarshalerError> {
    let type_index = VlqU32Marshaler.unmarshal(rb)?;
    if type_index == 0 {
        return Ok(FragmentTypeInfo::RawUuid(Uuid::from_bytes(
            *rb.read_array::<16>()?,
        )));
    }

    Ok(FragmentTypeInfo::TypeIndex(type_index))
}

pub fn write_fragment_type_info(wb: &mut WriteBuffer, type_info: FragmentTypeInfo) {
    match type_info {
        FragmentTypeInfo::TypeIndex(type_index) => {
            debug_assert!(type_index != 0, "zero typeIndex must be encoded as RawUuid");
            VlqU32Marshaler.marshal(wb, type_index);
        }
        FragmentTypeInfo::RawUuid(uuid) => {
            VlqU32Marshaler.marshal(wb, 0);
            wb.write_bytes(uuid.as_bytes());
        }
    }
}

/// Decode a concrete fragment body.
///
/// # Errors
///
/// Returns the first error reported by the fragment's body decoder.
pub fn decode_state_fragment_contents<T>(rb: &mut ReadBuffer<'_>) -> Result<T, MarshalerError>
where
    T: DynFragment + Default,
{
    let mut fragment = T::default();
    fragment.unmarshal_contents(rb)?;
    Ok(fragment)
}

/// Write a state record with a patched fragment count prefix.
///
/// # Errors
///
/// Returns the first error reported by the record body writer.
pub fn write_state_record<R>(
    wb: &mut WriteBuffer,
    interest_id: impl Into<InterestId>,
    write: impl FnOnce(&mut StateRecordWriter<'_>) -> Result<R, MarshalerError>,
) -> Result<R, MarshalerError> {
    let record_start = wb.mark();
    VlqU32Marshaler.marshal(wb, u32::from(interest_id.into().get()));

    let count = Cell::new(0u8);
    let result = wb.with_fixed_prefix(
        1,
        |wb| {
            let mut record = StateRecordWriter { wb, count: 0 };
            let result = write(&mut record);
            count.set(record.count);
            result
        },
        |prefix, _body| {
            prefix[0] = count.get();
        },
    );

    if result.is_err() || count.get() == 0 {
        wb.truncate_to(record_start);
    }
    result
}

pub struct StateRecordWriter<'a> {
    wb: &'a mut WriteBuffer,
    count: u8,
}

impl StateRecordWriter<'_> {
    /// Write a registered fragment body into this record.
    ///
    /// # Errors
    ///
    /// Returns an error when the record fragment count or bundle byte cap would be exceeded.
    pub fn write_fragment<C>(
        &mut self,
        fragment_key: impl Into<FragmentKey>,
        fragment: &C,
    ) -> Result<(), MarshalerError>
    where
        C: DynFragment + TypeRegistryEntry,
    {
        self.write_fragment_with_context(fragment_key, fragment, &MarshalContext::default())
    }

    /// Write a registered fragment body with an explicit marshal context.
    ///
    /// # Errors
    ///
    /// Returns an error when the record fragment count or bundle byte cap would be exceeded.
    pub fn write_fragment_with_context<C>(
        &mut self,
        fragment_key: impl Into<FragmentKey>,
        fragment: &C,
        marshal_context: &MarshalContext<'_>,
    ) -> Result<(), MarshalerError>
    where
        C: DynFragment + TypeRegistryEntry,
    {
        let fragment_key = fragment_key.into();
        self.reserve_fragment_slot()?;
        let fragment_start = self.wb.mark();
        VlqU32Marshaler.marshal(self.wb, fragment_key.get());
        FragmentTypeInfo::registered::<C>().write_to(self.wb);
        let wrote_payload = fragment.marshal_contents_with(marshal_context, self.wb);
        if !wrote_payload {
            self.wb.truncate_to(fragment_start);
            return Ok(());
        }
        if self.wb.len() > MAX_REPLICATED_STATE_BUNDLE_BUFFER_SIZE {
            let len = self.wb.len();
            self.wb.truncate_to(fragment_start);
            return Err(MarshalerError::ContainerOverflow {
                len,
                capacity: MAX_REPLICATED_STATE_BUNDLE_BUFFER_SIZE,
            });
        }
        self.count = self.count.saturating_add(1);
        Ok(())
    }

    /// Write a raw fragment body with caller-supplied type info.
    ///
    /// # Errors
    ///
    /// Returns an error when the record fragment count or bundle byte cap would be exceeded.
    pub fn write_raw_fragment(
        &mut self,
        fragment_key: impl Into<FragmentKey>,
        type_info: FragmentTypeInfo,
        body: &[u8],
    ) -> Result<(), MarshalerError> {
        let fragment_key = fragment_key.into();
        self.reserve_fragment_slot()?;
        let fragment_start = self.wb.mark();
        VlqU32Marshaler.marshal(self.wb, fragment_key.get());
        type_info.write_to(self.wb);
        self.wb.write_bytes(body);
        if self.wb.len() > MAX_REPLICATED_STATE_BUNDLE_BUFFER_SIZE {
            let len = self.wb.len();
            self.wb.truncate_to(fragment_start);
            return Err(MarshalerError::ContainerOverflow {
                len,
                capacity: MAX_REPLICATED_STATE_BUNDLE_BUFFER_SIZE,
            });
        }
        self.count = self.count.saturating_add(1);
        Ok(())
    }

    fn reserve_fragment_slot(&self) -> Result<(), MarshalerError> {
        if self.count == u8::MAX {
            return Err(MarshalerError::ContainerOverflow {
                len: usize::from(self.count) + 1,
                capacity: usize::from(u8::MAX),
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::{DynFragment, Fragment, FragmentBase};

    #[derive(
        nw_network_derive::Marshaler,
        Debug,
        Default,
        nw_network_derive::AzRtti,
        nw_network_derive::TypeRegistry,
    )]
    #[az_rtti("11111111-1111-4111-8111-111111111111")]
    #[type_registry(65_000)]
    struct EmptyFragment {
        #[marshal(skip)]
        base: FragmentBase,
    }

    impl DynFragment for EmptyFragment {
        fn base(&self) -> &FragmentBase {
            &self.base
        }

        fn base_mut(&mut self) -> &mut FragmentBase {
            &mut self.base
        }

        fn marshal_contents(&self, _wb: &mut WriteBuffer) -> bool {
            false
        }

        fn unmarshal_contents(&mut self, _rb: &mut ReadBuffer) -> Result<bool, MarshalerError> {
            Ok(false)
        }
    }

    impl Fragment for EmptyFragment {}

    #[derive(
        Debug,
        Default,
        nw_network_derive::AzRtti,
        nw_network_derive::TypeRegistry,
        nw_network_derive::Fragment,
    )]
    #[az_rtti("33333333-3333-4333-8333-333333333333")]
    #[type_registry(64_990)]
    struct FullBodyFragment {
        base: FragmentBase,
        contents: u8,
        attributes: u16,
        metadata: u32,
    }

    impl DynFragment for FullBodyFragment {
        fn base(&self) -> &FragmentBase {
            &self.base
        }

        fn base_mut(&mut self) -> &mut FragmentBase {
            &mut self.base
        }

        fn marshal_contents(&self, wb: &mut WriteBuffer) -> bool {
            self.contents.marshal(wb);
            true
        }

        fn unmarshal_contents(&mut self, rb: &mut ReadBuffer) -> Result<bool, MarshalerError> {
            self.contents = u8::unmarshal(rb)?;
            Ok(true)
        }

        fn marshal_attributes(&self, _mc: &MarshalContext<'_>, wb: &mut WriteBuffer) -> bool {
            self.attributes.marshal(wb);
            true
        }

        fn unmarshal_attributes(&mut self, rb: &mut ReadBuffer) -> Result<bool, MarshalerError> {
            self.attributes = u16::unmarshal(rb)?;
            Ok(true)
        }

        fn marshal_field_metadata(&self, _mc: &MarshalContext<'_>, wb: &mut WriteBuffer) -> bool {
            self.metadata.marshal(wb);
            true
        }

        fn unmarshal_field_metadata(
            &mut self,
            rb: &mut ReadBuffer,
        ) -> Result<bool, MarshalerError> {
            self.metadata = u32::unmarshal(rb)?;
            Ok(true)
        }
    }

    impl Fragment for FullBodyFragment {}

    #[test]
    fn bundle_buffer_is_vlq_len_then_raw_bytes() {
        let msg = ReplicatedStateBundle {
            bundle_buffer: vec![0xaa, 0xbb, 0xcc],
            ..Default::default()
        };

        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        msg.marshal(&mut wb);

        assert_eq!(
            wb.as_slice(),
            &[
                0, // invalid seq
                0, 0, 0, 0, 3, 0xaa, 0xbb, 0xcc,
            ]
        );
    }

    #[test]
    fn view_borrows_bundle_buffer() {
        let msg = ReplicatedStateBundle {
            bundle_buffer: vec![0xaa, 0xbb],
            ..Default::default()
        };
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        msg.marshal(&mut wb);
        let bytes = wb.into_vec();

        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bytes);
        let view = ReplicatedStateBundleView::read_from(&mut rb).unwrap();

        assert_eq!(view.bundle_buffer, &[0xaa, 0xbb]);
        assert_eq!(view.total_bundle_size(), bytes.len());
        assert_eq!(rb.left(), 0);
    }

    #[test]
    fn replication_control_wire_carries_ordered_id_list() {
        let data = ReplicationControlData::from_ids(&[10u16, 11], &[20u16, 21, 22]).unwrap();
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        data.marshal(&mut wb);

        assert_eq!(wb.as_slice(), &[5, 2, 0, 10, 0, 11, 0, 20, 0, 21, 0, 22]);

        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, wb.as_slice());
        let decoded = ReplicationControlData::unmarshal(&mut rb).unwrap();

        assert_eq!(
            decoded.stop_replication_ids(),
            &[InterestId::new(10), InterestId::new(11)]
        );
        assert_eq!(
            decoded.pause_replication_ids(),
            &[
                InterestId::new(20),
                InterestId::new(21),
                InterestId::new(22)
            ]
        );
        assert_eq!(decoded.len(), 5);
    }

    #[test]
    fn replication_control_removes_pending_stop_ids_only() {
        let mut data = ReplicationControlData::from_ids(&[10u16, 11, 12], &[20u16, 21]).unwrap();

        assert!(data.remove_pending_stop_id(11));
        assert_eq!(
            data.stop_replication_ids(),
            &[InterestId::new(10), InterestId::new(12)]
        );
        assert_eq!(
            data.pause_replication_ids(),
            &[InterestId::new(20), InterestId::new(21)]
        );
        assert_eq!(data.pause_start_idx(), 2);

        assert!(!data.remove_pending_stop_id(21));
        assert_eq!(
            data.stop_replication_ids(),
            &[InterestId::new(10), InterestId::new(12)]
        );
        assert_eq!(
            data.pause_replication_ids(),
            &[InterestId::new(20), InterestId::new(21)]
        );

        data.reset();
        assert!(data.is_empty());
        assert_eq!(data.pause_start_idx(), 0);
    }

    #[test]
    fn replication_performance_data_roundtrips_wire_field_order() {
        let msg = ReplicationPerformanceData {
            collection_period_ms: 250,
            count_bundles: 1,
            count_throttled: 2,
            count_overflows: 3,
            count_characters_intended: 4,
            count_characters_actual: 5,
            characters_missed_due_to_rate_limiter: 6,
            objects_missed_due_to_rate_limiter: 7,
            characters_missed_due_to_overflow: 8,
            objects_missed_due_to_overflow: 9,
            characters_added_during_period: 10,
            objects_added_during_period: 11,
        };

        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        msg.marshal(&mut wb);
        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, wb.as_slice());
        let decoded = ReplicationPerformanceData::unmarshal(&mut rb).unwrap();

        assert_eq!(decoded, msg);
        assert_eq!(rb.left(), 0);
        assert_eq!(ReplicationPerformanceData::TYPE_INDEX, 18);
    }

    #[test]
    fn replication_performance_data_uses_wrapping_counter_semantics() {
        let mut lhs = ReplicationPerformanceData {
            collection_period_ms: 100,
            count_bundles: u32::MAX,
            count_throttled: 1,
            ..Default::default()
        };
        let rhs = ReplicationPerformanceData {
            collection_period_ms: 250,
            count_bundles: 2,
            count_throttled: 3,
            ..Default::default()
        };

        lhs += &rhs;
        assert_eq!(lhs.collection_period_ms, 250);
        assert_eq!(lhs.count_bundles, 1);
        assert_eq!(lhs.count_throttled, 4);

        lhs.reset();
        assert_eq!(lhs.collection_period_ms, 250);
        assert_eq!(lhs.count_bundles, 0);
        assert_eq!(lhs.count_throttled, 0);
    }

    #[test]
    fn replicated_state_bundle_accessors_manage_flags_and_control_data() {
        let mut bundle = ReplicatedStateBundle::with_seq(7, 3, 1);

        assert_eq!(bundle.seq(), SequenceNumber::Seq(7));
        assert_eq!(bundle.client_context_instance_id(), 3);
        assert_eq!(bundle.bandwidth_mode(), 1);
        assert!(!bundle.is_unreliable());
        assert!(!bundle.includes_replication_control());

        bundle.set_unreliable();
        bundle.add_stop_id(10).unwrap();
        bundle.add_pause_id(20).unwrap();

        assert!(bundle.is_unreliable());
        assert!(bundle.includes_replication_control());
        assert_eq!(bundle.stop_replication_ids(), &[InterestId::new(10)]);
        assert_eq!(bundle.pause_replication_ids(), &[InterestId::new(20)]);
        assert_eq!(bundle.replication_control_count(), 2);

        assert!(bundle.remove_pending_stop_id(10));
        assert_eq!(bundle.stop_replication_ids(), &[] as &[InterestId]);
        assert_eq!(bundle.pause_replication_ids(), &[InterestId::new(20)]);

        bundle.reset_replication_control();
        assert!(bundle.includes_replication_control());
        assert_eq!(bundle.replication_control_count(), 0);
    }

    #[test]
    fn write_record_roundtrips_raw_fragment_headers() {
        let uuid = Uuid::parse_str("11223344-5566-7788-99aa-bbccddeeff00").unwrap();
        let mut bundle = ReplicatedStateBundle::default();
        bundle
            .write_record(2, |record| {
                record.write_raw_fragment(3, FragmentTypeInfo::RawUuid(uuid), &[0xcc])
            })
            .unwrap();

        let mut seen = Vec::new();
        bundle
            .visit_fragments(|record, fragment, rb| {
                seen.push((
                    record.interest_id,
                    fragment.fragment_key,
                    fragment.type_info,
                ));
                assert_eq!(rb.read_u8()?, 0xcc);
                Ok(())
            })
            .unwrap();

        assert_eq!(
            seen,
            vec![(
                InterestId::new(2),
                FragmentKey::new(3),
                FragmentTypeInfo::RawUuid(uuid)
            )]
        );
    }

    #[test]
    fn write_record_rolls_back_fragments_without_payload() {
        let mut bundle = ReplicatedStateBundle::default();
        bundle
            .write_record(7, |record| {
                record.write_fragment(12, &EmptyFragment::default())
            })
            .unwrap();

        assert!(bundle.bundle_buffer.is_empty());
    }

    #[test]
    fn baselineable_fragment_uses_full_registration_decoder() {
        let fragment = FullBodyFragment {
            contents: 0x11,
            attributes: 0x2233,
            metadata: 0x4455_6677,
            ..Default::default()
        };
        let body = BaselineableFragment::encode(&fragment, &MarshalContext::default());

        assert_eq!(
            body.type_info,
            FragmentTypeInfo::TypeIndex(FullBodyFragment::TYPE_INDEX)
        );
        assert_eq!(body.body, &[0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77]);

        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        body.marshal(&mut wb);
        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, wb.as_slice());

        let body_ref = BaselineableFragmentRef::read_from(&mut rb).unwrap();

        assert_eq!(body_ref.body, body.body);
        assert_eq!(rb.left(), 0);

        let decoded = body_ref.decode().unwrap();
        let decoded = decoded.downcast_ref::<FullBodyFragment>().unwrap();

        assert_eq!(decoded.contents, 0x11);
        assert_eq!(decoded.attributes, 0x2233);
        assert_eq!(decoded.metadata, 0x4455_6677);
    }

    #[test]
    fn failed_record_write_rolls_back() {
        let mut bundle = ReplicatedStateBundle::with_bundle_buffer(vec![0xaa]);
        let err = bundle
            .write_record(2, |record| -> Result<(), MarshalerError> {
                record.write_raw_fragment(3, FragmentTypeInfo::TypeIndex(4), &[0xcc])?;
                Err(MarshalerError::InvalidDiscriminant { value: 99 })
            })
            .unwrap_err();

        assert!(matches!(
            err,
            MarshalerError::InvalidDiscriminant { value: 99 }
        ));
        assert_eq!(bundle.bundle_buffer, vec![0xaa]);
    }
}
