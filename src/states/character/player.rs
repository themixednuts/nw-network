use uuid::Uuid;

use crate::hub::ReplicatedState;
use crate::serialize::ReplicatedFieldHandler;
use crate::{EntityRef, Marshaler};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Marshaler)]
pub struct FreePlayerCountdown {
    pub end_time_point: u64,
    pub active: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlayerIdentitySnapshot {
    pub character_id: EntityRef,
    pub character_name: String,
    pub home_world_id: EntityRef,
    pub player_connected: bool,
}

#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ReplicatedState,
    nw_network_derive::AzRtti,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("BDDDA784-A6E7-416B-A041-449920D90FB6")]
#[type_registry(3935)]
pub struct PlayerComponentReplicatedState {
    #[replicated_state(group = 1)]
    pub login_match_id: ReplicatedFieldHandler<String>,
    #[replicated_state(group = 1)]
    pub src_world_id: ReplicatedFieldHandler<EntityRef>,
    #[replicated_state(group = 1)]
    pub account_is_locked: ReplicatedFieldHandler<bool>,
    #[replicated_state(group = 1)]
    pub account_in_probation: ReplicatedFieldHandler<bool>,
    #[replicated_state(group = 1)]
    pub age_group: ReplicatedFieldHandler<u8>,
    #[replicated_state(group = 1)]
    pub territory_owner_guild_id: ReplicatedFieldHandler<Uuid>,
    #[replicated_state(group = 1)]
    pub session_start_wall_clock_time_point: ReplicatedFieldHandler<u64>,
    #[replicated_state(group = 1)]
    pub session_start_time_point: ReplicatedFieldHandler<u64>,
    #[replicated_state(group = 1)]
    pub debug_account_probation_override: ReplicatedFieldHandler<u8>,
    #[replicated_state(group = 1)]
    pub free_player_countdown: ReplicatedFieldHandler<FreePlayerCountdown>,
    #[replicated_state(group = 1)]
    pub entering_store_is_blocked: ReplicatedFieldHandler<bool>,
    #[replicated_state(group = 1)]
    pub is_fresh_start_world: ReplicatedFieldHandler<bool>,
    #[replicated_state(group = 1)]
    pub on_death_respawn_cooldown: ReplicatedFieldHandler<f32>,
    #[replicated_state(group = 1)]
    pub most_recent_pvp_active_switch_time_point: ReplicatedFieldHandler<u64>,
    #[replicated_state(group = 1)]
    pub notify_pvp_inactive: ReplicatedFieldHandler<bool>,
    #[replicated_state(group = 1)]
    pub is_pvp_active_character: ReplicatedFieldHandler<bool>,
    #[replicated_state(group = 1)]
    pub is_changing_mount: ReplicatedFieldHandler<bool>,
    #[replicated_state(group = 1)]
    pub is_transmog_station_screen_open: ReplicatedFieldHandler<bool>,
    #[replicated_state(group = 1)]
    pub is_transmog_screen_open: ReplicatedFieldHandler<bool>,
    #[replicated_state(group = 1)]
    pub is_in_mount_attachment_mode: ReplicatedFieldHandler<bool>,
    #[replicated_state(group = 1)]
    pub is_armor_dyeing_open: ReplicatedFieldHandler<bool>,
    #[replicated_state(group = 1)]
    pub player_backstory: ReplicatedFieldHandler<u32>,
    #[replicated_state(group = 2)]
    pub character_id: ReplicatedFieldHandler<EntityRef>,
    #[replicated_state(group = 2)]
    pub character_name: ReplicatedFieldHandler<String>,
    #[replicated_state(group = 2)]
    pub home_world_id: ReplicatedFieldHandler<EntityRef>,
    #[replicated_state(group = 2)]
    pub player_connected: ReplicatedFieldHandler<bool>,
    #[replicated_state(group = 2)]
    pub looking_through_loadout: ReplicatedFieldHandler<bool>,
    #[replicated_state(group = 2)]
    pub player_type: ReplicatedFieldHandler<u8>,
    #[replicated_state(group = 2)]
    pub is_in_store: ReplicatedFieldHandler<bool>,
    #[replicated_state(group = 2)]
    pub platform_account_id: ReplicatedFieldHandler<u64>,
    #[replicated_state(group = 2)]
    pub platform_type: ReplicatedFieldHandler<u8>,

    pub hub: ReplicatedState,
}

impl PlayerComponentReplicatedState {
    pub fn apply_identity_snapshot(&mut self, snapshot: PlayerIdentitySnapshot) {
        self.character_id.set_value(snapshot.character_id);
        self.character_name.set_value(snapshot.character_name);
        self.home_world_id.set_value(snapshot.home_world_id);
        self.player_connected.set_value(snapshot.player_connected);
    }
}
