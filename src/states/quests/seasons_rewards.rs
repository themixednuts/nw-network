use crate::serialize::{ReplicatedFieldHandler, ReplicatedVec};

pub type SeasonsRewardsTaskIds = Vec<u32>;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SeasonsRewardsSnapshot {
    pub card_template: u8,
    pub claimed_tasks: ReplicatedVec<u16>,
    pub stamped_squares: ReplicatedVec<u16>,
    pub wild_stamped_squares: ReplicatedVec<u16>,
    pub reward_claimed: bool,
    pub reroll_count: u8,
    pub activities_tasks: Vec<u32>,
    pub card_count: u16,
    pub wild_stamp_count: u16,
    pub wild_stamp_award_bound: u64,
    pub wild_stamp_awards_this_session: u8,
    pub wild_stamp_award_remaining: u16,
    pub is_initialized: bool,
    pub season_ids: ReplicatedVec<u32>,
    pub season_bitmask_count: ReplicatedVec<u8>,
    pub season_xp_by_season: ReplicatedVec<u64>,
    pub redeem_bitmask: ReplicatedVec<u64>,
    pub escrow_bitmask: ReplicatedVec<u64>,
    pub foreign_escrow_bitmask: ReplicatedVec<u64>,
    pub first_character_connect_time: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SeasonsRewardsStatsUpdateSnapshot {
    pub initialized: bool,
    pub group_list: ReplicatedVec<u32>,
    pub group_count_list: ReplicatedVec<u16>,
    pub group_stat_index: ReplicatedVec<u16>,
    pub group_stat_value: ReplicatedVec<u32>,
}

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("6F5CFE10-D60C-43C7-9C7A-2050B66DFABE")]
#[::nw_network::type_registry(5606)]
pub struct SeasonsRewardsStatsUpdateReplicatedState {
    pub initialized: ReplicatedFieldHandler<bool>,
    pub group_list: ReplicatedVec<u32>,
    pub group_count_list: ReplicatedVec<u16>,
    pub group_stat_index: ReplicatedVec<u16>,
    pub group_stat_value: ReplicatedVec<u32>,
}

impl SeasonsRewardsStatsUpdateReplicatedState {
    pub fn apply_snapshot(&mut self, snapshot: SeasonsRewardsStatsUpdateSnapshot) {
        self.initialized.set_value(snapshot.initialized);
        self.group_list = snapshot.group_list;
        self.group_count_list = snapshot.group_count_list;
        self.group_stat_index = snapshot.group_stat_index;
        self.group_stat_value = snapshot.group_stat_value;
    }
}

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("A18C7B82-DE1E-4BCA-9852-A6F1372FBFF7")]
#[::nw_network::type_registry(5485)]
pub struct SeasonsRewardsReplicatedState {
    pub card_template: ReplicatedFieldHandler<u8>,
    pub claimed_tasks: ReplicatedVec<u16>,
    pub stamped_squares: ReplicatedVec<u16>,
    pub wild_stamped_squares: ReplicatedVec<u16>,
    pub reward_claimed: ReplicatedFieldHandler<bool>,
    pub reroll_count: ReplicatedFieldHandler<u8>,
    pub activities_tasks: ReplicatedFieldHandler<SeasonsRewardsTaskIds>,
    pub card_count: ReplicatedFieldHandler<u16>,
    pub wild_stamp_count: ReplicatedFieldHandler<u16>,
    pub wild_stamp_award_bound: ReplicatedFieldHandler<u64>,
    pub wild_stamp_awards_this_session: ReplicatedFieldHandler<u8>,
    pub wild_stamp_award_remaining: ReplicatedFieldHandler<u16>,
    pub is_initialized: ReplicatedFieldHandler<bool>,
    pub season_ids: ReplicatedVec<u32>,
    pub season_bitmask_count: ReplicatedVec<u8>,
    pub season_xp_by_season: ReplicatedVec<u64>,
    pub redeem_bitmask: ReplicatedVec<u64>,
    pub escrow_bitmask: ReplicatedVec<u64>,
    pub foreign_escrow_bitmask: ReplicatedVec<u64>,
    pub first_character_connect_time: ReplicatedFieldHandler<u64>,
}

impl SeasonsRewardsReplicatedState {
    pub fn apply_snapshot(&mut self, snapshot: SeasonsRewardsSnapshot) {
        self.card_template.set_value(snapshot.card_template);
        self.claimed_tasks = snapshot.claimed_tasks;
        self.stamped_squares = snapshot.stamped_squares;
        self.wild_stamped_squares = snapshot.wild_stamped_squares;
        self.reward_claimed.set_value(snapshot.reward_claimed);
        self.reroll_count.set_value(snapshot.reroll_count);
        self.activities_tasks.set_value(snapshot.activities_tasks);
        self.card_count.set_value(snapshot.card_count);
        self.wild_stamp_count.set_value(snapshot.wild_stamp_count);
        self.wild_stamp_award_bound
            .set_value(snapshot.wild_stamp_award_bound);
        self.wild_stamp_awards_this_session
            .set_value(snapshot.wild_stamp_awards_this_session);
        self.wild_stamp_award_remaining
            .set_value(snapshot.wild_stamp_award_remaining);
        self.is_initialized.set_value(snapshot.is_initialized);
        self.season_ids = snapshot.season_ids;
        self.season_bitmask_count = snapshot.season_bitmask_count;
        self.season_xp_by_season = snapshot.season_xp_by_season;
        self.redeem_bitmask = snapshot.redeem_bitmask;
        self.escrow_bitmask = snapshot.escrow_bitmask;
        self.foreign_escrow_bitmask = snapshot.foreign_escrow_bitmask;
        self.first_character_connect_time
            .set_value(snapshot.first_character_connect_time);
    }
}

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("737D86A4-B063-49C1-AE69-E9EFA8ED11EC")]
#[::nw_network::type_registry(3290)]
pub struct SeasonsRewardsTrackedStatReplicatedState {
    pub start_time_point: ReplicatedFieldHandler<u64>,
    pub duration_at_start: ReplicatedFieldHandler<u64>,
    pub paid_duration_at_start: ReplicatedFieldHandler<u64>,
}
