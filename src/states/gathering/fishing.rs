use crate::hub::ReplicatedState;
use crate::serialize::{ReplicatedMap, VlqU64};

pub const MAX_FISHING_STATE_TRANSITION_CHANGES: usize = 0x3fff;

/// Generated network value shape.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, nw_network_derive::Marshaler)]
pub struct FishingStateTransition {
    pub state: u16,
    pub value: u32,
}

#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ReplicatedState,
    nw_network_derive::AzRtti,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("4FE321B0-4195-415A-8D37-57E202683785")]
#[type_registry(5338)]
pub struct FishingComponentReplicatedState {
    pub state_transitions:
        ReplicatedMap<VlqU64, FishingStateTransition, MAX_FISHING_STATE_TRANSITION_CHANGES>,

    pub hub: ReplicatedState,
}
