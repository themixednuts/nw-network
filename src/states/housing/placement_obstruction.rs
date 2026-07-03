//! Placement obstruction state replication.

use crate::{az_rtti, replicated_state, type_registry};

use crate::serialize::ReplicatedFieldHandler;

/// Replicated placement-completion obstruction state.
#[replicated_state]
#[derive(Debug, Clone, Default)]
#[az_rtti("EAE37835-E282-46CC-99A0-81C6BC53CEEA")]
#[type_registry(2187)]
pub struct PlacementObstructionComponentReplicatedState {
    pub has_completion_obstruction: ReplicatedFieldHandler<bool>,
}
