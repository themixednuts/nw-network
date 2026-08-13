//! Settlement or activity contribution progress and XP-event replication.

use crate::Marshaler;

use crate::states::inventory::SimpleItemDescriptor;

pub const MAX_CONTRIBUTION_XP_EVENT_CHANGES: usize = 0x3fff;

#[derive(Marshaler, Debug, Clone, Default, PartialEq)]
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
pub use crate::generated::states::ContributionComponentReplicatedState;
