use crate::serialize::{ReplicatedFieldHandler, ReplicatedVec};

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("2C91F563-3901-413D-941E-0CEF9366896D")]
#[::nw_network::type_registry(4896)]
pub struct MusicalPerformanceReplicatedState {
    pub performers: ReplicatedVec<u64>,
    pub radius: ReplicatedFieldHandler<f32>,
    pub zone_type: ReplicatedFieldHandler<u32>,
    pub selected_song: ReplicatedFieldHandler<u32>,
    pub packed_performer_sheet_indices: ReplicatedFieldHandler<u16>,
    pub start_time: ReplicatedFieldHandler<u64>,
    pub performance_state: ReplicatedFieldHandler<u8>,
    pub flags: ReplicatedFieldHandler<u8>,

    #[replicated_state(group = 1)]
    pub leader_index: ReplicatedFieldHandler<u8>,
    #[replicated_state(group = 1)]
    pub error_sync: ReplicatedFieldHandler<u8>,
    #[replicated_state(group = 1)]
    pub selected_reward_id: ReplicatedFieldHandler<u64>,
    #[replicated_state(group = 1)]
    pub performer_score: ReplicatedVec<u32>,
    #[replicated_state(group = 1)]
    pub performance_score: ReplicatedFieldHandler<f32>,

    #[replicated_state(group = 2)]
    pub performance_pages: ReplicatedVec<u16>,
    #[replicated_state(group = 2)]
    pub song_book_filter_type: ReplicatedFieldHandler<u8>,
    #[replicated_state(group = 2)]
    pub song_book_filter_type_id: ReplicatedFieldHandler<u32>,
    #[replicated_state(group = 2)]
    pub song_book_overrides: ReplicatedVec<u32>,
    #[replicated_state(group = 2)]
    pub reward_filter_type: ReplicatedFieldHandler<u8>,
    #[replicated_state(group = 2)]
    pub reward_filter_type_id: ReplicatedFieldHandler<u32>,
    #[replicated_state(group = 2)]
    pub reward_overrides: ReplicatedVec<u32>,
    #[replicated_state(group = 2)]
    pub rank: ReplicatedFieldHandler<u16>,
    #[replicated_state(group = 2)]
    pub end_reason: ReplicatedFieldHandler<u8>,

    #[replicated_state(group = 4)]
    pub performer_seed_0: ReplicatedFieldHandler<u64>,
    #[replicated_state(group = 4)]
    pub performance_zone_player_count: ReplicatedFieldHandler<u16>,
    #[replicated_state(group = 5)]
    pub performer_seed_1: ReplicatedFieldHandler<u64>,
    #[replicated_state(group = 6)]
    pub performer_seed_2: ReplicatedFieldHandler<u64>,
    #[replicated_state(group = 7)]
    pub performer_seed_3: ReplicatedFieldHandler<u64>,
}
