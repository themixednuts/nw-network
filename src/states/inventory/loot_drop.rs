use crate::serialize::ReplicatedFieldHandler;

/// Rarity state for a dropped loot container.
#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("66590644-2426-49D4-8DEA-BDB4EEA4E263")]
#[::nw_network::type_registry(2027)]
pub struct LootDropReplicatedState {
    pub rarity_level: ReplicatedFieldHandler<u32>,
}
