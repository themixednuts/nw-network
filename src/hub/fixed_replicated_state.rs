//!
//! client whitelists, hub attributes, the fixed group-present bitset, and the
//! VLQ-u64 field mask used inside each present group.

use super::{
    ClientActorHash, GroupIndex, SequenceNumber,
    fragment::{Fragment, FragmentBase, MarshalContext},
};
use crate::serialize::marshaler::Codec;
use crate::serialize::replicated_field::{
    DeltaCompressedCounterHandler, DeltaCompressedReplicatedFieldHandler, DeltaRangeValue,
    DynamicDeltaReplicatedFieldHandler, FloatTimerDeltaReplicatedField, ReplicatedFieldHandler,
    ReplicatedFieldHandlerBase,
};
use crate::serialize::vlq::{VlqU32Marshaler, VlqU64Marshaler};
use crate::serialize::{Marshaler, MarshalerError, ReadBuffer, WriteBuffer};
use arrayvec::ArrayVec;
use glam::Vec3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedMergeOutcome {
    pub last_modified: SequenceNumber,
    pub has_new_network_data: bool,
    pub detected_new_data_in_last_merge: bool,
}

impl Default for FixedMergeOutcome {
    fn default() -> Self {
        Self {
            last_modified: SequenceNumber::Invalid,
            has_new_network_data: false,
            detected_new_data_in_last_merge: false,
        }
    }
}

pub type ClientWhitelistVector<const CLIENT_WHITELIST_SIZE: usize> =
    ArrayVec<ClientActorHash, CLIENT_WHITELIST_SIZE>;
pub type ClientWhitelistField<const CLIENT_WHITELIST_SIZE: usize> =
    ReplicatedFieldHandler<ClientWhitelistVector<CLIENT_WHITELIST_SIZE>>;
pub type GroupPresentFlagBitset<const N_GROUPS: usize> = [bool; N_GROUPS];

#[derive(Debug, Clone)]
struct FieldGroupState<const CLIENT_WHITELIST_SIZE: usize> {
    client_whitelist: ClientWhitelistField<CLIENT_WHITELIST_SIZE>,
}

impl<const CLIENT_WHITELIST_SIZE: usize> Default for FieldGroupState<CLIENT_WHITELIST_SIZE> {
    fn default() -> Self {
        Self {
            client_whitelist: ReplicatedFieldHandler::some(ArrayVec::new()),
        }
    }
}

/// client-whitelist attributes. Registered fields live in the derived state,
/// so Rust exposes them through [`FixedReplicatedStateFields`] visitors
/// instead of storing self-referential field pointers.
#[derive(Debug, Clone)]
pub struct FixedReplicatedState<
    const N_GROUPS: usize,
    const N_FIELDS_PER_GROUP: usize,
    const CLIENT_WHITELIST_SIZE: usize = 0,
    const N_USER_ATTRIBUTES: usize = 0,
> {
    base: FragmentBase,
    field_groups: ArrayVec<FieldGroupState<CLIENT_WHITELIST_SIZE>, N_GROUPS>,
    is_fully_merged_state: bool,
    has_new_network_data: bool,
    detected_new_data_in_last_merge: bool,
    sequence: SequenceNumber,
    last_modified: SequenceNumber,
}

impl<
    const N_GROUPS: usize,
    const N_FIELDS_PER_GROUP: usize,
    const CLIENT_WHITELIST_SIZE: usize,
    const N_USER_ATTRIBUTES: usize,
> Default
    for FixedReplicatedState<N_GROUPS, N_FIELDS_PER_GROUP, CLIENT_WHITELIST_SIZE, N_USER_ATTRIBUTES>
{
    fn default() -> Self {
        let mut field_groups = ArrayVec::new();
        while field_groups.len() < N_GROUPS {
            field_groups.push(FieldGroupState::default());
        }
        Self {
            base: FragmentBase::new(),
            field_groups,
            is_fully_merged_state: false,
            has_new_network_data: false,
            detected_new_data_in_last_merge: false,
            sequence: SequenceNumber::Invalid,
            last_modified: SequenceNumber::Invalid,
        }
    }
}

impl<
    const N_GROUPS: usize,
    const N_FIELDS_PER_GROUP: usize,
    const CLIENT_WHITELIST_SIZE: usize,
    const N_USER_ATTRIBUTES: usize,
> FixedReplicatedState<N_GROUPS, N_FIELDS_PER_GROUP, CLIENT_WHITELIST_SIZE, N_USER_ATTRIBUTES>
{
    pub const N_GROUPS: usize = N_GROUPS;
    pub const N_FIELDS_PER_GROUP: usize = N_FIELDS_PER_GROUP;
    pub const CLIENT_WHITELIST_SIZE: usize = CLIENT_WHITELIST_SIZE;
    pub const N_USER_ATTRIBUTES: usize = N_USER_ATTRIBUTES;

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

    pub fn finish_merge(
        &mut self,
        seq: SequenceNumber,
        correlation_id: uuid::Uuid,
        outcome: FixedMergeOutcome,
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

    pub fn reset_client_whitelist_network_data(&mut self) {
        self.ensure_groups();
        for group in &mut self.field_groups {
            group.client_whitelist.reset_has_new_network_data();
        }
    }

    pub fn set_has_new_network_data_on_initial_state(&mut self) {
        self.has_new_network_data = true;
        self.detected_new_data_in_last_merge = true;
    }

    pub fn merge_client_whitelist_attributes(
        &mut self,
        old_state: &Self,
        new_state: &mut Self,
        seq: SequenceNumber,
        inherit_previous_network_data_status: bool,
        outcome: &mut FixedMergeOutcome,
    ) {
        self.ensure_groups();

        for group_idx in 0..N_GROUPS {
            let old_default = ReplicatedFieldHandler::some(ArrayVec::new());
            let mut new_default = ReplicatedFieldHandler::some(ArrayVec::new());
            let old_field = old_state
                .field_groups
                .get(group_idx)
                .map_or(&old_default, |group| &group.client_whitelist);
            let new_field = new_state
                .field_groups
                .get_mut(group_idx)
                .map_or(&mut new_default, |group| &mut group.client_whitelist);
            let merged_field = &mut self.field_groups[group_idx].client_whitelist;

            outcome.detected_new_data_in_last_merge |= merged_field.merge_and_update_sequence(
                old_field,
                new_field,
                seq,
                inherit_previous_network_data_status,
            );
            outcome.last_modified = outcome.last_modified.max(merged_field.last_modified());
            outcome.has_new_network_data |= merged_field.has_new_network_data();
        }
    }

    #[must_use]
    pub fn client_whitelist(&self, group_idx: GroupIndex) -> Option<&[ClientActorHash]> {
        let group_idx = group_idx.get();
        self.field_groups.get(group_idx).map(|group| {
            group
                .client_whitelist
                .value()
                .map_or(&[] as &[ClientActorHash], |whitelist| whitelist.as_slice())
        })
    }

    pub fn client_whitelists(&self) -> impl Iterator<Item = &[ClientActorHash]> {
        self.field_groups.iter().map(|group| {
            group
                .client_whitelist
                .value()
                .map_or(&[] as &[ClientActorHash], |whitelist| whitelist.as_slice())
        })
    }

    #[must_use]
    pub fn client_whitelist_last_modified(&self, group_idx: GroupIndex) -> SequenceNumber {
        let group_idx = group_idx.get();
        self.field_groups
            .get(group_idx)
            .map_or(SequenceNumber::ValidNonSequence, |group| {
                group.client_whitelist.last_modified()
            })
    }

    pub fn set_client_whitelist_last_modified(
        &mut self,
        group_idx: GroupIndex,
        seq: SequenceNumber,
    ) -> bool {
        let group_idx = group_idx.get();
        if group_idx >= N_GROUPS {
            return false;
        }
        self.ensure_groups();
        self.field_groups[group_idx]
            .client_whitelist
            .set_last_modified(seq);
        true
    }

    pub fn marshal_client_whitelist_attributes(
        &self,
        baseline: SequenceNumber,
        wb: &mut WriteBuffer,
    ) -> bool {
        let mut field_mask = 0u64;
        for group_idx in 0..N_GROUPS {
            let last_modified = self.client_whitelist_last_modified(GroupIndex::new(group_idx));
            if last_modified.is_valid() && baseline < last_modified {
                field_mask |= 1u64 << group_idx;
            }
        }

        if field_mask == 0 {
            wb.write_u8(0);
            return true;
        }

        VlqU64Marshaler.marshal(wb, field_mask);
        for group_idx in 0..N_GROUPS {
            if (field_mask & (1u64 << group_idx)) != 0 {
                let whitelist = self
                    .client_whitelist(GroupIndex::new(group_idx))
                    .unwrap_or(&[]);
                marshal_client_whitelist::<CLIENT_WHITELIST_SIZE>(whitelist, wb);
            }
        }
        true
    }

    /// Decode dirty client-whitelist attributes.
    ///
    /// # Errors
    ///
    /// Returns an error when the whitelist mask or any encoded client id is malformed.
    pub fn unmarshal_client_whitelist_attributes(
        &mut self,
        rb: &mut ReadBuffer,
    ) -> Result<bool, MarshalerError> {
        self.ensure_groups();
        let field_mask = VlqU64Marshaler.unmarshal(rb)?;
        if field_mask == 0 {
            return Ok(false);
        }

        for group_idx in 0..N_GROUPS {
            if (field_mask & (1u64 << group_idx)) != 0 {
                let whitelist = unmarshal_client_whitelist::<CLIENT_WHITELIST_SIZE>(rb)?;
                let field = &mut self.field_groups[group_idx].client_whitelist;
                field.set_value(whitelist);
            }
        }
        Ok(true)
    }

    #[must_use]
    pub fn should_send_to_client(&self, client_id: ClientActorHash, group_idx: GroupIndex) -> bool {
        let group_idx = group_idx.get();
        if group_idx >= N_GROUPS {
            return false;
        }
        let Some(group) = self.field_groups.get(group_idx) else {
            return true;
        };
        let whitelist = group
            .client_whitelist
            .value()
            .map_or(&[] as &[ClientActorHash], |whitelist| whitelist.as_slice());
        whitelist.is_empty() || whitelist.contains(&client_id)
    }

    pub fn add_client_to_replication_whitelist(
        &mut self,
        client_id: ClientActorHash,
        group_idx: GroupIndex,
    ) -> bool {
        let group_idx = group_idx.get();
        if group_idx >= N_GROUPS || CLIENT_WHITELIST_SIZE == 0 {
            return false;
        }
        self.ensure_groups();
        let mut inserted = false;
        self.field_groups[group_idx]
            .client_whitelist
            .access(|whitelist| {
                if whitelist.contains(&client_id) || whitelist.len() >= CLIENT_WHITELIST_SIZE {
                    return false;
                }
                whitelist.push(client_id);
                inserted = true;
                true
            });
        inserted
    }

    pub fn remove_client_from_replication_whitelist(
        &mut self,
        client_id: ClientActorHash,
        group_idx: GroupIndex,
    ) -> bool {
        let group_idx = group_idx.get();
        if group_idx >= N_GROUPS {
            return false;
        }
        self.ensure_groups();
        let mut removed = false;
        self.field_groups[group_idx]
            .client_whitelist
            .access(|whitelist| {
                let Some(position) = whitelist.iter().position(|entry| *entry == client_id) else {
                    return false;
                };
                whitelist.remove(position);
                removed = true;
                true
            });
        removed
    }

    pub fn clear_replication_whitelist(&mut self, group_idx: GroupIndex) -> bool {
        let group_idx = group_idx.get();
        if group_idx >= N_GROUPS {
            return false;
        }
        self.ensure_groups();
        let mut had_entries = false;
        self.field_groups[group_idx]
            .client_whitelist
            .access(|whitelist| {
                had_entries = !whitelist.is_empty();
                if had_entries {
                    whitelist.clear();
                }
                had_entries
            });
        had_entries
    }

    fn ensure_groups(&mut self) {
        while self.field_groups.len() < N_GROUPS {
            self.field_groups.push(FieldGroupState::default());
        }
    }
}

/// Callback used while merging registered fixed-state fields.
pub type FixedStateMergeVisitor<'a> = dyn FnMut(
        usize,
        &mut dyn ReplicatedFieldHandlerBase,
        &dyn ReplicatedFieldHandlerBase,
        &mut dyn ReplicatedFieldHandlerBase,
    ) -> Result<(), MarshalerError>
    + 'a;

/// Registers fixed-state fields in stable declaration order.
pub trait FixedStateRegister {
    const FIELD_COUNT: usize;

    fn visit_registered_fields<'a>(
        &'a self,
        first_index: usize,
        visit: &mut dyn FnMut(usize, &'a dyn ReplicatedFieldHandlerBase),
    );

    /// Visit registered fields mutably.
    ///
    /// # Errors
    ///
    /// Returns the first error reported by the visitor.
    fn try_visit_registered_fields_mut(
        &mut self,
        first_index: usize,
        visit: &mut dyn FnMut(
            usize,
            &mut dyn ReplicatedFieldHandlerBase,
        ) -> Result<(), MarshalerError>,
    ) -> Result<(), MarshalerError>;

    /// Visit old/new/merged field triples while merging.
    ///
    /// # Errors
    ///
    /// Returns the first error reported by the visitor.
    fn try_visit_registered_fields_for_merge(
        &mut self,
        old_state: &Self,
        new_state: &mut Self,
        first_index: usize,
        visit: &mut FixedStateMergeVisitor<'_>,
    ) -> Result<(), MarshalerError>;
}

impl<T, M> FixedStateRegister for ReplicatedFieldHandler<T, M>
where
    T: Default + PartialEq + Clone + 'static,
    M: Codec<T> + 'static,
{
    const FIELD_COUNT: usize = 1;

    fn visit_registered_fields<'a>(
        &'a self,
        first_index: usize,
        visit: &mut dyn FnMut(usize, &'a dyn ReplicatedFieldHandlerBase),
    ) {
        visit(first_index, self);
    }

    fn try_visit_registered_fields_mut(
        &mut self,
        first_index: usize,
        visit: &mut dyn FnMut(
            usize,
            &mut dyn ReplicatedFieldHandlerBase,
        ) -> Result<(), MarshalerError>,
    ) -> Result<(), MarshalerError> {
        visit(first_index, self)
    }

    fn try_visit_registered_fields_for_merge(
        &mut self,
        old_state: &Self,
        new_state: &mut Self,
        first_index: usize,
        visit: &mut FixedStateMergeVisitor<'_>,
    ) -> Result<(), MarshalerError> {
        visit(first_index, self, old_state, new_state)
    }
}

impl<T, const N: usize> FixedStateRegister for [T; N]
where
    T: FixedStateRegister,
{
    const FIELD_COUNT: usize = T::FIELD_COUNT * N;

    fn visit_registered_fields<'a>(
        &'a self,
        first_index: usize,
        visit: &mut dyn FnMut(usize, &'a dyn ReplicatedFieldHandlerBase),
    ) {
        let mut index = first_index;
        for item in self {
            item.visit_registered_fields(index, visit);
            index += T::FIELD_COUNT;
        }
    }

    fn try_visit_registered_fields_mut(
        &mut self,
        first_index: usize,
        visit: &mut dyn FnMut(
            usize,
            &mut dyn ReplicatedFieldHandlerBase,
        ) -> Result<(), MarshalerError>,
    ) -> Result<(), MarshalerError> {
        let mut index = first_index;
        for item in self {
            item.try_visit_registered_fields_mut(index, visit)?;
            index += T::FIELD_COUNT;
        }
        Ok(())
    }

    fn try_visit_registered_fields_for_merge(
        &mut self,
        old_state: &Self,
        new_state: &mut Self,
        first_index: usize,
        visit: &mut FixedStateMergeVisitor<'_>,
    ) -> Result<(), MarshalerError> {
        let mut index = first_index;
        for ((merged, old), new) in self
            .iter_mut()
            .zip(old_state.iter())
            .zip(new_state.iter_mut())
        {
            merged.try_visit_registered_fields_for_merge(old, new, index, visit)?;
            index += T::FIELD_COUNT;
        }
        Ok(())
    }
}

impl<T, const DELTA_RANGE: u32, AbsoluteM, RelativeM> FixedStateRegister
    for DeltaCompressedReplicatedFieldHandler<T, DELTA_RANGE, AbsoluteM, RelativeM>
where
    T: DeltaRangeValue + Clone + 'static,
    AbsoluteM: Codec<T> + 'static,
    RelativeM: Codec<T> + 'static,
{
    const FIELD_COUNT: usize = 2;

    fn visit_registered_fields<'a>(
        &'a self,
        first_index: usize,
        visit: &mut dyn FnMut(usize, &'a dyn ReplicatedFieldHandlerBase),
    ) {
        self.absolute_portion
            .visit_registered_fields(first_index, visit);
        self.relative_portion
            .visit_registered_fields(first_index + 1, visit);
    }

    fn try_visit_registered_fields_mut(
        &mut self,
        first_index: usize,
        visit: &mut dyn FnMut(
            usize,
            &mut dyn ReplicatedFieldHandlerBase,
        ) -> Result<(), MarshalerError>,
    ) -> Result<(), MarshalerError> {
        self.absolute_portion
            .try_visit_registered_fields_mut(first_index, visit)?;
        self.relative_portion
            .try_visit_registered_fields_mut(first_index + 1, visit)
    }

    fn try_visit_registered_fields_for_merge(
        &mut self,
        old_state: &Self,
        new_state: &mut Self,
        first_index: usize,
        visit: &mut FixedStateMergeVisitor<'_>,
    ) -> Result<(), MarshalerError> {
        self.absolute_portion
            .try_visit_registered_fields_for_merge(
                &old_state.absolute_portion,
                &mut new_state.absolute_portion,
                first_index,
                visit,
            )?;
        self.relative_portion.try_visit_registered_fields_for_merge(
            &old_state.relative_portion,
            &mut new_state.relative_portion,
            first_index + 1,
            visit,
        )
    }
}

impl FixedStateRegister for DeltaCompressedCounterHandler {
    const FIELD_COUNT: usize = 2;

    fn visit_registered_fields<'a>(
        &'a self,
        first_index: usize,
        visit: &mut dyn FnMut(usize, &'a dyn ReplicatedFieldHandlerBase),
    ) {
        self.absolute_portion
            .visit_registered_fields(first_index, visit);
        self.relative_portion
            .visit_registered_fields(first_index + 1, visit);
    }

    fn try_visit_registered_fields_mut(
        &mut self,
        first_index: usize,
        visit: &mut dyn FnMut(
            usize,
            &mut dyn ReplicatedFieldHandlerBase,
        ) -> Result<(), MarshalerError>,
    ) -> Result<(), MarshalerError> {
        self.absolute_portion
            .try_visit_registered_fields_mut(first_index, visit)?;
        self.relative_portion
            .try_visit_registered_fields_mut(first_index + 1, visit)
    }

    fn try_visit_registered_fields_for_merge(
        &mut self,
        old_state: &Self,
        new_state: &mut Self,
        first_index: usize,
        visit: &mut FixedStateMergeVisitor<'_>,
    ) -> Result<(), MarshalerError> {
        self.absolute_portion
            .try_visit_registered_fields_for_merge(
                &old_state.absolute_portion,
                &mut new_state.absolute_portion,
                first_index,
                visit,
            )?;
        self.relative_portion.try_visit_registered_fields_for_merge(
            &old_state.relative_portion,
            &mut new_state.relative_portion,
            first_index + 1,
            visit,
        )
    }
}

impl<AbsoluteM> FixedStateRegister for DynamicDeltaReplicatedFieldHandler<AbsoluteM>
where
    AbsoluteM: Codec<Vec3> + 'static,
{
    const FIELD_COUNT: usize = 3;

    fn visit_registered_fields<'a>(
        &'a self,
        first_index: usize,
        visit: &mut dyn FnMut(usize, &'a dyn ReplicatedFieldHandlerBase),
    ) {
        self.absolute_portion
            .visit_registered_fields(first_index, visit);
        self.quantized_relative_portion
            .visit_registered_fields(first_index + 1, visit);
        self.quantization
            .visit_registered_fields(first_index + 2, visit);
    }

    fn try_visit_registered_fields_mut(
        &mut self,
        first_index: usize,
        visit: &mut dyn FnMut(
            usize,
            &mut dyn ReplicatedFieldHandlerBase,
        ) -> Result<(), MarshalerError>,
    ) -> Result<(), MarshalerError> {
        self.absolute_portion
            .try_visit_registered_fields_mut(first_index, visit)?;
        self.quantized_relative_portion
            .try_visit_registered_fields_mut(first_index + 1, visit)?;
        self.quantization
            .try_visit_registered_fields_mut(first_index + 2, visit)
    }

    fn try_visit_registered_fields_for_merge(
        &mut self,
        old_state: &Self,
        new_state: &mut Self,
        first_index: usize,
        visit: &mut FixedStateMergeVisitor<'_>,
    ) -> Result<(), MarshalerError> {
        self.absolute_portion
            .try_visit_registered_fields_for_merge(
                &old_state.absolute_portion,
                &mut new_state.absolute_portion,
                first_index,
                visit,
            )?;
        self.quantized_relative_portion
            .try_visit_registered_fields_for_merge(
                &old_state.quantized_relative_portion,
                &mut new_state.quantized_relative_portion,
                first_index + 1,
                visit,
            )?;
        self.quantization.try_visit_registered_fields_for_merge(
            &old_state.quantization,
            &mut new_state.quantization,
            first_index + 2,
            visit,
        )
    }
}

impl<const QUANTIZATION: u32, const ROLLOVER_THRESHOLD: u32> FixedStateRegister
    for FloatTimerDeltaReplicatedField<QUANTIZATION, ROLLOVER_THRESHOLD>
{
    const FIELD_COUNT: usize = 4;

    fn visit_registered_fields<'a>(
        &'a self,
        first_index: usize,
        visit: &mut dyn FnMut(usize, &'a dyn ReplicatedFieldHandlerBase),
    ) {
        for (offset, field) in self.data.iter().enumerate() {
            field.visit_registered_fields(first_index + offset, visit);
        }
    }

    fn try_visit_registered_fields_mut(
        &mut self,
        first_index: usize,
        visit: &mut dyn FnMut(
            usize,
            &mut dyn ReplicatedFieldHandlerBase,
        ) -> Result<(), MarshalerError>,
    ) -> Result<(), MarshalerError> {
        for (offset, field) in self.data.iter_mut().enumerate() {
            field.try_visit_registered_fields_mut(first_index + offset, visit)?;
        }
        Ok(())
    }

    fn try_visit_registered_fields_for_merge(
        &mut self,
        old_state: &Self,
        new_state: &mut Self,
        first_index: usize,
        visit: &mut FixedStateMergeVisitor<'_>,
    ) -> Result<(), MarshalerError> {
        for (offset, ((merged, old), new)) in self
            .data
            .iter_mut()
            .zip(old_state.data.iter())
            .zip(new_state.data.iter_mut())
            .enumerate()
        {
            merged.try_visit_registered_fields_for_merge(old, new, first_index + offset, visit)?;
        }
        Ok(())
    }
}

/// Exposes fixed replicated-state fields for traversal and wire operations.
pub trait FixedReplicatedStateFields<
    const N_GROUPS: usize,
    const N_FIELDS_PER_GROUP: usize,
    const CLIENT_WHITELIST_SIZE: usize = 0,
    const N_USER_ATTRIBUTES: usize = 0,
>: Fragment
{
    fn fixed_replicated_state(
        &self,
    ) -> &FixedReplicatedState<N_GROUPS, N_FIELDS_PER_GROUP, CLIENT_WHITELIST_SIZE, N_USER_ATTRIBUTES>;

    fn fixed_replicated_state_mut(
        &mut self,
    ) -> &mut FixedReplicatedState<
        N_GROUPS,
        N_FIELDS_PER_GROUP,
        CLIENT_WHITELIST_SIZE,
        N_USER_ATTRIBUTES,
    >;

    fn fixed_group_field_count(&self, _group_idx: usize) -> Option<usize> {
        None
    }

    /// Visits borrowed field handlers on demand instead of storing
    /// self-references inside the state object.
    fn visit_fixed_fields<'a>(
        &'a self,
        _group_idx: usize,
        _visit: impl FnMut(usize, &'a dyn ReplicatedFieldHandlerBase),
    ) {
    }

    /// Visit mutable field handlers in one fixed group.
    ///
    /// # Errors
    ///
    /// Returns the first error reported by the visitor.
    fn try_visit_fixed_fields_mut(
        &mut self,
        _group_idx: usize,
        _visit: impl FnMut(usize, &mut dyn ReplicatedFieldHandlerBase) -> Result<(), MarshalerError>,
    ) -> Result<(), MarshalerError> {
        Ok(())
    }

    /// Visit old/new/merged field triples for one fixed group.
    ///
    /// # Errors
    ///
    /// Returns the first error reported by the visitor.
    fn try_visit_fixed_fields_for_merge(
        &mut self,
        _old_state: &Self,
        _new_state: &mut Self,
        _group_idx: usize,
        _visit: impl FnMut(
            usize,
            &mut dyn ReplicatedFieldHandlerBase,
            &dyn ReplicatedFieldHandlerBase,
            &mut dyn ReplicatedFieldHandlerBase,
        ) -> Result<(), MarshalerError>,
    ) -> Result<(), MarshalerError>
    where
        Self: Sized,
    {
        Ok(())
    }

    fn num_filter_groups(&self) -> usize {
        N_GROUPS
    }

    fn should_send_to_client(&self, client_id: ClientActorHash, group_idx: GroupIndex) -> bool {
        self.fixed_replicated_state()
            .should_send_to_client(client_id, group_idx)
    }

    fn add_client_to_replication_whitelist(
        &mut self,
        client_id: ClientActorHash,
        group_idx: GroupIndex,
    ) -> bool {
        self.fixed_replicated_state_mut()
            .add_client_to_replication_whitelist(client_id, group_idx)
    }

    fn remove_client_from_replication_whitelist(
        &mut self,
        client_id: ClientActorHash,
        group_idx: GroupIndex,
    ) -> bool {
        self.fixed_replicated_state_mut()
            .remove_client_from_replication_whitelist(client_id, group_idx)
    }

    fn clear_replication_whitelist(&mut self, group_idx: GroupIndex) -> bool {
        self.fixed_replicated_state_mut()
            .clear_replication_whitelist(group_idx)
    }

    fn merge_registered_field(
        merged_field: &mut dyn ReplicatedFieldHandlerBase,
        old_field: &dyn ReplicatedFieldHandlerBase,
        new_field: &mut dyn ReplicatedFieldHandlerBase,
        seq: SequenceNumber,
        inherit_previous_network_data_status: bool,
        outcome: &mut FixedMergeOutcome,
    ) {
        outcome.detected_new_data_in_last_merge |= merged_field.merge_and_update_sequence(
            old_field,
            new_field,
            seq,
            inherit_previous_network_data_status,
        );
        outcome.last_modified = outcome.last_modified.max(merged_field.last_modified());
        outcome.has_new_network_data |= merged_field.has_new_network_data();
    }

    fn merge_fixed_and_update_sequence(
        &self,
        new_fragment: &mut dyn Fragment,
        seq: SequenceNumber,
        inherit_previous_network_data_status: bool,
    ) -> Option<Box<dyn Fragment>>
    where
        Self: Default + Sized + 'static,
    {
        debug_assert!(seq.is_valid(), "Merge-to sequence should never be invalid");
        let new_correlation_id = new_fragment.correlation_id();
        let new_state = new_fragment.downcast_mut::<Self>()?;
        let mut merged_state = Self::default();
        let mut outcome = FixedMergeOutcome::default();

        for group_idx in 0..N_GROUPS {
            merged_state
                .try_visit_fixed_fields_for_merge(
                    self,
                    new_state,
                    group_idx,
                    |_, merged_field, old_field, new_field| {
                        Self::merge_registered_field(
                            merged_field,
                            old_field,
                            new_field,
                            seq,
                            inherit_previous_network_data_status,
                            &mut outcome,
                        );
                        Ok(())
                    },
                )
                .ok()?;
        }

        merged_state
            .fixed_replicated_state_mut()
            .merge_client_whitelist_attributes(
                self.fixed_replicated_state(),
                new_state.fixed_replicated_state_mut(),
                seq,
                inherit_previous_network_data_status,
                &mut outcome,
            );
        merged_state.finish_merge(seq, new_correlation_id, outcome);
        Some(Box::new(merged_state))
    }

    /// fully-merged flag, and correlation id on the merged state.
    fn finish_merge(
        &mut self,
        seq: SequenceNumber,
        correlation_id: uuid::Uuid,
        outcome: FixedMergeOutcome,
    ) {
        self.fixed_replicated_state_mut()
            .finish_merge(seq, correlation_id, outcome);
    }

    fn reset_fixed_has_new_network_data(&mut self) {
        for group_idx in 0..N_GROUPS {
            let _ = self.try_visit_fixed_fields_mut(group_idx, |_, field| {
                field.reset_has_new_network_data();
                Ok(())
            });
        }
        let fixed_state = self.fixed_replicated_state_mut();
        fixed_state.reset_client_whitelist_network_data();
        fixed_state.reset_has_new_network_data();
    }

    fn set_fixed_has_new_network_data_on_initial_state(&mut self) {
        self.fixed_replicated_state_mut()
            .set_has_new_network_data_on_initial_state();
    }

    fn marshal_fields(
        &self,
        group_idx: usize,
        baseline: SequenceNumber,
        wb: &mut WriteBuffer,
    ) -> bool
    where
        Self: Sized,
    {
        let Some(field_count) = self.fixed_group_field_count(group_idx) else {
            return false;
        };
        self.marshal_visited_fields(field_count, baseline, wb, |state, visit| {
            state.visit_fixed_fields(group_idx, |index, field| {
                visit(index, field);
            });
        })
    }

    /// Decode one fixed field group.
    ///
    /// # Errors
    ///
    /// Returns the first error reported by the field-mask or field decoders.
    fn unmarshal_fields(
        &mut self,
        group_idx: usize,
        rb: &mut ReadBuffer,
    ) -> Result<bool, MarshalerError>
    where
        Self: Sized,
    {
        let Some(field_count) = self.fixed_group_field_count(group_idx) else {
            return Ok(false);
        };
        self.unmarshal_visited_fields(group_idx, field_count, rb, |state, visit| {
            state.try_visit_fixed_fields_mut(group_idx, |index, field| visit(index, field))
        })
    }

    fn marshal_fixed_contents(&self, mc: &MarshalContext<'_>, wb: &mut WriteBuffer) -> bool
    where
        Self: Sized,
    {
        let mut body = WriteBuffer::new(wb.endian());
        let mut group_present: GroupPresentFlagBitset<N_GROUPS> = [false; N_GROUPS];

        for (group_idx, present) in group_present.iter_mut().enumerate() {
            if let Some(client_id) = mc.filter_target
                && !self.should_send_to_client(client_id, GroupIndex::new(group_idx))
            {
                continue;
            }

            let baseline = mc.group_baselines.map_or(mc.baseline_seq, |baselines| {
                baselines.baseline_for(GroupIndex::new(group_idx), mc.baseline_seq)
            });
            *present = self.marshal_fields(group_idx, baseline, &mut body);
        }

        if !group_present.iter().any(|present| *present) {
            return false;
        }

        write_group_present_flags(&group_present, wb);
        wb.write_bytes(body.as_slice());
        true
    }

    /// Decode the fixed-state group-present mask and all present groups.
    ///
    /// # Errors
    ///
    /// Returns the first error reported by the group-present mask or field decoders.
    fn unmarshal_fixed_contents(&mut self, rb: &mut ReadBuffer) -> Result<bool, MarshalerError>
    where
        Self: Sized,
    {
        let group_present = read_group_present_flags::<N_GROUPS>(rb)?;
        let mut read_any = false;

        for (group_idx, present) in group_present.iter().copied().enumerate() {
            if present {
                read_any |= self.unmarshal_fields(group_idx, rb)?;
            }
        }

        Ok(read_any)
    }

    fn marshal_registered_fields(
        &self,
        field_count: usize,
        baseline: SequenceNumber,
        wb: &mut WriteBuffer,
        mut is_dirty: impl FnMut(&Self, usize, SequenceNumber) -> bool,
        mut marshal_field: impl FnMut(&Self, usize, SequenceNumber, &mut WriteBuffer),
    ) -> bool
    where
        Self: Sized,
    {
        debug_assert!(
            field_count <= 64,
            "FixedReplicatedState supports at most 64 fields"
        );

        let mut field_mask = 0u64;
        for index in 0..field_count {
            if is_dirty(self, index, baseline) {
                field_mask |= 1u64 << index;
            }
        }

        if field_mask == 0 {
            return false;
        }

        VlqU64Marshaler.marshal(wb, field_mask);
        for index in 0..field_count {
            if (field_mask & (1u64 << index)) != 0 {
                marshal_field(self, index, baseline, wb);
            }
        }
        true
    }

    /// `NamedField` visitor instead of a stored self-referential registry.
    fn marshal_visited_fields(
        &self,
        field_count: usize,
        baseline: SequenceNumber,
        wb: &mut WriteBuffer,
        mut visit_fields: impl FnMut(&Self, &mut dyn FnMut(usize, &dyn ReplicatedFieldHandlerBase)),
    ) -> bool
    where
        Self: Sized,
    {
        debug_assert!(
            field_count <= 64,
            "FixedReplicatedState supports at most 64 fields"
        );

        let mut field_mask = 0u64;
        visit_fields(self, &mut |index, field| {
            debug_assert!(index < field_count);
            if index < field_count && field.is_dirty(baseline) {
                field_mask |= 1u64 << index;
            }
        });

        if field_mask == 0 {
            return false;
        }

        VlqU64Marshaler.marshal(wb, field_mask);
        visit_fields(self, &mut |index, field| {
            debug_assert!(index < field_count);
            if index < field_count && (field_mask & (1u64 << index)) != 0 {
                field.marshal_field_since(wb, baseline);
            }
        });
        true
    }

    /// Decode a field presence mask and invoke `unmarshal_field` for each set bit.
    ///
    /// # Errors
    ///
    /// Returns the first error reported by the mask reader or field callback.
    fn unmarshal_registered_fields(
        &mut self,
        field_count: usize,
        rb: &mut ReadBuffer,
        mut unmarshal_field: impl FnMut(&mut Self, usize, &mut ReadBuffer) -> Result<(), MarshalerError>,
    ) -> Result<bool, MarshalerError>
    where
        Self: Sized,
    {
        debug_assert!(
            field_count <= 64,
            "FixedReplicatedState supports at most 64 fields"
        );

        let presence = VlqU64Marshaler.unmarshal(rb)?;
        if presence == 0 {
            return Ok(false);
        }

        for index in 0..field_count {
            if (presence & (1u64 << index)) != 0 {
                unmarshal_field(self, index, rb)?;
            }
        }
        Ok(true)
    }

    /// mutable `NamedField` visitor.
    /// Decode fields selected by a presence mask through a mutable visitor.
    ///
    /// # Errors
    ///
    /// Returns the first error reported by the mask reader, visitor, or field decoder.
    fn unmarshal_visited_fields(
        &mut self,
        _group_idx: usize,
        field_count: usize,
        rb: &mut ReadBuffer,
        mut visit_fields: impl FnMut(
            &mut Self,
            &mut dyn FnMut(usize, &mut dyn ReplicatedFieldHandlerBase) -> Result<(), MarshalerError>,
        ) -> Result<(), MarshalerError>,
    ) -> Result<bool, MarshalerError>
    where
        Self: Sized,
    {
        debug_assert!(
            field_count <= 64,
            "FixedReplicatedState supports at most 64 fields"
        );

        let presence = VlqU64Marshaler.unmarshal(rb)?;
        if presence == 0 {
            return Ok(false);
        }

        visit_fields(self, &mut |index, field| {
            debug_assert!(index < field_count);
            if index < field_count && (presence & (1u64 << index)) != 0 {
                field.unmarshal_field(rb)?;
            }
            Ok(())
        })?;
        Ok(true)
    }

    /// zero field mask when no attribute is dirty because the read path
    fn marshal_registered_attributes(
        &self,
        attr_count: usize,
        baseline: SequenceNumber,
        wb: &mut WriteBuffer,
        is_dirty: impl FnMut(&Self, usize, SequenceNumber) -> bool,
        marshal_attr: impl FnMut(&Self, usize, SequenceNumber, &mut WriteBuffer),
    ) -> bool
    where
        Self: Sized,
    {
        if !self.marshal_registered_fields(attr_count, baseline, wb, is_dirty, marshal_attr) {
            wb.write_u8(0);
        }
        true
    }

    /// Decode registered attributes selected by a presence mask.
    ///
    /// # Errors
    ///
    /// Returns the first error reported by the mask reader or attribute callback.
    fn unmarshal_registered_attributes(
        &mut self,
        attr_count: usize,
        rb: &mut ReadBuffer,
        unmarshal_attr: impl FnMut(&mut Self, usize, &mut ReadBuffer) -> Result<(), MarshalerError>,
    ) -> Result<bool, MarshalerError>
    where
        Self: Sized,
    {
        self.unmarshal_registered_fields(attr_count, rb, unmarshal_attr)
    }

    fn marshal_field_metadata_sequences(
        &self,
        wb: &mut WriteBuffer,
        sequences: impl IntoIterator<Item = SequenceNumber>,
        mut marshal_sequence: impl FnMut(SequenceNumber, &mut WriteBuffer),
    ) -> bool {
        for sequence in sequences {
            marshal_sequence(sequence, wb);
        }
        true
    }

    /// Decode fixed field metadata sequences.
    ///
    /// # Errors
    ///
    /// Returns the first error reported by the sequence decoder.
    fn unmarshal_field_metadata_sequences(
        &mut self,
        rb: &mut ReadBuffer,
        sequence_count: usize,
        mut unmarshal_sequence: impl FnMut(&mut ReadBuffer) -> Result<SequenceNumber, MarshalerError>,
        mut apply_sequence: impl FnMut(&mut Self, usize, SequenceNumber),
    ) -> Result<bool, MarshalerError>
    where
        Self: Sized,
    {
        for index in 0..sequence_count {
            let sequence = unmarshal_sequence(rb)?;
            apply_sequence(self, index, sequence);
        }
        Ok(true)
    }
}

pub struct NamedField<'a> {
    pub name: &'a str,
    pub field_handler: &'a dyn ReplicatedFieldHandlerBase,
}

pub struct NamedFieldMut<'a> {
    pub name: &'a str,
    pub field_handler: &'a mut dyn ReplicatedFieldHandlerBase,
}

pub type FieldVector<'a, const N_FIELDS_PER_GROUP: usize> =
    ArrayVec<NamedField<'a>, N_FIELDS_PER_GROUP>;
pub type FieldVectorMut<'a, const N_FIELDS_PER_GROUP: usize> =
    ArrayVec<NamedFieldMut<'a>, N_FIELDS_PER_GROUP>;

pub struct FieldGroup<'a, const N_FIELDS_PER_GROUP: usize, const CLIENT_WHITELIST_SIZE: usize> {
    pub fields: FieldVector<'a, N_FIELDS_PER_GROUP>,
    pub client_whitelist: &'a ClientWhitelistField<CLIENT_WHITELIST_SIZE>,
}

pub struct FieldGroupMut<'a, const N_FIELDS_PER_GROUP: usize, const CLIENT_WHITELIST_SIZE: usize> {
    pub fields: FieldVectorMut<'a, N_FIELDS_PER_GROUP>,
    pub client_whitelist: &'a mut ClientWhitelistField<CLIENT_WHITELIST_SIZE>,
}

///
/// bytes and patches the value after writing group bodies. In the recovered
fn write_group_present_flags<const N_GROUPS: usize>(
    group_present: &GroupPresentFlagBitset<N_GROUPS>,
    wb: &mut WriteBuffer,
) {
    let byte_count = N_GROUPS.div_ceil(8).max(1);
    for byte_idx in 0..byte_count {
        let mut byte = 0u8;
        let start = byte_idx * 8;
        let end = (start + 8).min(N_GROUPS);
        for (bit, present) in group_present[start..end].iter().copied().enumerate() {
            if present {
                byte |= 1u8 << bit;
            }
        }
        byte.marshal(wb);
    }
}

fn read_group_present_flags<const N_GROUPS: usize>(
    rb: &mut ReadBuffer,
) -> Result<GroupPresentFlagBitset<N_GROUPS>, MarshalerError> {
    let byte_count = N_GROUPS.div_ceil(8).max(1);
    let mut flags = [false; N_GROUPS];
    for byte_idx in 0..byte_count {
        let byte = u8::unmarshal(rb)?;
        let start = byte_idx * 8;
        let end = (start + 8).min(N_GROUPS);
        for bit in 0..(end - start) {
            flags[start + bit] = (byte & (1u8 << bit)) != 0;
        }
    }
    Ok(flags)
}

/// Marshal one fixed field group by index.
///
/// presence mask only when at least one field is dirty, then write each dirty
/// field body in registration order.
#[cfg(test)]
fn marshal_registered_fields_with(
    field_count: usize,
    wb: &mut WriteBuffer,
    mut is_dirty: impl FnMut(usize) -> bool,
    mut marshal_field: impl FnMut(usize, &mut WriteBuffer),
) -> bool {
    debug_assert!(
        field_count <= 64,
        "FixedReplicatedState supports at most 64 fields"
    );

    let mut field_mask = 0u64;
    for index in 0..field_count {
        if is_dirty(index) {
            field_mask |= 1u64 << index;
        }
    }

    if field_mask == 0 {
        return false;
    }

    VlqU64Marshaler.marshal(wb, field_mask);
    for index in 0..field_count {
        if (field_mask & (1u64 << index)) != 0 {
            marshal_field(index, wb);
        }
    }
    true
}

/// Unmarshal one fixed field group by index.
#[cfg(test)]
fn unmarshal_registered_fields_with(
    field_count: usize,
    rb: &mut ReadBuffer,
    mut unmarshal_field: impl FnMut(usize, &mut ReadBuffer) -> Result<(), MarshalerError>,
) -> Result<bool, MarshalerError> {
    debug_assert!(
        field_count <= 64,
        "FixedReplicatedState supports at most 64 fields"
    );

    let presence = VlqU64Marshaler.unmarshal(rb)?;
    if presence == 0 {
        return Ok(false);
    }

    for index in 0..field_count {
        if (presence & (1u64 << index)) != 0 {
            unmarshal_field(index, rb)?;
        }
    }
    Ok(true)
}

/// Marshal one fixed field group — VLQ-u64 presence bitset + dirty field bodies.
///
/// is dirty relative to `baseline` (caller may write a zero sentinel before
#[cfg(test)]
fn marshal_named_field_handlers(
    fields: &[&dyn ReplicatedFieldHandlerBase],
    baseline: SequenceNumber,
    wb: &mut WriteBuffer,
) -> bool {
    marshal_registered_fields_with(
        fields.len(),
        wb,
        |index| fields[index].is_dirty(baseline),
        |index, wb| fields[index].marshal_field_since(wb, baseline),
    )
}

#[cfg(test)]
fn marshal_named_fields(
    fields: &[NamedField<'_>],
    baseline: SequenceNumber,
    wb: &mut WriteBuffer,
) -> bool {
    marshal_registered_fields_with(
        fields.len(),
        wb,
        |index| fields[index].field_handler.is_dirty(baseline),
        |index, wb| {
            fields[index]
                .field_handler
                .marshal_field_since(wb, baseline);
        },
    )
}

#[cfg(test)]
fn unmarshal_named_field_handlers(
    fields: &mut [&mut dyn ReplicatedFieldHandlerBase],
    rb: &mut ReadBuffer,
) -> Result<bool, MarshalerError> {
    unmarshal_registered_fields_with(fields.len(), rb, |index, rb| {
        fields[index].unmarshal_field(rb)
    })
}

///
/// Writes no bytes and returns `false` when no group has dirty fields. When
/// one or more groups are dirty, the output is:
///
/// ```text
/// for each present group:
///   VlqU64 field_present
/// ```
#[cfg(test)]
fn marshal_attributes(
    attributes: &[NamedField<'_>],
    baseline: SequenceNumber,
    wb: &mut WriteBuffer,
) -> bool {
    if !marshal_named_fields(attributes, baseline, wb) {
        wb.write_u8(0);
    }
    true
}

fn marshal_client_whitelist<const CLIENT_WHITELIST_SIZE: usize>(
    whitelist: &[ClientActorHash],
    wb: &mut WriteBuffer,
) {
    debug_assert!(
        whitelist.len() <= CLIENT_WHITELIST_SIZE,
        "ClientWhitelist exceeds FixedReplicatedState capacity"
    );
    if whitelist.len() > CLIENT_WHITELIST_SIZE {
        VlqU32Marshaler.marshal(wb, 0);
        return;
    }

    let len = u32::try_from(whitelist.len()).expect("client whitelist cap fits in u32");
    VlqU32Marshaler.marshal(wb, len);
    for client_id in whitelist {
        client_id.marshal(wb);
    }
}

fn unmarshal_client_whitelist<const CLIENT_WHITELIST_SIZE: usize>(
    rb: &mut ReadBuffer,
) -> Result<ClientWhitelistVector<CLIENT_WHITELIST_SIZE>, MarshalerError> {
    let len = usize::try_from(VlqU32Marshaler.unmarshal(rb)?).map_err(|_| {
        MarshalerError::ContainerOverflow {
            len: usize::MAX,
            capacity: CLIENT_WHITELIST_SIZE,
        }
    })?;
    if len > CLIENT_WHITELIST_SIZE {
        return Err(MarshalerError::ContainerOverflow {
            len,
            capacity: CLIENT_WHITELIST_SIZE,
        });
    }

    let mut whitelist = ArrayVec::new();
    for _ in 0..len {
        whitelist.push(ClientActorHash::unmarshal(rb)?);
    }
    Ok(whitelist)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::DynFragment;
    use crate::serialize::buffer::CARRIER_ENDIAN;
    use crate::serialize::replicated_field::ReplicatedFieldHandler;

    #[test]
    fn fixed_group_fields_omit_empty_mask_when_nothing_is_dirty() {
        let field = ReplicatedFieldHandler::<u8>::default();
        let fields: [&dyn ReplicatedFieldHandlerBase; 1] = [&field];
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);

        assert!(!marshal_named_field_handlers(
            &fields,
            SequenceNumber::Invalid,
            &mut wb
        ));
        assert!(wb.as_slice().is_empty());
    }

    #[test]
    fn fixed_group_fields_write_field_mask_then_payload() {
        let mut field = ReplicatedFieldHandler::<u8>::default();
        field.set_value(0x2a);
        let fields: [&dyn ReplicatedFieldHandlerBase; 1] = [&field];
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);

        assert!(marshal_named_field_handlers(
            &fields,
            SequenceNumber::Invalid,
            &mut wb
        ));
        assert_eq!(wb.as_slice(), &[0x01, 0x2a]);
    }

    #[test]
    fn fixed_group_fields_unmarshal_present_fields() {
        let bytes = [0x01, 0x2a];
        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bytes);
        let mut field = ReplicatedFieldHandler::<u8>::default();
        let mut fields: [&mut dyn ReplicatedFieldHandlerBase; 1] = [&mut field];

        assert!(unmarshal_named_field_handlers(&mut fields, &mut rb).unwrap());
        assert_eq!(field.value().copied(), Some(0x2a));
        assert_eq!(rb.left(), 0);
    }

    #[test]
    fn fixed_attributes_write_zero_mask_when_empty() {
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);

        assert!(marshal_attributes(&[], SequenceNumber::Invalid, &mut wb));
        assert_eq!(wb.as_slice(), &[0]);
    }

    #[test]
    fn fixed_client_whitelist_attributes_are_replicated_fields() {
        let mut fixed_state = FixedReplicatedState::<2, 1, 1>::default();
        assert!(
            fixed_state
                .add_client_to_replication_whitelist(ClientActorHash::new(7), GroupIndex::new(1))
        );

        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        assert!(fixed_state.marshal_client_whitelist_attributes(SequenceNumber::Invalid, &mut wb));

        assert_eq!(&wb.as_slice()[0..3], &[0x03, 0x00, 0x01]);
        let mut expected_client_id = WriteBuffer::new(CARRIER_ENDIAN);
        7u64.marshal(&mut expected_client_id);
        assert_eq!(&wb.as_slice()[3..11], expected_client_id.as_slice());

        let mut decoded = FixedReplicatedState::<2, 1, 1>::default();
        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, wb.as_slice());
        assert!(
            decoded
                .unmarshal_client_whitelist_attributes(&mut rb)
                .unwrap()
        );
        assert_eq!(
            decoded.client_whitelist(GroupIndex::new(0)),
            Some(&[] as &[ClientActorHash])
        );
        assert_eq!(
            decoded.client_whitelist(GroupIndex::new(1)),
            Some([ClientActorHash::new(7)].as_slice())
        );
        assert_eq!(rb.left(), 0);
    }

    #[test]
    fn fixed_whitelist_filters_when_non_empty() {
        #[derive(Debug, Default)]
        struct Demo {
            fixed_state: FixedReplicatedState<2, 1, 2>,
        }

        impl DynFragment for Demo {
            fn base(&self) -> &FragmentBase {
                self.fixed_state.base()
            }

            fn base_mut(&mut self) -> &mut FragmentBase {
                self.fixed_state.base_mut()
            }

            fn marshal_contents(&self, _wb: &mut WriteBuffer) -> bool {
                false
            }

            fn unmarshal_contents(&mut self, _rb: &mut ReadBuffer) -> Result<bool, MarshalerError> {
                Ok(false)
            }
        }

        impl Fragment for Demo {}

        impl FixedReplicatedStateFields<2, 1, 2> for Demo {
            fn fixed_replicated_state(&self) -> &FixedReplicatedState<2, 1, 2> {
                &self.fixed_state
            }

            fn fixed_replicated_state_mut(&mut self) -> &mut FixedReplicatedState<2, 1, 2> {
                &mut self.fixed_state
            }
        }

        let mut state = Demo::default();
        let group = GroupIndex::new(1);
        assert!(state.should_send_to_client(ClientActorHash::new(10), group));

        assert!(state.add_client_to_replication_whitelist(ClientActorHash::new(10), group));
        assert!(state.should_send_to_client(ClientActorHash::new(10), group));
        assert!(!state.should_send_to_client(ClientActorHash::new(11), group));

        assert!(state.remove_client_from_replication_whitelist(ClientActorHash::new(10), group));
        assert!(state.should_send_to_client(ClientActorHash::new(11), group));
    }
}
