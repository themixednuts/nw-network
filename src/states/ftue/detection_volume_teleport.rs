use crate::hub::ReplicatedState;
use crate::serialize::{ReplicatedFieldHandler, VlqU32Marshaler};
use crate::types::RemoteServerGdeRef;

#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ReplicatedState,
    nw_network_derive::AzRtti,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("59B9807C-CE02-4611-81A3-F0DDCAC27900")]
#[type_registry(3194)]
pub struct FtueDetectionVolumeTeleportReplicatedState {
    pub player_gde: ReplicatedFieldHandler<RemoteServerGdeRef>,
    pub region_x: ReplicatedFieldHandler<u32, VlqU32Marshaler>,
    pub region_y: ReplicatedFieldHandler<u32, VlqU32Marshaler>,
    pub region_size: ReplicatedFieldHandler<u32, VlqU32Marshaler>,

    pub hub: ReplicatedState,
}
