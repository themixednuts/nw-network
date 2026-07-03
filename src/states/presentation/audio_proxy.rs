//! Audio proxy switch and obstruction replication.

use crate::{az_rtti, replicated_state, type_registry};

use crate::serialize::ReplicatedFieldHandler;

#[replicated_state]
#[derive(Debug, Clone, Default)]
#[az_rtti("27830472-00A7-4204-A0BC-D1179F6EE4A2")]
#[type_registry(603)]
pub struct AudioProxyComponentReplicatedState {
    pub script_list_for_joints: ReplicatedFieldHandler<Vec<u32>>,
}
