//! Base fragment visibility and hide-level replication.

use crate::{az_rtti, replicated_state, type_registry};

use crate::serialize::ReplicatedFieldHandler;

#[replicated_state]
#[derive(Debug, Clone, Default)]
#[az_rtti("1774C5D5-C7E0-4E52-A9A1-3816DF9E25DA")]
#[type_registry(100)]
pub struct HubIFragmentReplicatedState {
    #[replicated_state(name = "replicatedHideLevel")]
    pub replicated_hide_level: ReplicatedFieldHandler<u32>,
}
