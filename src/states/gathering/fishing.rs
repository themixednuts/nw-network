//! Fishing state transition replication.

use crate::{Marshaler, az_rtti, replicated_state, type_registry};

use crate::serialize::{IndexMap, ReplicatedContainer, VlqU64};

pub const MAX_FISHING_STATE_TRANSITION_CHANGES: usize = 0x3fff;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Marshaler)]
pub struct FishingStateTransition {
    pub state: u16,
    pub value: u32,
}

#[replicated_state]
#[derive(Debug, Clone, Default)]
#[az_rtti("4FE321B0-4195-415A-8D37-57E202683785")]
#[type_registry(5338)]
pub struct FishingComponentReplicatedState {
    pub state_transitions: ReplicatedContainer<
        IndexMap<VlqU64, FishingStateTransition>,
        MAX_FISHING_STATE_TRANSITION_CHANGES,
    >,
}
