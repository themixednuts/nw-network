use glam::Vec3;

use crate::hub::ReplicatedState;
use crate::serialize::ReplicatedFieldHandler;

#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ReplicatedState,
    nw_network_derive::AzRtti,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("B84EBDAF-8450-4C65-B345-3B02B892F05C")]
#[type_registry(4321)]
pub struct WaypointsComponentReplicatedState {
    pub replicated_waypoint_position: ReplicatedFieldHandler<Vec3>,

    pub hub: ReplicatedState,
}

impl WaypointsComponentReplicatedState {
    #[must_use]
    pub fn with_position(position: Vec3) -> Self {
        let mut state = Self::default();
        state.replicated_waypoint_position.set_value(position);
        state
    }
}
