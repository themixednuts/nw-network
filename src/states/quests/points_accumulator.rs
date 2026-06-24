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
#[az_rtti("63AD3B5A-3E2E-4923-ACCD-1DA221431EE0")]
#[type_registry(6951)]
pub struct PointsAccumulatorComponentReplicatedState {
    pub num_points0: ReplicatedFieldHandler<u32>,
    pub max_num_points0: ReplicatedFieldHandler<u32>,
    pub time_when_points_zeroed0: ReplicatedFieldHandler<u64>,

    pub hub: ReplicatedState,
}
