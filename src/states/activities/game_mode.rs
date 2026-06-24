use std::collections::HashMap;

use glam::Vec2;

use crate::hub::ReplicatedState;
use crate::serialize::{
    Codec, DefaultMarshaler, Marshaler, MarshalerError, ReadBuffer, ReplicatedContainer,
    ReplicatedFieldHandler, ReplicatedMap, VlqU64, WIRE_VEC_CAP, WriteBuffer,
};
use crate::types::{
    AfflictionData, EntityRef, GameModeParticipantStatus,
    RemoteServerFacetRefGameModeParticipantComponentServerFacet,
};

pub type GameModeIndexedByteMap = ReplicatedMap<VlqU64, u8>;
pub type GameModeTimerMap = ReplicatedMap<u32, VlqU64>;
pub type GameModeParticipantFacetRefs =
    ReplicatedMap<VlqU64, RemoteServerFacetRefGameModeParticipantComponentServerFacet>;
pub type GameModeParticipantCharacterIds = ReplicatedMap<VlqU64, EntityRef>;
pub type GameModeRaidIds = ReplicatedMap<VlqU64, u64>;
pub type GameModeParticipantStatuses = ReplicatedContainer<
    HashMap<VlqU64, GameModeParticipantStatus>,
    WIRE_VEC_CAP,
    DefaultMarshaler<VlqU64>,
    GameModeParticipantStatusByte,
>;

#[derive(Debug, Clone, Copy, Default)]
pub struct GameModeParticipantStatusByte;

impl Codec<GameModeParticipantStatus> for GameModeParticipantStatusByte {
    const MARSHAL_SIZE: usize = <u8 as Marshaler>::MARSHAL_SIZE;

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

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, nw_network_derive::Marshaler)]
pub struct GameModeReplicatedEvent {
    pub field_00: u32,
    pub field_08: u64,
    pub field_10: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, nw_network_derive::Marshaler)]
pub struct GameModeMapIcon {
    pub icon_id: u32,
    pub position: Vec2,
}

#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ReplicatedState,
    nw_network_derive::AzRtti,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("78EA6535-BB84-4D6A-A5A3-747AF2F5167C")]
#[type_registry(2343)]
pub struct GameModeReplicatedState {
    pub cur_script_state_id: ReplicatedFieldHandler<u8>,
    pub cur_script_id: ReplicatedFieldHandler<u32>,
    pub spawned_entity_ids_by_spawner_id: ReplicatedMap<u32, u64>,
    pub game_mode_id: ReplicatedFieldHandler<u32>,
    pub game_mode_map_id: ReplicatedFieldHandler<u32>,
    pub participant_facet_refs: GameModeParticipantFacetRefs,
    pub participant_statuses: GameModeParticipantStatuses,
    pub participant_team_indexes: GameModeIndexedByteMap,
    pub participant_character_ids: GameModeParticipantCharacterIds,
    pub raid_ids: GameModeRaidIds,
    pub values: GameModeTimerMap,
    pub synced_timers: GameModeTimerMap,
    pub event1: ReplicatedFieldHandler<GameModeReplicatedEvent>,
    pub event2: ReplicatedFieldHandler<GameModeReplicatedEvent>,
    pub event3: ReplicatedFieldHandler<GameModeReplicatedEvent>,
    pub event4: ReplicatedFieldHandler<GameModeReplicatedEvent>,
    pub event5: ReplicatedFieldHandler<GameModeReplicatedEvent>,
    pub event6: ReplicatedFieldHandler<GameModeReplicatedEvent>,
    pub event7: ReplicatedFieldHandler<GameModeReplicatedEvent>,
    pub event8: ReplicatedFieldHandler<GameModeReplicatedEvent>,
    pub event9: ReplicatedFieldHandler<GameModeReplicatedEvent>,
    pub event10: ReplicatedFieldHandler<GameModeReplicatedEvent>,
    pub global_affliction_data: ReplicatedFieldHandler<AfflictionData>,
    pub map_origin: ReplicatedFieldHandler<u32>,
    pub tile_size_meters: ReplicatedFieldHandler<u8>,
    pub map_size_in_tiles: ReplicatedFieldHandler<u16>,
    pub tile_ui_filename_id_and_rotation: GameModeIndexedByteMap,
    pub tile_visited: GameModeIndexedByteMap,
    pub icons: ReplicatedMap<VlqU64, GameModeMapIcon>,
    pub linked_mode: ReplicatedFieldHandler<bool>,

    pub hub: ReplicatedState,
}

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
