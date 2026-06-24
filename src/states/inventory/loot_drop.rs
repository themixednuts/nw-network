use crate::hub::ReplicatedState;
use crate::serialize::ReplicatedFieldHandler;

/// Rarity state for a dropped loot container.
#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ReplicatedState,
    nw_network_derive::AzRtti,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("66590644-2426-49D4-8DEA-BDB4EEA4E263")]
#[type_registry(2027)]
pub struct LootDropReplicatedState {
    pub rarity_level: ReplicatedFieldHandler<u32>,

    pub hub: ReplicatedState,
}
