use crate::serialize::{ReplicatedFieldHandler, ReplicatedMap};

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("D4090863-57F0-409D-9EAD-DF97633634D0")]
#[::nw_network::type_registry(2930)]
pub struct InteractReplicatedState {
    pub enabled: ReplicatedFieldHandler<bool>,
    pub has_interactors: ReplicatedFieldHandler<u32>,
    pub cooldown_updates: ReplicatedMap<u32, u64>,
}
