use crate::serialize::{ReplicatedFieldHandler, VlqU32Marshaler};
use crate::types::RemoteServerGdeRef;

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("59B9807C-CE02-4611-81A3-F0DDCAC27900")]
#[::nw_network::type_registry(3194)]
pub struct FtueDetectionVolumeTeleportReplicatedState {
    pub player_gde: ReplicatedFieldHandler<RemoteServerGdeRef>,
    pub region_x: ReplicatedFieldHandler<u32, VlqU32Marshaler>,
    pub region_y: ReplicatedFieldHandler<u32, VlqU32Marshaler>,
    pub region_size: ReplicatedFieldHandler<u32, VlqU32Marshaler>,
}
