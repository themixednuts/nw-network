use crate::hub::ReplicatedState;
use crate::serialize::{ReplicatedFieldHandler, ReplicatedMap};

#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ReplicatedState,
    nw_network_derive::AzRtti,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("D4090863-57F0-409D-9EAD-DF97633634D0")]
#[type_registry(2930)]
pub struct InteractReplicatedState {
    pub enabled: ReplicatedFieldHandler<bool>,
    pub has_interactors: ReplicatedFieldHandler<u32>,
    pub cooldown_updates: ReplicatedMap<u32, u64>,

    pub hub: ReplicatedState,
}
