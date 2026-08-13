//! Social collection replication for friends and related player lists.

use crate::serialize::ReplicatedContainer;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SocialCollectionsSnapshot {
    pub friends: ReplicatedContainer<Vec<String>>,
    pub friend_invites: ReplicatedContainer<Vec<String>>,
    pub social_blocks: ReplicatedContainer<Vec<String>>,
}
pub use crate::generated::states::SocialReplicatedState;
