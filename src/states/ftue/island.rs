//! FTUE island phase and completion-state replication.

use crate::{az_rtti, replicated_state, type_registry};

use crate::serialize::ReplicatedFieldHandler;

#[replicated_state]
#[derive(Debug, Clone, Default)]
#[az_rtti("C2A97536-74E8-46F2-A372-9D16BF072B6A")]
#[type_registry(1055)]
pub struct FtueIslandComponentReplicatedState {
    pub player_entered_trigger: ReplicatedFieldHandler<bool>,
}
