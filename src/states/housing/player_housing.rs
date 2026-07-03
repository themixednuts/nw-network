//! Player-housing replicated value helpers.

use glam::Vec3;

use crate::Marshaler;
use crate::types::{
    RemoteServerFacetRefHousingPlotComponentServerFacet, RemoteServerGdeRef, WallClockTimePoint,
};

/// Owned-house value carried by `PlayerHousingReplicatedState::owned_houses`.
#[derive(Debug, Clone, Default, PartialEq, Marshaler)]
pub struct ReplicatedOwnedHouseData {
    pub housing_plot_remote_ref: RemoteServerFacetRefHousingPlotComponentServerFacet,
    pub house_data_remote_ref: RemoteServerGdeRef,
    pub position: Vec3,
    pub value_a: WallClockTimePoint,
    pub value_b: WallClockTimePoint,
    pub value_c: u16,
    pub flag_a: bool,
    pub values: Vec<u32>,
    pub value_d: f32,
    pub flag_b: bool,
    pub flag_c: bool,
    pub value_e: WallClockTimePoint,
    pub value_f: u32,
}
