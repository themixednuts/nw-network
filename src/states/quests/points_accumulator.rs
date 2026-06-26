use crate::serialize::ReplicatedFieldHandler;

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("63AD3B5A-3E2E-4923-ACCD-1DA221431EE0")]
#[::nw_network::type_registry(6951)]
pub struct PointsAccumulatorComponentReplicatedState {
    pub num_points0: ReplicatedFieldHandler<u32>,
    pub max_num_points0: ReplicatedFieldHandler<u32>,
    pub time_when_points_zeroed0: ReplicatedFieldHandler<u64>,
}
