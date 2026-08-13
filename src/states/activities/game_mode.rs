//! Game-mode instance replication for timers, participants, events, and map UI.

use crate::{Marshal, Marshaler, Unmarshal};

use glam::Vec2;

use crate::serialize::{
    Codec, DefaultMarshaler, IndexMap, MarshalerError, ReadBuffer, ReplicatedContainer, VlqU64,
    WIRE_VEC_CAP, WriteBuffer,
};
use crate::types::{
    Crc32, EntityRef, GameModeParticipantStatus,
    RemoteServerFacetRefGameModeParticipantComponentServerFacet,
};

pub type GameModeIndexedByteMap = ReplicatedContainer<IndexMap<VlqU64, u8>>;
pub type GameModeTimerMap = ReplicatedContainer<IndexMap<Crc32, VlqU64>>;
pub type GameModeParticipantFacetRefs = ReplicatedContainer<
    IndexMap<VlqU64, RemoteServerFacetRefGameModeParticipantComponentServerFacet>,
>;
pub type GameModeParticipantCharacterIds = ReplicatedContainer<IndexMap<VlqU64, EntityRef>>;
pub type GameModeRaidIds = ReplicatedContainer<IndexMap<VlqU64, u64>>;
pub type GameModeParticipantStatuses = ReplicatedContainer<
    IndexMap<VlqU64, GameModeParticipantStatus>,
    WIRE_VEC_CAP,
    DefaultMarshaler<VlqU64>,
    GameModeParticipantStatusByte,
>;

#[derive(Debug, Clone, Copy, Default)]
pub struct GameModeParticipantStatusByte;

impl Codec<GameModeParticipantStatus> for GameModeParticipantStatusByte {
    const MARSHAL_SIZE: usize = <u8 as Marshal>::MARSHAL_SIZE;

    fn marshal(value: &GameModeParticipantStatus, wb: &mut WriteBuffer) {
        let raw: u8 = match value {
            GameModeParticipantStatus::Active => 0,
            GameModeParticipantStatus::ClientConnected => 1,
            GameModeParticipantStatus::Dead => 2,
            GameModeParticipantStatus::DeathsDoor => 3,
            GameModeParticipantStatus::InCombat => 4,
        };
        raw.marshal(wb);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<GameModeParticipantStatus, MarshalerError> {
        let raw = u8::unmarshal(rb)?;
        GameModeParticipantStatus::try_from(i32::from(raw)).map_err(|_| {
            MarshalerError::InvalidRange {
                value: u64::from(raw),
                min: 0,
                max: 4,
            }
        })
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Marshaler)]
pub struct GameModeReplicatedEvent {
    pub field_00: u32,
    pub field_08: u64,
    pub field_10: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Marshaler)]
pub struct GameModeMapIcon {
    pub icon_id: u32,
    pub position: Vec2,
}
pub use crate::generated::states::GameModeReplicatedState;
#[cfg(test)]
mod tests {
    use super::*;
    use crate::serialize::CARRIER_ENDIAN;

    #[test]
    fn participant_status_codec_uses_one_byte() {
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        GameModeParticipantStatusByte::marshal(&GameModeParticipantStatus::InCombat, &mut wb);
        assert_eq!(wb.as_slice(), &[4]);

        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &[3]);
        let decoded = GameModeParticipantStatusByte::unmarshal(&mut rb).unwrap();
        assert_eq!(decoded, GameModeParticipantStatus::DeathsDoor);
        assert_eq!(rb.left(), 0);
    }
}
