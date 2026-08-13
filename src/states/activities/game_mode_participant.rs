//! Per-player game-mode participation, queue, raid, and mutation state.

use uuid::Uuid;

use crate::Marshaler;

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Marshaler)]
pub struct GameModeInstanceId {
    pub game_mode_id: Uuid,
    pub field_10: u64,
    pub field_18: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct ActiveGameModeData {
    pub field_00: u32,
    pub field_04: u32,
    pub field_08: u64,
    pub field_10: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct QueuedGameModeData {
    pub field_00: u32,
    pub field_08: u64,
    pub field_10: u32,
    pub field_14: u32,
    pub field_18: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct GameModeMutationContext {
    pub field_00: u32,
    pub field_04: u32,
    pub field_08: u32,
    pub field_0c: u8,
}
pub use crate::generated::states::GameModeParticipantReplicatedState;
