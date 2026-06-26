use crate::hub::{ClientActorHash, GroupIndex, ReplicatedState};
use crate::serialize::ReplicatedFieldHandler;
use crate::types::{Crc32, WallClockTimePoint};
use crate::{EntityRef, Marshaler};
use std::fmt;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Marshaler)]
pub struct FreePlayerCountdown {
    pub time_end: u64,
    pub auto_renew: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Marshaler)]
#[repr(u8)]
pub enum DebugAccountProbationOverride {
    #[default]
    None = 0,
    ForceProbationOn = 1,
    ForceProbationOff = 2,
}

impl DebugAccountProbationOverride {
    #[must_use]
    pub const fn from_value(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::None),
            1 => Some(Self::ForceProbationOn),
            2 => Some(Self::ForceProbationOff),
            _ => None,
        }
    }

    #[must_use]
    pub const fn value(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::ForceProbationOn => "ForceProbationOn",
            Self::ForceProbationOff => "ForceProbationOff",
        }
    }
}

impl fmt::Display for DebugAccountProbationOverride {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<DebugAccountProbationOverride> for u8 {
    fn from(value: DebugAccountProbationOverride) -> Self {
        value.value()
    }
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
    pub session_start_wall_clock_time_point: ReplicatedFieldHandler<WallClockTimePoint>,
    #[replicated_state(group = 1)]
    pub session_start_time_point: ReplicatedFieldHandler<u64>,
    #[replicated_state(group = 1)]
    pub debug_account_probation_override: ReplicatedFieldHandler<DebugAccountProbationOverride>,
    #[replicated_state(group = 1)]
    pub free_player_countdown: ReplicatedFieldHandler<FreePlayerCountdown>,
    #[replicated_state(group = 1)]
    pub entering_store_is_blocked: ReplicatedFieldHandler<bool>,
    #[replicated_state(group = 1)]
    pub is_fresh_start_world: ReplicatedFieldHandler<bool>,
    #[replicated_state(group = 1)]
    pub on_death_respawn_cooldown: ReplicatedFieldHandler<f32>,
    #[replicated_state(group = 1)]
    pub most_recent_pvp_active_switch_time_point: ReplicatedFieldHandler<WallClockTimePoint>,
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
    pub player_backstory: ReplicatedFieldHandler<Crc32>,
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

fn guild_id_equal_even_if_invalid(left: &Uuid, right: &Uuid) -> bool {
    left == right
}

impl Default for PlayerComponentReplicatedState {
    fn default() -> Self {
        let mut state = Self {
            login_match_id: Default::default(),
            src_world_id: Default::default(),
            account_is_locked: Default::default(),
            account_in_probation: Default::default(),
            age_group: Default::default(),
            territory_owner_guild_id: ReplicatedFieldHandler::with_equals(
                guild_id_equal_even_if_invalid,
            ),
            session_start_wall_clock_time_point: Default::default(),
            session_start_time_point: Default::default(),
            debug_account_probation_override: Default::default(),
            free_player_countdown: Default::default(),
            entering_store_is_blocked: Default::default(),
            is_fresh_start_world: Default::default(),
            on_death_respawn_cooldown: Default::default(),
            most_recent_pvp_active_switch_time_point: Default::default(),
            notify_pvp_inactive: Default::default(),
            is_pvp_active_character: Default::default(),
            is_changing_mount: Default::default(),
            is_transmog_station_screen_open: Default::default(),
            is_transmog_screen_open: Default::default(),
            is_in_mount_attachment_mode: Default::default(),
            is_armor_dyeing_open: Default::default(),
            player_backstory: Default::default(),
            character_id: Default::default(),
            character_name: Default::default(),
            home_world_id: Default::default(),
            player_connected: Default::default(),
            looking_through_loadout: Default::default(),
            player_type: Default::default(),
            is_in_store: Default::default(),
            platform_account_id: Default::default(),
            platform_type: Default::default(),
            hub: ReplicatedState::new(),
        };

        let auth_player_group = state.hub.add_filter_group();
        let all_player_group = state.hub.add_filter_group();
        debug_assert_eq!(auth_player_group, Self::AUTH_PLAYER_GROUP);
        debug_assert_eq!(all_player_group, Self::ALL_PLAYER_GROUP);

        state.player_type.set_default_value(Self::PLAYER_TYPE_HUMAN);
        state
            .territory_owner_guild_id
            .set_default_value(Uuid::nil());
        state.player_connected.set_default_value(false);
        state.looking_through_loadout.set_default_value(false);
        state.account_in_probation.set_default_value(true);
        state
            .age_group
            .set_default_value(Self::AGE_GROUP_NUM_AGE_GROUPS);
        state
            .debug_account_probation_override
            .set_default_value(DebugAccountProbationOverride::None);
        state
            .free_player_countdown
            .set_default_value(FreePlayerCountdown::default());
        state.home_world_id.set_default_value(EntityRef::default());
        state.src_world_id.set_default_value(EntityRef::default());
        state.login_match_id.set_default_value(String::new());
        state.notify_pvp_inactive.set_default_value(false);
        state.is_in_store.set_default_value(false);
        state.is_changing_mount.set_default_value(false);
        state
            .is_transmog_station_screen_open
            .set_default_value(false);
        state.is_transmog_screen_open.set_default_value(false);
        state.is_in_mount_attachment_mode.set_default_value(false);
        state.is_armor_dyeing_open.set_default_value(false);

        state
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::ClientActorHash;
    use crate::serialize::{CARRIER_ENDIAN, WriteBuffer};

    #[test]
    fn free_player_countdown_uses_wire_order_and_defaults() {
        let countdown = FreePlayerCountdown::default();
        assert_eq!(countdown.time_end, 0);
        assert!(!countdown.auto_renew);

        let countdown = FreePlayerCountdown {
            time_end: 0x0102_0304_0506_0708,
            auto_renew: true,
        };
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        countdown.marshal(&mut wb);

        assert_eq!(
            wb.as_slice(),
            &[0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x01]
        );
    }

    #[test]
    fn debug_account_probation_override_is_one_byte_named_enum() {
        assert_eq!(
            DebugAccountProbationOverride::default(),
            DebugAccountProbationOverride::None
        );
        assert_eq!(DebugAccountProbationOverride::ForceProbationOn.value(), 1);
        assert_eq!(
            DebugAccountProbationOverride::ForceProbationOff.to_string(),
            "ForceProbationOff"
        );
        assert_eq!(
            DebugAccountProbationOverride::from_value(2),
            Some(DebugAccountProbationOverride::ForceProbationOff)
        );
        assert_eq!(DebugAccountProbationOverride::from_value(3), None);

        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        DebugAccountProbationOverride::ForceProbationOn.marshal(&mut wb);
        assert_eq!(wb.as_slice(), &[0x01]);
    }

    #[test]
    fn replicated_state_default_uses_player_groups_and_field_defaults() {
        let state = PlayerComponentReplicatedState::default();

        assert_eq!(state.hub.num_filter_groups(), 3);
        assert!(state.hub.should_send_to_client(
            ClientActorHash::new(7),
            PlayerComponentReplicatedState::AUTH_PLAYER_GROUP
        ));
        assert_eq!(
            state.territory_owner_guild_id.default_value().copied(),
            Some(Uuid::nil())
        );
        assert!(state.territory_owner_guild_id.equals_policy().is_some());
        assert_eq!(state.player_type.default_value().copied(), Some(0));
        assert_eq!(state.player_connected.default_value().copied(), Some(false));
        assert_eq!(
            state.looking_through_loadout.default_value().copied(),
            Some(false)
        );
        assert_eq!(
            state.account_in_probation.default_value().copied(),
            Some(true)
        );
        assert_eq!(
            state.age_group.default_value().copied(),
            Some(PlayerComponentReplicatedState::AGE_GROUP_NUM_AGE_GROUPS)
        );
        assert_eq!(
            state
                .debug_account_probation_override
                .default_value()
                .copied(),
            Some(DebugAccountProbationOverride::None)
        );
        assert_eq!(
            state.free_player_countdown.default_value().copied(),
            Some(FreePlayerCountdown::default())
        );
        assert_eq!(
            state.login_match_id.default_value().map(String::as_str),
            Some("")
        );
        assert_eq!(
            state.home_world_id.default_value(),
            Some(&EntityRef::default())
        );
        assert_eq!(
            state.src_world_id.default_value(),
            Some(&EntityRef::default())
        );
        assert_eq!(
            state.notify_pvp_inactive.default_value().copied(),
            Some(false)
        );
        assert_eq!(state.is_in_store.default_value().copied(), Some(false));
        assert_eq!(
            state.is_changing_mount.default_value().copied(),
            Some(false)
        );
        assert_eq!(
            state
                .is_transmog_station_screen_open
                .default_value()
                .copied(),
            Some(false)
        );
        assert_eq!(
            state.is_transmog_screen_open.default_value().copied(),
            Some(false)
        );
        assert_eq!(
            state.is_in_mount_attachment_mode.default_value().copied(),
            Some(false)
        );
        assert_eq!(
            state.is_armor_dyeing_open.default_value().copied(),
            Some(false)
        );

        assert!(state.account_is_locked.default_value().is_none());
        assert!(state.character_id.default_value().is_none());
        assert!(state.character_name.default_value().is_none());
    }

    #[test]
    fn account_probation_default_can_be_overridden_to_false() {
        let mut state = PlayerComponentReplicatedState::default();

        state.account_in_probation.set_value(false);

        assert_eq!(state.account_in_probation.value().copied(), Some(false));
        assert!(!state.account_in_probation.is_default_value());
    }

    #[test]
    fn auth_player_helpers_manage_auth_group_whitelist() {
        let mut state = PlayerComponentReplicatedState::default();
        let allowed = ClientActorHash::new(0x1001);
        let denied = ClientActorHash::new(0x1002);

        assert!(state.can_send_auth_player_fields_to(denied));

        state.allow_auth_player_fields_for(allowed);
        assert!(state.can_send_auth_player_fields_to(allowed));
        assert!(!state.can_send_auth_player_fields_to(denied));

        state.allow_auth_player_fields_for(allowed);
        state.revoke_auth_player_fields_for(allowed);
        assert!(state.can_send_auth_player_fields_to(allowed));
        assert!(!state.can_send_auth_player_fields_to(denied));

        state.revoke_auth_player_fields_for(allowed);
        assert!(state.can_send_auth_player_fields_to(denied));

        state.allow_auth_player_fields_for(allowed);
        state.clear_auth_player_field_access();
        assert!(state.can_send_auth_player_fields_to(denied));
    }
}

impl PlayerComponentReplicatedState {
    pub const AUTH_PLAYER_GROUP: GroupIndex = GroupIndex::new(1);
    pub const ALL_PLAYER_GROUP: GroupIndex = GroupIndex::new(2);
    pub const PLAYER_TYPE_HUMAN: u8 = 0;
    pub const AGE_GROUP_NUM_AGE_GROUPS: u8 = 3;

    pub fn allow_auth_player_fields_for(&mut self, client_id: ClientActorHash) {
        self.hub
            .add_client_to_replication_whitelist(client_id, Self::AUTH_PLAYER_GROUP);
    }

    pub fn revoke_auth_player_fields_for(&mut self, client_id: ClientActorHash) {
        self.hub
            .remove_client_from_replication_whitelist(client_id, Self::AUTH_PLAYER_GROUP);
    }

    pub fn clear_auth_player_field_access(&mut self) {
        self.hub
            .clear_replication_whitelist(Self::AUTH_PLAYER_GROUP);
    }

    #[must_use]
    pub fn can_send_auth_player_fields_to(&self, client_id: ClientActorHash) -> bool {
        self.hub
            .should_send_to_client(client_id, Self::AUTH_PLAYER_GROUP)
    }

    pub fn apply_identity_snapshot(&mut self, snapshot: PlayerIdentitySnapshot) {
        self.character_id.set_value(snapshot.character_id);
        self.character_name.set_value(snapshot.character_name);
        self.home_world_id.set_value(snapshot.home_world_id);
        self.player_connected.set_value(snapshot.player_connected);
    }
}
