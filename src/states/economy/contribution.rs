use crate::hub::ReplicatedState;
use crate::serialize::{ReplicatedMap, VlqU64};
use crate::states::inventory::SimpleItemDescriptor;

pub const MAX_CONTRIBUTION_XP_EVENT_CHANGES: usize = 0x3fff;

#[derive(nw_network_derive::Marshaler, Debug, Clone, Default, PartialEq)]
pub struct ContributionXpEvent {
    pub field_08: u64,
    pub field_10: u32,
    pub field_14: f32,
    pub field_18: f32,
    pub field_1c: u32,
    pub field_20: bool,
    pub field_28: u32,
    pub field_2c: u32,
    pub field_30: u32,
    pub field_40: Vec<SimpleItemDescriptor>,
    pub field_60: u8,
    pub field_68: u8,
    pub field_24: u32,
    pub field_38: u16,
    pub field_34: u32,
    pub field_64: u32,
}

#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ReplicatedState,
    nw_network_derive::AzRtti,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("94DDD5E5-80E8-4538-B6C4-AD6747755DC1")]
#[type_registry(1982)]
pub struct ContributionComponentReplicatedState {
    pub xp_events: ReplicatedMap<VlqU64, ContributionXpEvent, MAX_CONTRIBUTION_XP_EVENT_CHANGES>,

    pub hub: ReplicatedState,
}
