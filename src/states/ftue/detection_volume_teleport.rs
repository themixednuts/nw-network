//! FTUE detection-volume teleport target replication.

use crate::{az_rtti, replicated_state, type_registry};

use crate::serialize::{ReplicatedFieldHandler, VlqU32Marshaler};
use crate::types::RemoteServerGdeRef;

#[replicated_state]
#[derive(Debug, Clone, Default)]
#[az_rtti("59B9807C-CE02-4611-81A3-F0DDCAC27900")]
#[type_registry(3194)]
pub struct FtueDetectionVolumeTeleportReplicatedState {
    pub player_gde: ReplicatedFieldHandler<RemoteServerGdeRef>,
    pub region_x: ReplicatedFieldHandler<u32, VlqU32Marshaler>,
    pub region_y: ReplicatedFieldHandler<u32, VlqU32Marshaler>,
    pub region_size: ReplicatedFieldHandler<u32, VlqU32Marshaler>,
}
