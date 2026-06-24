use crate::hub::ReplicatedState;
use crate::serialize::ReplicatedMap;
use crate::types::{Crc32, WallClockTimePoint};

pub const MAX_CRAFTING_RECIPE_COOLDOWNS: usize = 0x1d;
pub const MAX_CRAFTING_GS_BONUSES: usize = 7;

/// Generated network value shape.
#[derive(nw_network_derive::Marshaler, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RecipeCooldownData {
    pub count: u8,
    pub cooldown_end: WallClockTimePoint,
}

#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ReplicatedState,
    nw_network_derive::AzRtti,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("FD24C20B-FB95-49F8-9BB0-DEC472F0B6EA")]
#[type_registry(205)]
pub struct CraftingComponentReplicatedState {
    pub cooldowns: ReplicatedMap<Crc32, RecipeCooldownData, MAX_CRAFTING_RECIPE_COOLDOWNS>,
    pub craft_gs_bonuses: ReplicatedMap<u8, u16, MAX_CRAFTING_GS_BONUSES>,

    pub hub: ReplicatedState,
}
