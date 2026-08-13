//! Seasonal reward, task, and tracked-stat replication.

use crate::serialize::ReplicatedContainer;

pub type SeasonsRewardsTaskIds = Vec<u32>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SeasonsRewardsSnapshot {
    pub card_template: u8,
    pub claimed_tasks: ReplicatedContainer<Vec<u16>>,
    pub stamped_squares: ReplicatedContainer<Vec<u16>>,
    pub wild_stamped_squares: ReplicatedContainer<Vec<u16>>,
    pub reward_claimed: bool,
    pub reroll_count: u8,
    pub activities_tasks: Vec<u32>,
    pub card_count: u16,
    pub wild_stamp_count: u16,
    pub wild_stamp_award_bound: u64,
    pub wild_stamp_awards_this_session: u8,
    pub wild_stamp_award_remaining: u16,
    pub is_initialized: bool,
    pub season_ids: ReplicatedContainer<Vec<u32>>,
    pub season_bitmask_count: ReplicatedContainer<Vec<u8>>,
    pub season_xp_by_season: ReplicatedContainer<Vec<u64>>,
    pub redeem_bitmask: ReplicatedContainer<Vec<u64>>,
    pub escrow_bitmask: ReplicatedContainer<Vec<u64>>,
    pub foreign_escrow_bitmask: ReplicatedContainer<Vec<u64>>,
    pub first_character_connect_time: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SeasonsRewardsStatsUpdateSnapshot {
    pub initialized: bool,
    pub group_list: ReplicatedContainer<Vec<u32>>,
    pub group_count_list: ReplicatedContainer<Vec<u16>>,
    pub group_stat_index: ReplicatedContainer<Vec<u16>>,
    pub group_stat_value: ReplicatedContainer<Vec<u32>>,
}

pub use crate::generated::states::SeasonsRewardsReplicatedState;
pub use crate::generated::states::SeasonsRewardsTrackedStatReplicatedState;
