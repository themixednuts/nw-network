//! Capture-oriented validation helpers.

use std::collections::BTreeSet;

use crate::{
    hub::{TypeIndex, fragment_registration_by_type_index, registered_fragment_type_indices},
    network_schema::{NETWORK_TYPES, NetworkTypeDescriptor, type_by_type_index},
};

/// Coverage buckets for compact state-fragment type indices.
///
/// The buckets are sorted and deduplicated so capture checks produce stable
/// output even when a bundle repeats the same fragment type.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StateFragmentTypeCoverage {
    pub unknown_type_indices: Vec<u32>,
    pub non_replicated_state_type_indices: Vec<u32>,
    pub unregistered_replicated_state_type_indices: Vec<u32>,
    pub registered_replicated_state_type_indices: Vec<u32>,
    pub field_shape_incomplete_replicated_state_type_indices: Vec<u32>,
    pub generation_ready_unregistered_replicated_state_type_indices: Vec<u32>,
}

impl StateFragmentTypeCoverage {
    #[must_use]
    pub fn is_fully_registered(&self) -> bool {
        self.unknown_type_indices.is_empty()
            && self.non_replicated_state_type_indices.is_empty()
            && self.unregistered_replicated_state_type_indices.is_empty()
    }

    #[must_use]
    pub fn has_complete_field_shapes(&self) -> bool {
        self.field_shape_incomplete_replicated_state_type_indices
            .is_empty()
    }

    #[must_use]
    pub fn is_fully_supported(&self) -> bool {
        self.is_fully_registered() && self.has_complete_field_shapes()
    }
}

#[must_use]
pub fn validate_state_fragment_type_indices(
    type_indices: impl IntoIterator<Item = u32>,
) -> StateFragmentTypeCoverage {
    let mut coverage = StateFragmentTypeCoverage::default();
    for type_index in BTreeSet::from_iter(type_indices) {
        let Some(descriptor) = type_by_type_index(type_index) else {
            coverage.unknown_type_indices.push(type_index);
            continue;
        };
        if !descriptor.is_replicated_state() {
            coverage.non_replicated_state_type_indices.push(type_index);
            continue;
        }
        let has_complete_shapes =
            descriptor.has_complete_field_wire_shapes() && !descriptor.fields.is_empty();
        if !has_complete_shapes {
            coverage
                .field_shape_incomplete_replicated_state_type_indices
                .push(type_index);
        }
        if fragment_registration_by_type_index(type_index).is_some() {
            coverage
                .registered_replicated_state_type_indices
                .push(type_index);
        } else {
            coverage
                .unregistered_replicated_state_type_indices
                .push(type_index);
            if has_complete_shapes {
                coverage
                    .generation_ready_unregistered_replicated_state_type_indices
                    .push(type_index);
            }
        }
    }
    coverage
}

/// Schema and decoder coverage for one replicated-state type index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplicatedStatePortStatus {
    /// Runtime type index used in packet fragments.
    pub type_index: u32,
    /// Best recovered type name, when the static schema has one.
    pub name: Option<&'static str>,
    /// Whether this crate has a registered fragment implementation.
    pub is_registered: bool,
    /// Number of registered fields recovered into the static schema.
    pub field_count: usize,
    /// Number of recovered fields whose wire shape is still unknown.
    pub missing_field_wire_shape_count: usize,
}

impl ReplicatedStatePortStatus {
    #[must_use]
    pub const fn has_complete_field_shapes(&self) -> bool {
        self.missing_field_wire_shape_count == 0 && self.field_count != 0
    }

    #[must_use]
    pub const fn can_generate_state_fields(&self) -> bool {
        !self.is_registered && self.has_complete_field_shapes()
    }
}

#[must_use]
pub fn replicated_state_port_statuses() -> Vec<ReplicatedStatePortStatus> {
    let registered = BTreeSet::from_iter(registered_fragment_type_indices());
    NETWORK_TYPES
        .iter()
        .filter(|descriptor| descriptor.is_replicated_state())
        .map(|descriptor| replicated_state_port_status(descriptor, &registered))
        .collect()
}

fn replicated_state_port_status(
    descriptor: &NetworkTypeDescriptor,
    registered: &BTreeSet<TypeIndex>,
) -> ReplicatedStatePortStatus {
    ReplicatedStatePortStatus {
        type_index: descriptor.type_index,
        name: descriptor.name,
        is_registered: registered.contains(&TypeIndex::new(descriptor.type_index)),
        field_count: descriptor.fields.len(),
        missing_field_wire_shape_count: descriptor.missing_field_wire_shape_count(),
    }
}
