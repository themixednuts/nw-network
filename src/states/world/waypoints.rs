use glam::Vec3;

use crate::serialize::ReplicatedFieldHandler;

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("B84EBDAF-8450-4C65-B345-3B02B892F05C")]
#[::nw_network::type_registry(4321)]
pub struct WaypointsComponentReplicatedState {
    pub replicated_waypoint_position: ReplicatedFieldHandler<Vec3>,
}

impl WaypointsComponentReplicatedState {
    #[must_use]
    pub fn with_position(position: Vec3) -> Self {
        let mut state = Self::default();
        state.replicated_waypoint_position.set_value(position);
        state
    }
}
