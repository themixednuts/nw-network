//! Housing, building, camping, and home-point replicated state modules.

pub mod buildable_controller;
pub mod buildable_grid;
pub mod camping;
pub mod house_data;
pub mod placement_obstruction;
pub mod player_home;
pub mod player_housing;

pub use crate::generated::states::BuilderComponentReplicatedState;
pub use buildable_controller::{BuildableControllerReplicatedState, CommittedResourceValue};
pub use buildable_grid::{
    BuildableGridComponentReplicatedState, BuildableGridSideActive, MAX_BUILDABLE_GRID_SIDE_CHANGES,
};
pub use camping::CampingComponentReplicatedState;
pub use house_data::{HouseDataReplicatedState, HousingItemValue};
pub use placement_obstruction::PlacementObstructionComponentReplicatedState;
pub use player_home::{
    HomePointPersistentRef, HomePointReplicatedState, PlayerHomeComponentReplicatedState,
    PlayerHomeSnapshot,
};
pub use player_housing::ReplicatedOwnedHouseData;
