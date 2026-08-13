//! Crafting recipe cooldown and gear-score bonus replication.
use crate::serialize::marshaler::{Marshal, Unmarshal};

use crate::serialize::{MarshalerError, ReadBuffer, WriteBuffer};
use crate::types::{RecipeCooldownData, WallClockTimePoint};

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
pub use crate::generated::states::CraftingComponentReplicatedState;
