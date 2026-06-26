use std::{any::Any, fmt::Debug};

use glam::Vec3;
use uuid::Uuid;

use super::{ClientActorHash, SequenceNumber, TypeIndex};
use crate::serialize::{MarshalerError, ReadBuffer, WriteBuffer};
use crate::types::{AzRtti, TypeRegistryEntry};

pub const I_FRAGMENT_TYPE_ID: Uuid = Uuid::from_u128(0x766994ea_5c1d_47bf_856c_8216052f5957);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum FragmentCategory {
    #[default]
    Uncategorized = 0,
    PlayerCharacter = 1,
    NonPlayerCharacter = 2,
    ImportantNonPlayerCharacter = 3,
    Spell = 4,
    Projectile = 5,
    Buildable = 6,
    NumCategories = 7,
}

impl FragmentCategory {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Uncategorized => "Uncategorized",
            Self::PlayerCharacter => "PlayerCharacter",
            Self::NonPlayerCharacter => "NonPlayerCharacter",
            Self::ImportantNonPlayerCharacter => "ImportantNonPlayerCharacter",
            Self::Spell => "Spell",
            Self::Projectile => "Projectile",
            Self::Buildable => "Buildable",
            Self::NumCategories => "NumCategories",
        }
    }

    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "Uncategorized" => Some(Self::Uncategorized),
            "PlayerCharacter" => Some(Self::PlayerCharacter),
            "NonPlayerCharacter" => Some(Self::NonPlayerCharacter),
            "ImportantNonPlayerCharacter" => Some(Self::ImportantNonPlayerCharacter),
            "Spell" => Some(Self::Spell),
            "Projectile" => Some(Self::Projectile),
            "Buildable" => Some(Self::Buildable),
            _ => None,
        }
    }
}

pub const NUM_FRAGMENT_CATEGORIES: usize = FragmentCategory::NumCategories as usize;
pub type FragmentCategoryBitset = [bool; NUM_FRAGMENT_CATEGORIES];

#[must_use]
pub const fn fragment_category_to_string(category: FragmentCategory) -> &'static str {
    category.as_str()
}

#[must_use]
pub fn fragment_category_from_string(name: &str) -> Option<FragmentCategory> {
    FragmentCategory::from_name(name)
}

pub const MAXIMUM_REPLICATION_FRAGMENTS: usize = u8::MAX as usize;

pub const FRAGMENT_POOL_SIZE_BYTES: usize = 3 * 1024;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FragmentBase {
    correlation_id: Uuid,
}

impl FragmentBase {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            correlation_id: Uuid::from_u128(0),
        }
    }

    #[must_use]
    pub const fn correlation_id(&self) -> Uuid {
        self.correlation_id
    }

    pub fn set_correlation_id(&mut self, correlation_id: Uuid) {
        self.correlation_id = correlation_id;
    }
}

/// Index of a replicated-state filter/field group.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GroupIndex(usize);

impl GroupIndex {
    #[must_use]
    pub const fn new(value: usize) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> usize {
        self.0
    }
}

impl From<usize> for GroupIndex {
    fn from(value: usize) -> Self {
        Self::new(value)
    }
}

impl From<GroupIndex> for usize {
    fn from(value: GroupIndex) -> Self {
        value.get()
    }
}

impl std::fmt::Display for GroupIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

/// Per-group baselines used when encoding a replicated state.
#[derive(Debug, Clone, Copy)]
pub struct GroupBaselines<'a> {
    baselines: &'a [SequenceNumber],
}

impl<'a> GroupBaselines<'a> {
    #[must_use]
    pub const fn new(baselines: &'a [SequenceNumber]) -> Self {
        Self { baselines }
    }

    #[must_use]
    pub const fn as_slice(self) -> &'a [SequenceNumber] {
        self.baselines
    }

    #[must_use]
    pub fn get(self, group_idx: impl Into<GroupIndex>) -> Option<&'a SequenceNumber> {
        self.baselines.get(group_idx.into().get())
    }

    #[must_use]
    pub fn baseline_for(
        self,
        group_idx: impl Into<GroupIndex>,
        fallback: SequenceNumber,
    ) -> SequenceNumber {
        self.get(group_idx).copied().unwrap_or(fallback)
    }
}

impl<'a> From<&'a [SequenceNumber]> for GroupBaselines<'a> {
    fn from(baselines: &'a [SequenceNumber]) -> Self {
        Self::new(baselines)
    }
}

/// Context used while marshaling replicated fragments.
#[derive(Debug, Clone, Copy)]
pub struct MarshalContext<'a> {
    pub baseline_seq: SequenceNumber,
    pub filter_target: Option<ClientActorHash>,
    pub group_baselines: Option<GroupBaselines<'a>>,
}

impl Default for MarshalContext<'_> {
    fn default() -> Self {
        Self {
            baseline_seq: SequenceNumber::Invalid,
            filter_target: None,
            group_baselines: None,
        }
    }
}

/// Object-safe fragment behavior: base identity plus body/attribute bytes.
pub trait DynFragment: Any + Debug + Send + Sync {
    fn base(&self) -> &FragmentBase;
    fn base_mut(&mut self) -> &mut FragmentBase;

    fn marshal_contents(&self, wb: &mut WriteBuffer) -> bool;

    fn marshal_contents_with(&self, _mc: &MarshalContext<'_>, wb: &mut WriteBuffer) -> bool {
        self.marshal_contents(wb)
    }

    /// Decode the fragment body.
    ///
    /// # Errors
    ///
    /// Returns the first malformed-field or buffer error reported by the body decoder.
    fn unmarshal_contents(&mut self, rb: &mut ReadBuffer) -> Result<bool, MarshalerError>;

    fn marshal_attributes(&self, _mc: &MarshalContext<'_>, _wb: &mut WriteBuffer) -> bool {
        true
    }

    /// Decode fragment attributes.
    ///
    /// # Errors
    ///
    /// Returns the first malformed-field or buffer error reported by the attribute decoder.
    fn unmarshal_attributes(&mut self, _rb: &mut ReadBuffer) -> Result<bool, MarshalerError> {
        Ok(true)
    }

    fn marshal_field_metadata(&self, _mc: &MarshalContext<'_>, _wb: &mut WriteBuffer) -> bool {
        true
    }

    /// Decode per-field metadata.
    ///
    /// # Errors
    ///
    /// Returns the first malformed-field or buffer error reported by the metadata decoder.
    fn unmarshal_field_metadata(&mut self, _rb: &mut ReadBuffer) -> Result<bool, MarshalerError> {
        Ok(true)
    }

    fn full_marshal(&self, mc: &MarshalContext<'_>, wb: &mut WriteBuffer) -> bool {
        let wrote_contents = self.marshal_contents_with(mc, wb);
        let wrote_attributes = self.marshal_attributes(mc, wb);
        let wrote_metadata = self.marshal_field_metadata(mc, wb);
        wrote_contents || wrote_attributes || wrote_metadata
    }

    /// Decode body, attributes, and field metadata in order.
    ///
    /// # Errors
    ///
    /// Returns the first error reported by any fragment section.
    fn full_unmarshal(&mut self, rb: &mut ReadBuffer) -> Result<bool, MarshalerError> {
        let read_contents = self.unmarshal_contents(rb)?;
        let read_attributes = self.unmarshal_attributes(rb)?;
        let read_metadata = self.unmarshal_field_metadata(rb)?;
        Ok(read_contents || read_attributes || read_metadata)
    }
}

impl dyn DynFragment + '_ {
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

/// Replication-facing fragment behavior.
///
/// Concrete states usually embed [`super::ReplicatedState`] or
/// [`super::FixedReplicatedState`] for lifecycle and filtering; byte encoding
/// lives in [`DynFragment`].
pub trait Fragment: DynFragment {
    fn correlation_id(&self) -> Uuid {
        self.base().correlation_id()
    }

    /// Update the route/correlation id carried by this fragment.
    fn set_correlation_id(&mut self, correlation_id: Uuid) {
        self.base_mut().set_correlation_id(correlation_id);
    }

    /// Merge new data into this fragment and advance the update sequence.
    fn merge_and_update_sequence(
        &self,
        _new_fragment: &mut dyn Fragment,
        _seq: SequenceNumber,
        _inherit_previous_network_data_status: bool,
    ) -> Option<Box<dyn Fragment>> {
        None
    }

    fn is_fully_merged_state(&self) -> bool {
        true
    }

    fn has_new_network_data(&self) -> bool {
        false
    }

    fn detected_new_data_in_last_merge(&self) -> bool {
        false
    }

    fn reset_has_new_network_data(&mut self) {}

    fn set_has_new_network_data_on_initial_state(&mut self) {}

    fn update_sequence(&self) -> SequenceNumber {
        SequenceNumber::Invalid
    }

    fn is_fragment_dirty(&self, _baseline: SequenceNumber) -> bool {
        false
    }

    fn params_to_string(&self) -> String {
        "...".to_string()
    }

    fn fragment_to_string(&self) -> String {
        format!(
            "{}({})",
            std::any::type_name::<Self>(),
            self.params_to_string()
        )
    }

    fn is_metadata(&self) -> bool {
        false
    }

    fn category(&self) -> FragmentCategory {
        FragmentCategory::Uncategorized
    }

    fn has_world_position(&self) -> bool {
        false
    }

    fn world_position(&self) -> Option<Vec3> {
        None
    }

    fn transform(&self) -> Option<glam::Mat4> {
        None
    }

    fn num_filter_groups(&self) -> usize {
        1
    }

    fn should_send_to_client_group(
        &self,
        _target: ClientActorHash,
        _group_idx: GroupIndex,
    ) -> bool {
        false
    }

    fn should_send_to_client_any_group(&self, target: ClientActorHash) -> bool {
        (0..self.num_filter_groups())
            .any(|group_idx| self.should_send_to_client_group(target, GroupIndex::new(group_idx)))
    }

    /// Create an empty instance of this fragment for merge/decode paths.
    fn create_new_instance(&self) -> Option<Box<dyn Fragment>> {
        None
    }
}

impl dyn Fragment + '_ {
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

#[cfg(test)]
mod tests {
    use super::{GroupBaselines, GroupIndex};
    use crate::hub::SequenceNumber;

    #[test]
    fn group_baselines_falls_back_when_group_is_missing() {
        let baselines = [SequenceNumber::Seq(10), SequenceNumber::Seq(20)];
        let group_baselines = GroupBaselines::new(&baselines);

        assert_eq!(
            group_baselines.baseline_for(GroupIndex::new(1), SequenceNumber::Invalid),
            SequenceNumber::Seq(20)
        );
        assert_eq!(
            group_baselines.baseline_for(GroupIndex::new(2), SequenceNumber::Invalid),
            SequenceNumber::Invalid
        );
    }
}

pub type FragmentContentsDecodeFn =
    for<'a> fn(&mut ReadBuffer<'a>) -> Result<Box<dyn Fragment>, MarshalerError>;
pub type FragmentContentsConsumeFn = for<'a> fn(&mut ReadBuffer<'a>) -> Result<(), MarshalerError>;
pub type FullFragmentDecodeFn =
    for<'a> fn(&mut ReadBuffer<'a>) -> Result<Box<dyn Fragment>, MarshalerError>;
pub type FullFragmentConsumeFn = for<'a> fn(&mut ReadBuffer<'a>) -> Result<(), MarshalerError>;

pub struct FragmentRegistration {
    pub uuid: fn() -> Uuid,
    pub name: fn() -> &'static str,
    pub type_index: fn() -> u32,
    pub decode_contents: FragmentContentsDecodeFn,
    pub consume_contents: FragmentContentsConsumeFn,
    pub decode_full: FullFragmentDecodeFn,
    pub consume_full: FullFragmentConsumeFn,
}

inventory::collect!(FragmentRegistration);

impl FragmentRegistration {
    #[must_use]
    pub const fn of<T>() -> Self
    where
        T: Fragment + AzRtti + TypeRegistryEntry + Default + Debug + 'static,
    {
        Self {
            uuid: || T::TYPE_ID,
            name: || T::TYPE_NAME,
            type_index: || T::TYPE_INDEX,
            decode_contents: |rb| {
                let mut fragment = T::default();
                fragment.unmarshal_contents(rb)?;
                Ok(Box::new(fragment))
            },
            consume_contents: |rb| {
                let mut fragment = T::default();
                fragment.unmarshal_contents(rb)?;
                Ok(())
            },
            decode_full: |rb| {
                let mut fragment = T::default();
                fragment.full_unmarshal(rb)?;
                Ok(Box::new(fragment))
            },
            consume_full: |rb| {
                let mut fragment = T::default();
                fragment.full_unmarshal(rb)?;
                Ok(())
            },
        }
    }
}

#[must_use]
pub fn fragment_registration_by_uuid(uuid: Uuid) -> Option<&'static FragmentRegistration> {
    inventory::iter::<FragmentRegistration>
        .into_iter()
        .find(|entry| (entry.uuid)() == uuid)
}

#[must_use]
pub fn fragment_registration_by_type_index(
    type_index: impl Into<TypeIndex>,
) -> Option<&'static FragmentRegistration> {
    let type_index = type_index.into();
    inventory::iter::<FragmentRegistration>
        .into_iter()
        .find(|entry| TypeIndex::new((entry.type_index)()) == type_index)
}

#[must_use]
pub fn registered_fragment_type_indices() -> Vec<TypeIndex> {
    let mut type_indices = inventory::iter::<FragmentRegistration>
        .into_iter()
        .filter_map(|entry| fragment_type_index_by_uuid((entry.uuid)()))
        .collect::<Vec<_>>();
    type_indices.sort_unstable();
    type_indices.dedup();
    type_indices
}

#[must_use]
pub fn fragment_name_for_type_index(type_index: impl Into<TypeIndex>) -> Option<&'static str> {
    fragment_registration_by_type_index(type_index).map(|entry| (entry.name)())
}

#[must_use]
pub fn fragment_type_index_by_uuid(uuid: Uuid) -> Option<TypeIndex> {
    inventory::iter::<FragmentRegistration>
        .into_iter()
        .find(|entry| (entry.uuid)() == uuid)
        .map(|entry| TypeIndex::new((entry.type_index)()))
}

/// Consume a registered fragment body by compact type index.
///
/// # Errors
///
/// Returns an error when the type index is unknown or the body decoder fails.
pub fn consume_fragment_contents_by_type_index(
    type_index: impl Into<TypeIndex>,
    rb: &mut ReadBuffer<'_>,
) -> Result<(), MarshalerError> {
    let type_index = type_index.into();
    let registration = fragment_registration_by_type_index(type_index).ok_or(
        MarshalerError::UnknownTypeIndex {
            type_index: type_index.get(),
        },
    )?;
    (registration.consume_contents)(rb)
}

/// Decode a registered fragment body by compact type index.
///
/// # Errors
///
/// Returns an error when the type index is unknown or the body decoder fails.
pub fn decode_fragment_contents_by_type_index(
    type_index: impl Into<TypeIndex>,
    rb: &mut ReadBuffer<'_>,
) -> Result<Box<dyn Fragment>, MarshalerError> {
    let type_index = type_index.into();
    let registration = fragment_registration_by_type_index(type_index).ok_or(
        MarshalerError::UnknownTypeIndex {
            type_index: type_index.get(),
        },
    )?;
    (registration.decode_contents)(rb)
}
