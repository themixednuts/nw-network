//! Territory-wide land-claim governance and progression replication.

use glam::{Vec3, Vec4};
use uuid::Uuid;

use crate::Marshaler;

#[derive(Debug, Clone, Default, PartialEq, Marshaler)]
pub struct LandClaimOwnerData {
    pub owner_id: Uuid,
    pub owner_name: String,
    pub field_12: u16,
    pub field_14: Vec4,
    pub field_24: u16,
    pub field_26: Vec4,
    pub field_name: String,
    pub field_flag: u8,
    pub field_time: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Marshaler)]
pub struct LandClaimGovernanceData {
    pub field_00: f32,
    pub field_04: f32,
    pub field_08: f32,
    pub field_0c: f32,
    pub field_10: bool,
    pub field_11: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct LandClaimProgressionPair {
    pub field_00: u32,
    pub field_04: u8,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct LandClaimProgressionTriple {
    pub field_00: u32,
    pub field_04: u8,
    pub field_08: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct LandClaimProgressionData {
    pub pairs: Vec<LandClaimProgressionPair>,
    pub triples: Vec<LandClaimProgressionTriple>,
    pub state: u8,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LandClaimSnapshot {
    pub sequence: u64,
    pub fcp_lock_timers: Vec<u16>,
    pub fcp_lock_anchor: u64,
    pub claim_keys: Vec<u16>,
    pub conflict_factions: Vec<u8>,
    pub conflict_lottery_end_times: Vec<u64>,
    pub conflict_start_times: Vec<u64>,
    pub darkness_thresholds: Vec<u32>,
    pub darkness_cycle_end_times: Vec<u64>,
    pub faction_control_point_data: Vec<u8>,
    pub faction1_influence_percentages: Vec<u8>,
    pub faction2_influence_percentages: Vec<u8>,
    pub faction3_influence_percentages: Vec<u8>,
    pub governance: Vec<LandClaimGovernanceData>,
    pub influence_race_start_times: Vec<u64>,
    pub positions: Vec<Vec3>,
    pub progressions: Vec<LandClaimProgressionData>,
    pub owners: Vec<LandClaimOwnerData>,
    pub war_dec_threshold_met_factions: Vec<u8>,
}

/// Territory-wide land-claim replicated state.
pub use crate::generated::states::LandClaimManagerComponentReplicatedState;
