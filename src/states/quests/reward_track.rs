use crate::Marshaler;
use crate::serialize::{ReplicatedFieldHandler, ReplicatedVec};
use crate::states::inventory::ItemTransformItemDescriptor;

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct RolledReward {
    pub reward_id: u32,
    pub quantity: u32,
    pub item: ItemTransformItemDescriptor,
    pub stage: u8,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RewardTrackSnapshot {
    pub rolled_rewards: ReplicatedVec<RolledReward>,
}

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("CCEA2E6C-2C3E-4A7F-97A7-C5CB86167960")]
#[::nw_network::type_registry(4913)]
pub struct RewardTrackComponentReplicatedState {
    #[replicated_state(group = 2)]
    pub rolled_rewards: ReplicatedVec<RolledReward>,
    #[replicated_state(group = 2)]
    pub selected_rewards: ReplicatedVec<u8>,
    #[replicated_state(group = 2)]
    pub debug_track_excluded_tags: ReplicatedVec<u32>,
    #[replicated_state(group = 1)]
    pub pvp_xp_rank: ReplicatedFieldHandler<u16>,
}

impl RewardTrackComponentReplicatedState {
    pub fn apply_snapshot(&mut self, snapshot: RewardTrackSnapshot) {
        self.rolled_rewards = snapshot.rolled_rewards;
    }
}
