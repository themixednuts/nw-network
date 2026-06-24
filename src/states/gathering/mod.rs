pub mod component;
pub mod fishing;
pub mod gatherable_controller;

pub use component::GatheringComponentReplicatedState;
pub use fishing::{
    FishingComponentReplicatedState, FishingStateTransition, MAX_FISHING_STATE_TRANSITION_CHANGES,
};
pub use gatherable_controller::{GatherableControllerReplicatedState, ReplicatedGatherableState};
