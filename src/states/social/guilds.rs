//! Guild roster, invite, crest, and territory influence replication.

use crate::Marshaler;

#[derive(Debug, Clone, Copy, Default, PartialEq, Marshaler)]
pub struct GuildCrestColor {
    pub rgba: [f32; 4],
}

#[derive(Debug, Clone, Default, PartialEq, Marshaler)]
pub struct GuildCrestData {
    pub foreground_id: u16,
    pub foreground_color: GuildCrestColor,
    pub background_id: u16,
    pub background_color: GuildCrestColor,
}

#[derive(Debug, Clone, Default, PartialEq, Marshaler)]
pub struct GuildPlayerIdentification {
    pub player_name: String,
    pub display_name: String,
    pub status: u8,
}

#[derive(Debug, Clone, Default, PartialEq, Marshaler)]
pub struct ReplicatedGuildInfluence {
    pub territory_id: u16,
    pub influence: f32,
    pub next_decay_time: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct EligibleTerritoryWar {
    pub war_id: u64,
    pub war_state: u8,
    pub territory_id: u16,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct GuildInviteSenderData {
    pub rank: u32,
    pub display_name: String,
}

#[derive(Debug, Clone, Default, PartialEq, Marshaler)]
pub struct GuildInviteStateData {
    pub guild_name: String,
    pub guild_crest_data: GuildCrestData,
    pub sender_name: String,
    pub sender_data: GuildInviteSenderData,
    pub sent_time: u64,
}

pub use crate::generated::states::GuildsComponentReplicatedState;
