use std::marker::PhantomData;

use super::{
    BandwidthMode, ClientContextId, InterestId, ReplicatedStateBundle, ReplicationControlData,
    SequenceNumber,
};
use crate::serialize::MarshalerError;

pub mod state {
    #[derive(Debug, Clone, Copy)]
    pub struct Set;

    #[derive(Debug, Clone, Copy)]
    pub struct Unset;
}

#[derive(Debug, Clone)]
pub struct StateBundleBuilder<
    Reliability = state::Unset,
    Bandwidth = state::Unset,
    Payload = state::Unset,
> {
    seq: SequenceNumber,
    client_context_id: ClientContextId,
    bandwidth: BandwidthMode,
    is_unreliable: bool,
    replication_control: Option<ReplicationControlData>,
    bundle_buffer: Vec<u8>,
    _state: PhantomData<(Reliability, Bandwidth, Payload)>,
}

impl StateBundleBuilder {
    #[must_use]
    pub fn new(
        seq: impl Into<SequenceNumber>,
        client_context_id: impl Into<ClientContextId>,
    ) -> Self {
        Self {
            seq: seq.into(),
            client_context_id: client_context_id.into(),
            bandwidth: BandwidthMode::default(),
            is_unreliable: false,
            replication_control: None,
            bundle_buffer: Vec::new(),
            _state: PhantomData,
        }
    }
}

impl<Reliability, Bandwidth, Payload> StateBundleBuilder<Reliability, Bandwidth, Payload> {
    /// Add an id to the stop-replication partition.
    ///
    /// # Errors
    ///
    /// Returns [`MarshalerError::ContainerOverflow`] when the replication
    /// control section already contains the maximum number of ids.
    pub fn stop_replication(
        mut self,
        interest_id: impl Into<InterestId>,
    ) -> Result<Self, MarshalerError> {
        self.replication_control()
            .push_stop_replication_id(interest_id)?;
        Ok(self)
    }

    /// Add an id to the pause-replication partition.
    ///
    /// # Errors
    ///
    /// Returns [`MarshalerError::ContainerOverflow`] when the replication
    /// control section already contains the maximum number of ids.
    pub fn pause_replication(
        mut self,
        interest_id: impl Into<InterestId>,
    ) -> Result<Self, MarshalerError> {
        self.replication_control()
            .push_pause_replication_id(interest_id)?;
        Ok(self)
    }

    /// Build a bundle from the fields set so far.
    ///
    /// Empty payloads are valid; the typestate parameters prevent setting the
    /// same axis twice, but they do not require every optional axis.
    #[must_use]
    pub fn build(self) -> ReplicatedStateBundle {
        ReplicatedStateBundle {
            seq: self.seq,
            client_context_instance_id: self.client_context_id.get(),
            bandwidth_mode: self.bandwidth.get(),
            is_unreliable: self.is_unreliable,
            replication_control: self.replication_control,
            bundle_buffer: self.bundle_buffer,
        }
    }

    fn replication_control(&mut self) -> &mut ReplicationControlData {
        self.replication_control
            .get_or_insert_with(ReplicationControlData::default)
    }

    fn cast<NextReliability, NextBandwidth, NextPayload>(
        self,
    ) -> StateBundleBuilder<NextReliability, NextBandwidth, NextPayload> {
        StateBundleBuilder {
            seq: self.seq,
            client_context_id: self.client_context_id,
            bandwidth: self.bandwidth,
            is_unreliable: self.is_unreliable,
            replication_control: self.replication_control,
            bundle_buffer: self.bundle_buffer,
            _state: PhantomData,
        }
    }
}

impl<Bandwidth, Payload> StateBundleBuilder<state::Unset, Bandwidth, Payload> {
    #[must_use]
    pub fn unreliable(self) -> StateBundleBuilder<state::Set, Bandwidth, Payload> {
        let mut next = self.cast::<state::Set, Bandwidth, Payload>();
        next.is_unreliable = true;
        next
    }
}

impl<Reliability, Payload> StateBundleBuilder<Reliability, state::Unset, Payload> {
    #[must_use]
    pub fn bandwidth(
        self,
        bandwidth: impl Into<BandwidthMode>,
    ) -> StateBundleBuilder<Reliability, state::Set, Payload> {
        let mut next = self.cast::<Reliability, state::Set, Payload>();
        next.bandwidth = bandwidth.into();
        next
    }
}

impl<Reliability, Bandwidth> StateBundleBuilder<Reliability, Bandwidth, state::Unset> {
    #[must_use]
    pub fn payload(
        self,
        bundle_buffer: impl Into<Vec<u8>>,
    ) -> StateBundleBuilder<Reliability, Bandwidth, state::Set> {
        let mut next = self.cast::<Reliability, Bandwidth, state::Set>();
        next.bundle_buffer = bundle_buffer.into();
        next
    }

    #[must_use]
    pub fn bundle_buffer(
        self,
        bundle_buffer: impl Into<Vec<u8>>,
    ) -> StateBundleBuilder<Reliability, Bandwidth, state::Set> {
        self.payload(bundle_buffer)
    }
}

impl ReplicatedStateBundle {
    #[must_use]
    pub fn builder(
        seq: impl Into<SequenceNumber>,
        client_context_id: impl Into<ClientContextId>,
    ) -> StateBundleBuilder {
        StateBundleBuilder::new(seq, client_context_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::FragmentTypeInfo;

    #[test]
    fn typestate_builder_sets_wire_header_and_control_data() {
        let bundle = ReplicatedStateBundle::builder(11, 1)
            .unreliable()
            .bandwidth(2)
            .stop_replication(111)
            .unwrap()
            .payload(vec![0xaa, 0xbb])
            .build();

        assert_eq!(bundle.seq(), SequenceNumber::Seq(11));
        assert_eq!(bundle.client_context_id(), ClientContextId::new(1));
        assert_eq!(bundle.bandwidth(), BandwidthMode::new(2));
        assert!(bundle.is_unreliable());
        assert_eq!(bundle.stop_replication_ids(), &[InterestId::new(111)]);
        assert_eq!(bundle.pause_replication_ids(), &[] as &[InterestId]);
        assert_eq!(bundle.bundle_buffer, vec![0xaa, 0xbb]);
    }

    #[test]
    fn typestate_builder_accepts_writer_payload() {
        let mut payload = ReplicatedStateBundle::default();
        payload
            .write_record(7, |record| {
                record.write_raw_fragment(3, FragmentTypeInfo::TypeIndex(4), &[0xcc])
            })
            .unwrap();

        let bundle = ReplicatedStateBundle::builder(2, 1)
            .bundle_buffer(payload.bundle_buffer.clone())
            .build();

        assert_eq!(bundle.bundle_buffer, payload.bundle_buffer);
    }
}
