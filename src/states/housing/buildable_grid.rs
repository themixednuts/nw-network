use crate::hub::ReplicatedState;
use crate::serialize::{ReplicatedMap, VlqU64};

pub const MAX_BUILDABLE_GRID_SIDE_CHANGES: usize = 0x3fff;

/// Generated network value shape.
#[derive(nw_network_derive::Marshaler, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BuildableGridSideActive {
    pub side0: u8,
    pub side1: u8,
    pub active: bool,
}

#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ReplicatedState,
    nw_network_derive::AzRtti,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("FFABCADB-4B64-41C2-B159-A3A6980F44D0")]
#[type_registry(2134)]
pub struct BuildableGridComponentReplicatedState {
    pub grid_sides_active:
        ReplicatedMap<VlqU64, BuildableGridSideActive, MAX_BUILDABLE_GRID_SIDE_CHANGES>,

    pub hub: ReplicatedState,
}
