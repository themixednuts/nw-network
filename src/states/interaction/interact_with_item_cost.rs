//! Interactable item-cost replication.

use crate::{az_rtti, replicated_state, type_registry};

use crate::serialize::{IndexMap, ReplicatedContainer, ReplicatedFieldHandler};

#[replicated_state]
#[derive(Debug, Clone, Default)]
#[az_rtti("D4090863-57F0-409D-9EAD-DF97633634D0")]
#[type_registry(2930)]
pub struct InteractReplicatedState {
    pub enabled: ReplicatedFieldHandler<bool>,
    pub has_interactors: ReplicatedFieldHandler<u32>,
    pub cooldown_updates: ReplicatedContainer<IndexMap<u32, u64>>,
}
