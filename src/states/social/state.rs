use crate::hub::ReplicatedState;
use crate::serialize::{ReplicatedFieldHandler, ReplicatedVec};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SocialCollectionsSnapshot {
    pub friends: ReplicatedVec<String>,
    pub friend_invites: ReplicatedVec<String>,
    pub social_blocks: ReplicatedVec<String>,
}

#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ReplicatedState,
    nw_network_derive::AzRtti,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("92FE39A5-1948-4EE4-A49F-B02EA344DC57")]
#[type_registry(4176)]
pub struct SocialReplicatedState {
    #[replicated_state(group = 1)]
    pub war_data: ReplicatedFieldHandler<Vec<u32>>,
    #[replicated_state(group = 1)]
    pub daily_war_as_attacker_count: ReplicatedFieldHandler<u8>,
    #[replicated_state(group = 1)]
    pub daily_war_as_defender_count: ReplicatedFieldHandler<u8>,
    #[replicated_state(group = 1)]
    pub last_daily_reset_time: ReplicatedFieldHandler<u64>,
    #[replicated_state(group = 1)]
    pub friends: ReplicatedVec<String>,
    #[replicated_state(group = 1)]
    pub friend_invites: ReplicatedVec<String>,
    #[replicated_state(group = 1)]
    pub social_blocks: ReplicatedVec<String>,
    #[replicated_state(group = 1)]
    pub most_recent_join_character_call: ReplicatedFieldHandler<u64>,
    pub player_title_id: ReplicatedFieldHandler<u32>,
    pub pronoun_type: ReplicatedFieldHandler<u8>,
    pub chatting_state_message_type: ReplicatedFieldHandler<u32>,

    pub hub: ReplicatedState,
}

impl SocialReplicatedState {
    pub fn apply_collections(&mut self, snapshot: SocialCollectionsSnapshot) {
        if snapshot.friends.has_value() {
            self.friends = snapshot.friends;
        }
        if snapshot.friend_invites.has_value() {
            self.friend_invites = snapshot.friend_invites;
        }
        if snapshot.social_blocks.has_value() {
            self.social_blocks = snapshot.social_blocks;
        }
    }
}
