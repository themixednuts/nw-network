use uuid::Uuid;

use super::{
    ClientActorHash, GroupIndex, SequenceNumber,
    fragment::{FragmentBase, FragmentCategory},
};
use crate::serialize::container_marshal::ContainerMarshaler;
use crate::serialize::replicated_field::{ReplicatedFieldHandler, ReplicatedFieldHandlerBase};
use crate::serialize::vlq::VlqU64Marshaler;
use crate::serialize::{Marshaler, MarshalerError, ReadBuffer, WriteBuffer};

pub const REPLICATED_STATE_TYPE_ID: Uuid = Uuid::from_u128(0x261f7815_2be0_44f5_be4a_e8070677a57b);

#[allow(non_snake_case)]
pub mod ReplicatedStateConstants {
    pub const MAX_FIELDS_PER_REPLICATED_STATE: u8 = 64;
    pub const MAX_FILTERGROUPS_PER_REPLICATED_STATE: u8 = 10;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FilterValue {
    pub value: ClientActorHash,
    pub ref_count: u64,
}

impl FilterValue {
    #[must_use]
    pub const fn new(value: ClientActorHash, ref_count: u64) -> Self {
        Self { value, ref_count }
    }
}

impl Marshaler for FilterValue {
    fn marshal(&self, wb: &mut WriteBuffer) {
        self.value.marshal(wb);
        self.ref_count.marshal(wb);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        Ok(Self {
            value: ClientActorHash::unmarshal(rb)?,
            ref_count: u64::unmarshal(rb)?,
        })
    }
}

pub struct ReplicatedFieldInfo<'a> {
    pub name: &'a str,
    pub handler: &'a dyn ReplicatedFieldHandlerBase,
    pub is_filter_group: bool,
}

pub struct ReplicatedFieldInfoMut<'a> {
    pub name: &'a str,
    pub handler: &'a mut dyn ReplicatedFieldHandlerBase,
    pub is_filter_group: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ReplicatedDefaultBits(u64);

impl ReplicatedDefaultBits {
    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    #[must_use]
    pub const fn bits(self) -> u64 {
        self.0
    }

    #[must_use]
    pub fn get(self, index: usize) -> bool {
        index < ReplicatedStateConstants::MAX_FIELDS_PER_REPLICATED_STATE as usize
            && (self.0 & (1u64 << index)) != 0
    }

    pub fn set(&mut self, index: usize, value: bool) {
        if index >= ReplicatedStateConstants::MAX_FIELDS_PER_REPLICATED_STATE as usize {
            return;
        }
        if value {
            self.0 |= 1u64 << index;
        } else {
            self.0 &= !(1u64 << index);
        }
    }
}

impl Marshaler for ReplicatedDefaultBits {
    fn marshal(&self, wb: &mut WriteBuffer) {
        self.0.marshal(wb);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        Ok(Self(u64::unmarshal(rb)?))
    }
}

pub type ClientFilterContainer = Vec<FilterValue>;
pub type ClientFilterContainerMarshalShim = ContainerMarshaler<FilterValue>;
pub type ClientFilterField =
    ReplicatedFieldHandler<ClientFilterContainer, ClientFilterContainerMarshalShim>;
pub type DefaultBitsField = ReplicatedFieldHandler<ReplicatedDefaultBits>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReplicatedMergeOutcome {
    pub last_modified: SequenceNumber,
    pub has_new_network_data: bool,
    pub detected_new_data_in_last_merge: bool,
}

impl Default for ReplicatedMergeOutcome {
    fn default() -> Self {
        Self {
            last_modified: SequenceNumber::Invalid,
            has_new_network_data: false,
            detected_new_data_in_last_merge: false,
        }
    }
}

/// concrete field handlers.
#[derive(Debug, Clone)]
pub struct ReplicatedFilterGroup {
    explicit_client_targets: ClientFilterField,
    default_bits: DefaultBitsField,
}

impl Default for ReplicatedFilterGroup {
    fn default() -> Self {
        Self {
            explicit_client_targets: ReplicatedFieldHandler::some(Vec::new()),
            default_bits: ReplicatedFieldHandler::some(ReplicatedDefaultBits::empty()),
        }
    }
}

impl ReplicatedFilterGroup {
    #[must_use]
    pub fn explicit_client_targets(&self) -> &[FilterValue] {
        self.explicit_client_targets
            .value()
            .map_or(&[], Vec::as_slice)
    }

    #[must_use]
    pub fn default_bits(&self) -> ReplicatedDefaultBits {
        self.default_bits
            .value()
            .copied()
            .unwrap_or_else(ReplicatedDefaultBits::empty)
    }

    pub fn set_default_bits(&mut self, bits: ReplicatedDefaultBits) {
        self.default_bits.set_value(bits);
    }

    #[must_use]
    pub fn explicit_client_targets_last_modified(&self) -> SequenceNumber {
        self.explicit_client_targets.last_modified()
    }

    #[must_use]
    pub fn default_bits_last_modified(&self) -> SequenceNumber {
        self.default_bits.last_modified()
    }
}

///
/// Variable-length sibling of [`super::FixedReplicatedState`]. Concrete Rust
/// through the `ReplicatedState` derive.
#[derive(Debug, Clone)]
pub struct ReplicatedState {
    base: FragmentBase,
    pub category: FragmentCategory,
    filter_groups: Vec<ReplicatedFilterGroup>,
    attr_count: usize,
    has_new_network_data: bool,
    detected_new_data_in_last_merge: bool,
    sequence: SequenceNumber,
    last_modified: SequenceNumber,
    is_fully_merged_state: bool,
}

impl Default for ReplicatedState {
    fn default() -> Self {
        Self::new()
    }
}

impl ReplicatedState {
    #[must_use]
    pub fn new() -> Self {
        Self::with_reserved_groups(1)
    }

    ///
    /// whose field/group APIs are fully overridden.
    #[must_use]
    pub fn with_reserved_groups(reserve_groups: usize) -> Self {
        let mut filter_groups = Vec::with_capacity(reserve_groups);
        if reserve_groups != 0 {
            filter_groups.push(ReplicatedFilterGroup::default());
        }
        Self {
            base: FragmentBase::default(),
            category: FragmentCategory::Uncategorized,
            filter_groups,
            attr_count: 0,
            has_new_network_data: false,
            detected_new_data_in_last_merge: false,
            sequence: SequenceNumber::Invalid,
            last_modified: SequenceNumber::Invalid,
            is_fully_merged_state: false,
        }
    }

    #[must_use]
    pub const fn base(&self) -> &FragmentBase {
        &self.base
    }

    #[must_use]
    pub fn base_mut(&mut self) -> &mut FragmentBase {
        &mut self.base
    }

    #[must_use]
    pub const fn sequence(&self) -> SequenceNumber {
        self.sequence
    }

    #[must_use]
    pub const fn last_modified(&self) -> SequenceNumber {
        self.last_modified
    }

    #[must_use]
    pub const fn is_fully_merged_state(&self) -> bool {
        self.is_fully_merged_state
    }

    #[must_use]
    pub const fn has_new_network_data(&self) -> bool {
        self.has_new_network_data
    }

    #[must_use]
    pub const fn detected_new_data_in_last_merge(&self) -> bool {
        self.detected_new_data_in_last_merge
    }

    #[must_use]
    pub fn filter_groups(&self) -> &[ReplicatedFilterGroup] {
        &self.filter_groups
    }

    #[must_use]
    pub fn filter_groups_mut(&mut self) -> &mut [ReplicatedFilterGroup] {
        &mut self.filter_groups
    }

    /// Ensure generated state groups exist in header bookkeeping.
    ///
    /// Derived Rust states know their descriptor groups at compile time, so
    /// missing groups are treated as empty-whitelist groups until callers add
    /// explicit targets.
    pub fn ensure_filter_groups(&mut self, group_count: usize) {
        let max_groups = ReplicatedStateConstants::MAX_FILTERGROUPS_PER_REPLICATED_STATE as usize;
        let target = group_count.min(max_groups);
        while self.filter_groups.len() < target {
            self.filter_groups.push(ReplicatedFilterGroup::default());
        }
    }

    #[must_use]
    pub fn num_filter_groups(&self) -> usize {
        self.filter_groups.len()
    }

    pub fn add_filter_group(&mut self) -> GroupIndex {
        let index = self.filter_groups.len();
        if index < ReplicatedStateConstants::MAX_FILTERGROUPS_PER_REPLICATED_STATE as usize {
            self.filter_groups.push(ReplicatedFilterGroup::default());
        }
        GroupIndex::new(index)
    }

    #[must_use]
    pub const fn attr_count(&self) -> usize {
        self.attr_count
    }

    pub fn register_attribute(&mut self) {
        self.attr_count += 1;
    }

    pub fn finish_merge(
        &mut self,
        seq: SequenceNumber,
        correlation_id: Uuid,
        outcome: ReplicatedMergeOutcome,
    ) {
        self.sequence = seq;
        self.last_modified = outcome.last_modified;
        self.is_fully_merged_state = true;
        self.has_new_network_data = outcome.has_new_network_data;
        self.detected_new_data_in_last_merge = outcome.detected_new_data_in_last_merge;
        self.base.set_correlation_id(correlation_id);
    }

    pub fn reset_has_new_network_data(&mut self) {
        self.has_new_network_data = false;
    }

    pub fn reset_filter_group_attribute_network_data(&mut self) {
        for group in &mut self.filter_groups {
            group.explicit_client_targets.reset_has_new_network_data();
            group.default_bits.reset_has_new_network_data();
        }
    }

    pub fn set_has_new_network_data_on_initial_state(&mut self) {
        self.has_new_network_data = true;
        self.detected_new_data_in_last_merge = true;
    }

    pub fn merge_filter_group_attributes(
        &mut self,
        old_state: &Self,
        new_state: &mut Self,
        seq: SequenceNumber,
        inherit_previous_network_data_status: bool,
        outcome: &mut ReplicatedMergeOutcome,
    ) {
        self.ensure_filter_groups(
            old_state
                .num_filter_groups()
                .max(new_state.num_filter_groups()),
        );

        let old_default = ReplicatedFilterGroup::default();
        for group_idx in 0..self.num_filter_groups() {
            let mut new_default = ReplicatedFilterGroup::default();
            let old_group = old_state
                .filter_groups
                .get(group_idx)
                .unwrap_or(&old_default);
            let new_group = new_state
                .filter_groups
                .get_mut(group_idx)
                .unwrap_or(&mut new_default);
            let merged_group = &mut self.filter_groups[group_idx];

            outcome.detected_new_data_in_last_merge |= merged_group
                .explicit_client_targets
                .merge_and_update_sequence(
                    &old_group.explicit_client_targets,
                    &new_group.explicit_client_targets,
                    seq,
                    inherit_previous_network_data_status,
                );
            outcome.last_modified = outcome
                .last_modified
                .max(merged_group.explicit_client_targets.last_modified());
            outcome.has_new_network_data |=
                merged_group.explicit_client_targets.has_new_network_data();

            outcome.detected_new_data_in_last_merge |=
                merged_group.default_bits.merge_and_update_sequence(
                    &old_group.default_bits,
                    &new_group.default_bits,
                    seq,
                    inherit_previous_network_data_status,
                );
            outcome.last_modified = outcome
                .last_modified
                .max(merged_group.default_bits.last_modified());
            outcome.has_new_network_data |= merged_group.default_bits.has_new_network_data();
        }
    }

    #[must_use]
    pub fn should_send_to_client(&self, client_id: ClientActorHash, group_idx: GroupIndex) -> bool {
        let group_idx = group_idx.get();
        if group_idx >= ReplicatedStateConstants::MAX_FILTERGROUPS_PER_REPLICATED_STATE as usize {
            return false;
        }
        let Some(group) = self.filter_groups.get(group_idx) else {
            return true;
        };
        group.explicit_client_targets().is_empty()
            || group
                .explicit_client_targets()
                .iter()
                .any(|entry| entry.value == client_id)
    }

    pub fn add_client_to_replication_whitelist(
        &mut self,
        client_id: ClientActorHash,
        group_idx: GroupIndex,
    ) {
        let group_idx = group_idx.get();
        self.ensure_filter_groups(group_idx.saturating_add(1));
        let Some(group) = self.filter_groups.get_mut(group_idx) else {
            return;
        };
        group.explicit_client_targets.access(|targets| {
            if let Some(entry) = targets.iter_mut().find(|entry| entry.value == client_id) {
                entry.ref_count = entry.ref_count.saturating_add(1);
            } else {
                targets.push(FilterValue::new(client_id, 1));
            }
            true
        });
    }

    pub fn remove_client_from_replication_whitelist(
        &mut self,
        client_id: ClientActorHash,
        group_idx: GroupIndex,
    ) {
        let group_idx = group_idx.get();
        let Some(group) = self.filter_groups.get_mut(group_idx) else {
            return;
        };
        group.explicit_client_targets.access(|targets| {
            let Some(position) = targets.iter().position(|entry| entry.value == client_id) else {
                return false;
            };
            if targets[position].ref_count > 1 {
                targets[position].ref_count -= 1;
            } else {
                targets.remove(position);
            }
            true
        });
    }

    pub fn clear_replication_whitelist(&mut self, group_idx: GroupIndex) {
        let group_idx = group_idx.get();
        if let Some(group) = self.filter_groups.get_mut(group_idx) {
            group.explicit_client_targets.access(|targets| {
                let had_entries = !targets.is_empty();
                targets.clear();
                had_entries
            });
        }
    }

    pub fn calculate_default_bits(
        &mut self,
        group_idx: GroupIndex,
        fields: &[ReplicatedFieldInfo<'_>],
        _baseline: SequenceNumber,
    ) -> ReplicatedDefaultBits {
        let group_idx = group_idx.get();
        let mut bits = ReplicatedDefaultBits::empty();
        for (index, field) in fields.iter().enumerate() {
            bits.set(index, field.handler.is_default_value());
        }
        if let Some(group) = self.filter_groups.get_mut(group_idx) {
            group.default_bits.set_value(bits);
        }
        bits
    }

    pub fn apply_default_bits(
        &self,
        group_idx: GroupIndex,
        fields: &mut [ReplicatedFieldInfoMut<'_>],
    ) {
        let group_idx = group_idx.get();
        let Some(group) = self.filter_groups.get(group_idx) else {
            return;
        };
        for (index, field) in fields.iter_mut().enumerate() {
            if group.default_bits().get(index) {
                field.handler.set_current_value_as_default();
            }
        }
    }

    ///
    /// User attributes are emitted by generated states after these fields once
    /// only need the filter whitelist/default-bit attributes.
    pub fn marshal_filter_group_attributes(
        &self,
        baseline: SequenceNumber,
        wb: &mut WriteBuffer,
    ) -> bool {
        let attr_count = self.filter_groups.len().saturating_mul(2);
        debug_assert!(
            attr_count <= ReplicatedStateConstants::MAX_FIELDS_PER_REPLICATED_STATE as usize,
            "ReplicatedState attributes are encoded in one u64 field mask"
        );

        let mut field_mask = 0u64;
        for (group_idx, group) in self.filter_groups.iter().enumerate() {
            let explicit_idx = group_idx * 2;
            let default_bits_idx = explicit_idx + 1;
            if group.explicit_client_targets.is_dirty_since(baseline) {
                field_mask |= 1u64 << explicit_idx;
            }
            if group.default_bits.is_dirty_since(baseline) {
                field_mask |= 1u64 << default_bits_idx;
            }
        }

        if field_mask == 0 {
            wb.write_u8(0);
            return true;
        }

        VlqU64Marshaler.marshal(wb, field_mask);
        for (group_idx, group) in self.filter_groups.iter().enumerate() {
            let explicit_idx = group_idx * 2;
            let default_bits_idx = explicit_idx + 1;
            if (field_mask & (1u64 << explicit_idx)) != 0 {
                group
                    .explicit_client_targets
                    .marshal_field_since(wb, baseline);
            }
            if (field_mask & (1u64 << default_bits_idx)) != 0 {
                group.default_bits.marshal_field_since(wb, baseline);
            }
        }
        true
    }

    /// Decode dirty filter-group attributes.
    ///
    /// # Errors
    ///
    /// Returns the first error reported by the mask or attribute field decoders.
    pub fn unmarshal_filter_group_attributes(
        &mut self,
        rb: &mut ReadBuffer,
    ) -> Result<bool, MarshalerError> {
        let field_mask = VlqU64Marshaler.unmarshal(rb)?;
        if field_mask == 0 {
            return Ok(false);
        }

        self.ensure_filter_groups(self.filter_groups.len().max(1));
        for group_idx in 0..self.filter_groups.len() {
            let explicit_idx = group_idx * 2;
            let default_bits_idx = explicit_idx + 1;
            if (field_mask & (1u64 << explicit_idx)) != 0 {
                self.filter_groups[group_idx]
                    .explicit_client_targets
                    .unmarshal_field(rb)?;
            }
            if (field_mask & (1u64 << default_bits_idx)) != 0 {
                self.filter_groups[group_idx]
                    .default_bits
                    .unmarshal_field(rb)?;
            }
        }
        Ok(true)
    }

    pub fn marshal_filter_group_attribute_metadata(&self, wb: &mut WriteBuffer) -> bool {
        for group in &self.filter_groups {
            group.explicit_client_targets.last_modified().marshal(wb);
            group.default_bits.last_modified().marshal(wb);
        }
        true
    }

    /// Decode filter-group attribute sequence metadata.
    ///
    /// # Errors
    ///
    /// Returns the first error reported while reading sequence numbers.
    pub fn unmarshal_filter_group_attribute_metadata(
        &mut self,
        rb: &mut ReadBuffer,
    ) -> Result<bool, MarshalerError> {
        for group in &mut self.filter_groups {
            let explicit_seq = SequenceNumber::unmarshal(rb)?;
            group
                .explicit_client_targets
                .set_last_modified(explicit_seq);
            let default_bits_seq = SequenceNumber::unmarshal(rb)?;
            group.default_bits.set_last_modified(default_bits_seq);
        }
        Ok(true)
    }

    #[must_use]
    pub fn sanity_check_field_count(&self, field_group: GroupIndex, field_count: usize) -> bool {
        let field_group = field_group.get();
        field_group < self.filter_groups.len()
            && field_count <= ReplicatedStateConstants::MAX_FIELDS_PER_REPLICATED_STATE as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_generated_groups_behave_like_empty_whitelists() {
        let mut header = ReplicatedState::new();
        let group = GroupIndex::new(2);

        assert_eq!(header.num_filter_groups(), 1);
        assert!(header.should_send_to_client(ClientActorHash::new(7), group));

        header.add_client_to_replication_whitelist(ClientActorHash::new(7), group);

        assert_eq!(header.num_filter_groups(), 3);
        assert!(header.should_send_to_client(ClientActorHash::new(7), group));
        assert!(!header.should_send_to_client(ClientActorHash::new(8), group));
    }
}
