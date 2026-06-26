use uuid::Uuid;

use crate::serialize::{ReplicatedFieldHandler, ReplicatedMap, VlqU64};
use crate::{EntityRef, Marshaler};

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

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("704F90BD-BA5A-4DED-A3DF-DA1827A45E93")]
#[::nw_network::type_registry(3217)]
pub struct GuildsReplicatedState {
    pub guild_id: ReplicatedFieldHandler<Uuid>,
    pub guild_owner: ReplicatedFieldHandler<String>,
    pub guild_crest_data: ReplicatedFieldHandler<GuildCrestData>,
    pub owner_player_identification: ReplicatedFieldHandler<GuildPlayerIdentification>,
    pub current_permission_rule: ReplicatedFieldHandler<u8>,
    pub guild_structure_name: ReplicatedFieldHandler<String>,
    pub raid_id: ReplicatedFieldHandler<u64>,
    pub faction_type: ReplicatedFieldHandler<u8>,
    pub owner_pvp_flag: ReplicatedFieldHandler<bool>,
    pub owner_ffa_flag: ReplicatedFieldHandler<bool>,
    pub permission_options: ReplicatedFieldHandler<u8>,
}

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("F5398BCB-8AF9-47C7-84C6-B8AFBF249F34")]
#[::nw_network::type_registry(3147)]
pub struct GuildsComponentReplicatedState {
    pub guild_id: ReplicatedFieldHandler<Uuid>,
    pub guild_rank: ReplicatedFieldHandler<u16>,
    #[replicated_state(group = 1)]
    pub using_gm_commands: ReplicatedFieldHandler<bool>,
    #[replicated_state(group = 1)]
    pub guild_forced_rename: ReplicatedFieldHandler<bool>,
    #[replicated_state(group = 1)]
    pub guild_message_of_the_day: ReplicatedFieldHandler<String>,
    #[replicated_state(group = 1)]
    pub guild_message_of_the_day_author: ReplicatedFieldHandler<EntityRef>,
    #[replicated_state(group = 1)]
    pub guild_message_of_the_day_time: ReplicatedFieldHandler<u64>,
    #[replicated_state(group = 1)]
    pub guild_siege_window: ReplicatedFieldHandler<f32>,
    #[replicated_state(group = 1)]
    pub guild_last_siege_window_set: ReplicatedFieldHandler<u64>,
    #[replicated_state(group = 1)]
    pub guild_treasury_current_funds: ReplicatedFieldHandler<u64>,
    #[replicated_state(group = 1)]
    pub guild_treasury_daily_withdrawal_limit: ReplicatedFieldHandler<u64>,
    #[replicated_state(group = 1)]
    pub last_guild_leave_time: ReplicatedFieldHandler<u64>,
    #[replicated_state(group = 1)]
    pub guild_treasury_withdrawn_today: ReplicatedFieldHandler<u64>,
    #[replicated_state(group = 1)]
    pub is_guild_master_seat_overthrowable: ReplicatedFieldHandler<bool>,
    #[replicated_state(group = 1)]
    pub is_guild_master_seat_vacant: ReplicatedFieldHandler<bool>,
    #[replicated_state(group = 1)]
    pub governor_leave_guild_time: ReplicatedFieldHandler<u64>,
    #[replicated_state(group = 1)]
    pub guild_members_character_id_string: ReplicatedMap<VlqU64, String>,
    #[replicated_state(group = 1)]
    pub guild_members_rank: ReplicatedMap<VlqU64, u16>,
    #[replicated_state(group = 1)]
    pub guild_members_online_status: ReplicatedMap<VlqU64, u8>,
    #[replicated_state(group = 1)]
    pub guild_members_last_online_time: ReplicatedMap<VlqU64, u64>,
    #[replicated_state(group = 1)]
    pub guild_member_was_kicked: ReplicatedFieldHandler<bool>,
    #[replicated_state(group = 1)]
    pub guild_influence_data: ReplicatedMap<VlqU64, ReplicatedGuildInfluence>,
    #[replicated_state(group = 1)]
    pub eligible_territory_wars: ReplicatedMap<VlqU64, EligibleTerritoryWar>,
    #[replicated_state(group = 1)]
    pub guild_war_lottery_deadline: ReplicatedFieldHandler<u64>,
    #[replicated_state(group = 1)]
    pub guild_invites: ReplicatedMap<Uuid, GuildInviteStateData>,
    #[replicated_state(group = 1)]
    pub number_of_outstanding_invites: ReplicatedFieldHandler<u64>,
}
