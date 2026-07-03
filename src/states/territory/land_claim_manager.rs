//! Territory-wide land-claim governance and progression replication.

use crate::{az_rtti, replicated_state, type_registry};

use glam::{Vec3, Vec4};
use uuid::Uuid;

use crate::Marshaler;
use crate::serialize::{ReplicatedContainer, ReplicatedFieldHandler};

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
#[replicated_state]
#[derive(Debug, Clone, Default)]
#[az_rtti("ABBA1776-6E4C-4BA6-A831-6F4052AFC9C0")]
#[type_registry(3086)]
pub struct LandClaimManagerComponentReplicatedState {
    pub fcp_lock_timers: ReplicatedContainer<Vec<u16>>,
    pub fcp_lock_anchor: ReplicatedFieldHandler<u64>,
    pub replicated_claim_keys: ReplicatedContainer<Vec<u16>>,
    pub replicated_conflict_faction: ReplicatedContainer<Vec<u8>>,
    pub replicated_conflict_lottery_end_time: ReplicatedContainer<Vec<u64>>,
    pub replicated_conflict_start_time: ReplicatedContainer<Vec<u64>>,
    pub replicated_darkness_threshold: ReplicatedContainer<Vec<u32>>,
    pub replicated_darkness_cycle_end_time: ReplicatedContainer<Vec<u64>>,
    pub replicated_faction_control_point_data: ReplicatedContainer<Vec<u8>>,
    pub replicated_faction1_influence_percentages: ReplicatedContainer<Vec<u8>>,
    pub replicated_faction2_influence_percentages: ReplicatedContainer<Vec<u8>>,
    pub replicated_faction3_influence_percentages: ReplicatedContainer<Vec<u8>>,
    pub replicated_governance: ReplicatedContainer<Vec<LandClaimGovernanceData>>,
    pub replicated_influence_race_start_time: ReplicatedContainer<Vec<u64>>,
    pub replicated_pos_data: ReplicatedContainer<Vec<Vec3>>,
    pub replicated_progression: ReplicatedContainer<Vec<LandClaimProgressionData>>,
    pub replicated_owner_data: ReplicatedContainer<Vec<LandClaimOwnerData>>,
    pub replicated_war_dec_threshold_met_faction: ReplicatedContainer<Vec<u8>>,
}

impl LandClaimManagerComponentReplicatedState {
    pub fn apply_snapshot(&mut self, snapshot: LandClaimSnapshot) {
        let sequence = snapshot.sequence;

        self.fcp_lock_timers = ReplicatedContainer::new(sequence, snapshot.fcp_lock_timers);
        self.fcp_lock_anchor.set_value(snapshot.fcp_lock_anchor);
        self.replicated_claim_keys = ReplicatedContainer::new(sequence, snapshot.claim_keys);
        self.replicated_conflict_faction =
            ReplicatedContainer::new(sequence, snapshot.conflict_factions);
        self.replicated_conflict_lottery_end_time =
            ReplicatedContainer::new(sequence, snapshot.conflict_lottery_end_times);
        self.replicated_conflict_start_time =
            ReplicatedContainer::new(sequence, snapshot.conflict_start_times);
        self.replicated_darkness_threshold =
            ReplicatedContainer::new(sequence, snapshot.darkness_thresholds);
        self.replicated_darkness_cycle_end_time =
            ReplicatedContainer::new(sequence, snapshot.darkness_cycle_end_times);
        self.replicated_faction_control_point_data =
            ReplicatedContainer::new(sequence, snapshot.faction_control_point_data);
        self.replicated_faction1_influence_percentages =
            ReplicatedContainer::new(sequence, snapshot.faction1_influence_percentages);
        self.replicated_faction2_influence_percentages =
            ReplicatedContainer::new(sequence, snapshot.faction2_influence_percentages);
        self.replicated_faction3_influence_percentages =
            ReplicatedContainer::new(sequence, snapshot.faction3_influence_percentages);
        self.replicated_governance = ReplicatedContainer::new(sequence, snapshot.governance);
        self.replicated_influence_race_start_time =
            ReplicatedContainer::new(sequence, snapshot.influence_race_start_times);
        self.replicated_pos_data = ReplicatedContainer::new(sequence, snapshot.positions);
        self.replicated_progression = ReplicatedContainer::new(sequence, snapshot.progressions);
        self.replicated_owner_data = ReplicatedContainer::new(sequence, snapshot.owners);
        self.replicated_war_dec_threshold_met_faction =
            ReplicatedContainer::new(sequence, snapshot.war_dec_threshold_met_factions);
    }
}
