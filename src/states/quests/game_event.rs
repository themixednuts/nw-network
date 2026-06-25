use arrayvec::ArrayVec;

use crate::Marshaler;
use crate::hub::ReplicatedState;
use crate::serialize::ReplicatedVec;

#[derive(Debug, Clone, Default, PartialEq, Marshaler)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "wire shape keeps independent bool fields in order"
)]
pub struct GameEventSubEntry {
    pub category_id: u32,
    pub rank: u16,
    pub value: u32,
    pub enabled: bool,
    pub score: u32,
    pub threshold: u32,
    pub claimed: bool,
    pub visible: bool,
    pub primary_tags: ArrayVec<u32, 5>,
    pub secondary_tags: ArrayVec<u32, 5>,
    pub unlocked: bool,
    pub reward_ids: ArrayVec<u32, 3>,
}

#[derive(Debug, Clone, Default, PartialEq, Marshaler)]
pub struct GameEventData {
    pub event_id: u64,
    pub phase_id: u32,
    pub start_offset: f32,
    pub end_offset: f32,
    pub state: u32,
    pub active: bool,
    pub score: u32,
    pub rank: u32,
    pub objective_id: u32,
    pub sub_events: Vec<GameEventSubEntry>,
    pub tier: u8,
    pub category: u8,
    pub reward_id: u32,
    pub difficulty: u16,
    pub completion_count: u32,
    pub flags: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct DailyBonusUsed {
    pub bonus_id: u32,
    pub state: u8,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct GameEventSnapshot {
    pub game_events_sequence: u64,
    pub daily_bonuses_used_sequence: u64,
    pub game_events: Vec<GameEventData>,
    pub daily_bonuses_used: Vec<DailyBonusUsed>,
}

#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ReplicatedState,
    nw_network_derive::AzRtti,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("8B6ADD1E-927D-42BF-B2C7-C9D1665AB82D")]
#[type_registry(497)]
pub struct GameEventComponentReplicatedState {
    pub game_events: ReplicatedVec<GameEventData, 10>,
    pub daily_bonuses_used: ReplicatedVec<DailyBonusUsed, 5>,

    pub hub: ReplicatedState,
}

impl GameEventComponentReplicatedState {
    pub fn apply_snapshot(&mut self, snapshot: GameEventSnapshot) {
        self.game_events = ReplicatedVec::new(snapshot.game_events_sequence, snapshot.game_events);
        self.daily_bonuses_used = ReplicatedVec::new(
            snapshot.daily_bonuses_used_sequence,
            snapshot.daily_bonuses_used,
        );
    }
}
