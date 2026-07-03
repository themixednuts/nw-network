//! Social collection replication for friends and related player lists.

use crate::{az_rtti, replicated_state, type_registry};

use uuid::Uuid;

use crate::serialize::{IndexMap, ReplicatedContainer, ReplicatedFieldHandler};
use crate::states::territory::WarDataValue;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SocialCollectionsSnapshot {
    pub friends: ReplicatedContainer<Vec<String>>,
    pub friend_invites: ReplicatedContainer<Vec<String>>,
    pub social_blocks: ReplicatedContainer<Vec<String>>,
}

#[replicated_state]
#[derive(Debug, Clone, Default)]
#[az_rtti("92FE39A5-1948-4EE4-A49F-B02EA344DC57")]
#[type_registry(4176)]
pub struct SocialReplicatedState {
    #[replicated_state(group = 1)]
    pub war_data: ReplicatedContainer<IndexMap<Uuid, WarDataValue>>,
    #[replicated_state(group = 1)]
    pub daily_war_as_attacker_count: ReplicatedFieldHandler<u8>,
    #[replicated_state(group = 1)]
    pub daily_war_as_defender_count: ReplicatedFieldHandler<u8>,
    #[replicated_state(group = 1)]
    pub last_daily_reset_time: ReplicatedFieldHandler<u64>,
    #[replicated_state(group = 1)]
    pub friends: ReplicatedContainer<Vec<String>>,
    #[replicated_state(group = 1)]
    pub friend_invites: ReplicatedContainer<Vec<String>>,
    #[replicated_state(group = 1)]
    pub social_blocks: ReplicatedContainer<Vec<String>>,
    #[replicated_state(group = 1)]
    pub most_recent_join_character_call: ReplicatedFieldHandler<u64>,
    pub player_title_id: ReplicatedFieldHandler<u32>,
    pub pronoun_type: ReplicatedFieldHandler<u8>,
    pub chatting_state_message_type: ReplicatedFieldHandler<u32>,
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
