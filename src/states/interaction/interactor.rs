//! Active interactor identity and interaction-state replication.

use crate::{az_rtti, replicated_state, type_registry};

use crate::GdeId;
use crate::serialize::ReplicatedFieldHandler;

#[replicated_state]
#[derive(Debug, Clone, Default)]
#[az_rtti("6F49B860-36AA-4583-B426-94CC174B2B9D")]
#[type_registry(3752)]
pub struct InteractorComponentReplicatedState {
    #[replicated_state(group = 1)]
    pub enabled: ReplicatedFieldHandler<bool>,
    pub cached_committed_interact_gdeid: ReplicatedFieldHandler<GdeId>,
}
