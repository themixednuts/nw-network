//! Crafting recipe cooldown and gear-score bonus replication.
use crate::serialize::marshaler::{Marshal, Unmarshal};

use crate::{az_rtti, replicated_state, type_registry};

use crate::serialize::{IndexMap, MarshalerError, ReadBuffer, ReplicatedContainer, WriteBuffer};
use crate::types::{Crc32, RecipeCooldownData, WallClockTimePoint};

pub const MAX_CRAFTING_RECIPE_COOLDOWNS: usize = 0x1d;
pub const MAX_CRAFTING_GS_BONUSES: usize = 7;

impl Marshal for RecipeCooldownData {
    const MARSHAL_SIZE: usize =
        <u8 as Marshal>::MARSHAL_SIZE + <WallClockTimePoint as Marshal>::MARSHAL_SIZE;

    fn marshal(&self, wb: &mut WriteBuffer) {
        self.count.marshal(wb);
        self.cooldown_end.marshal(wb);
    }
}

impl Unmarshal for RecipeCooldownData {
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        Ok(Self {
            count: u8::unmarshal(rb)?,
            cooldown_end: WallClockTimePoint::unmarshal(rb)?,
        })
    }
}

#[replicated_state]
#[derive(Debug, Clone, Default)]
#[az_rtti("FD24C20B-FB95-49F8-9BB0-DEC472F0B6EA")]
#[type_registry(205)]
pub struct CraftingComponentReplicatedState {
    pub cooldowns:
        ReplicatedContainer<IndexMap<Crc32, RecipeCooldownData>, MAX_CRAFTING_RECIPE_COOLDOWNS>,
    pub craft_gs_bonuses: ReplicatedContainer<IndexMap<u8, u16>, MAX_CRAFTING_GS_BONUSES>,
}
