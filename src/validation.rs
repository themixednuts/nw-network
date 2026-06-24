//! Capture-oriented validation helpers.

use std::collections::BTreeSet;

use crate::{
    hub::fragment_registration_by_type_index,
    network_schema::{NetworkTypeKind, type_by_type_index},
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
}

impl StateFragmentTypeCoverage {
    #[must_use]
    pub fn is_fully_registered(&self) -> bool {
        self.unknown_type_indices.is_empty()
            && self.non_replicated_state_type_indices.is_empty()
            && self.unregistered_replicated_state_type_indices.is_empty()
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
        if descriptor.kind != NetworkTypeKind::ReplicatedState {
            coverage.non_replicated_state_type_indices.push(type_index);
            continue;
        }
        if fragment_registration_by_type_index(type_index).is_some() {
            coverage
                .registered_replicated_state_type_indices
                .push(type_index);
        } else {
            coverage
                .unregistered_replicated_state_type_indices
                .push(type_index);
        }
    }
    coverage
}
