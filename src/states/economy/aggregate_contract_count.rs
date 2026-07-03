//! Aggregate contract contribution counts replicated by project id.

use crate::{az_rtti, replicated_state, type_registry};

use crate::serialize::{IndexMap, ReplicatedContainer, ReplicatedFieldHandler};

#[replicated_state]
#[derive(Debug, Clone, Default)]
#[az_rtti("BF75653B-B1DF-4DBE-840D-0236A69DA247")]
#[type_registry(3791)]
pub struct AggregateContractCountComponentReplicatedState {
    pub total_buy_contracts: ReplicatedFieldHandler<u32>,
    pub total_sell_contracts: ReplicatedFieldHandler<u32>,
    #[replicated_state(group = 1)]
    pub buy_category_counts: ReplicatedContainer<IndexMap<u32, u8>>,
    #[replicated_state(group = 1)]
    pub buy_family_counts: ReplicatedContainer<IndexMap<u32, u8>>,
    #[replicated_state(group = 1)]
    pub buy_group_counts: ReplicatedContainer<IndexMap<u32, u8>>,
    #[replicated_state(group = 1)]
    pub buy_item_counts: ReplicatedContainer<IndexMap<u32, u8>>,
    #[replicated_state(group = 1)]
    pub sell_category_counts: ReplicatedContainer<IndexMap<u32, u8>>,
    #[replicated_state(group = 1)]
    pub sell_family_counts: ReplicatedContainer<IndexMap<u32, u8>>,
    #[replicated_state(group = 1)]
    pub sell_group_counts: ReplicatedContainer<IndexMap<u32, u8>>,
    #[replicated_state(group = 1)]
    pub sell_item_counts: ReplicatedContainer<IndexMap<u32, u8>>,
}
