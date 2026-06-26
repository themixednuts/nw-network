use crate::serialize::{Marshaler, MarshalerError, ReadBuffer, ReplicatedMap, WriteBuffer};
use crate::types::{Crc32, RecipeCooldownData, WallClockTimePoint};

pub const MAX_CRAFTING_RECIPE_COOLDOWNS: usize = 0x1d;
pub const MAX_CRAFTING_GS_BONUSES: usize = 7;

impl Marshaler for RecipeCooldownData {
    const MARSHAL_SIZE: usize =
        <u8 as Marshaler>::MARSHAL_SIZE + <WallClockTimePoint as Marshaler>::MARSHAL_SIZE;

    fn marshal(&self, wb: &mut WriteBuffer) {
        self.count.marshal(wb);
        self.cooldown_end.marshal(wb);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        Ok(Self {
            count: u8::unmarshal(rb)?,
            cooldown_end: WallClockTimePoint::unmarshal(rb)?,
        })
    }
}

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("FD24C20B-FB95-49F8-9BB0-DEC472F0B6EA")]
#[::nw_network::type_registry(205)]
pub struct CraftingComponentReplicatedState {
    pub cooldowns: ReplicatedMap<Crc32, RecipeCooldownData, MAX_CRAFTING_RECIPE_COOLDOWNS>,
    pub craft_gs_bonuses: ReplicatedMap<u8, u16, MAX_CRAFTING_GS_BONUSES>,
}
