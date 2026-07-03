//! Interaction and world-trigger replicated state modules.

pub mod door;
pub mod interact_with_item_cost;
pub mod interactor;
pub mod trigger_area_entity;

pub use crate::generated::states::{
    DelayedEventComponentReplicatedState, DetectionVolumeEventReplicatedState,
    EventTimelineComponentReplicatedState,
};
pub use door::{DoorComponentReplicatedState, DoorState};
pub use interact_with_item_cost::InteractReplicatedState;
pub use interactor::InteractorComponentReplicatedState;
pub use trigger_area_entity::TriggerAreaEntityEventTimingsReplicatedState;
