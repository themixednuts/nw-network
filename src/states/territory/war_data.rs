use std::collections::HashMap;

use arrayvec::ArrayVec;
use uuid::Uuid;

use crate::Marshaler;
use crate::serialize::{ReplicatedMap, ReplicatedVec};

/// Fixed-cap list used by war-data participant blocks.
#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct WarDataUuidList(pub ArrayVec<Uuid, 10>);

/// Reused participant payload in one war-data entry.
#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct WarDataParticipantBlock {
    pub field_18: u64,
    pub field_20_ids: WarDataUuidList,
    pub field_180_id: Uuid,
    pub field_190: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct WarDataAssetReference {
    pub value_id: u32,
    pub instance_id: Uuid,
    pub type_id: Uuid,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct WarDataDetailBlock {
    pub field_10_id: Uuid,
    pub field_20: u8,
    pub field_24: u32,
    pub field_30_raw: u8,
    pub field_40c: bool,
    pub field_410: u32,
    pub field_420: u64,
    pub asset_ref: WarDataAssetReference,
}

/// Value payload of one replicated war-data entry.
#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct WarDataValue {
    pub field_10_id: Uuid,
    pub field_30_id: Uuid,
    pub field_40: WarDataParticipantBlock,
    pub field_1e0: WarDataParticipantBlock,
    pub field_380: u8,
    pub field_381: u8,
    pub field_382: u16,
    pub field_384: u32,
    pub field_390_id: Uuid,
    pub field_3a8: u64,
    pub detail: WarDataDetailBlock,
}

/// Value payload of a war schedule adjustment entry.
#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct WarScheduleAdjustmentReplicatedState {
    pub field_10_id: Uuid,
    pub field_20: u32,
    pub field_24: u16,
    pub field_28: u32,
    pub field_38: u64,
}

/// Value payload of one influence-race entry.
#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct InfluenceRaceData {
    pub field_10: u64,
    pub field_18: u16,
    pub field_1a: u8,
    pub field_1b: u8,
    pub field_1c: u8,
    pub field_1d: bool,
    pub field_20: Vec<u8>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct WarDataSnapshot {
    pub war_data_sequence: u64,
    pub schedule_adjustments_sequence: u64,
    pub influence_race_data_sequence: u64,
    pub war_data: Vec<WarDataValue>,
    pub schedule_adjustments: HashMap<u16, WarScheduleAdjustmentReplicatedState>,
    pub influence_race_data: HashMap<u16, InfluenceRaceData>,
}

/// Replicated war-data state.
#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("87072E57-BD7A-43DE-B221-382343AC0B43")]
#[::nw_network::type_registry(1739)]
pub struct WarDataComponentReplicatedState {
    pub war_data: ReplicatedVec<WarDataValue>,
    pub war_schedule_adjustments: ReplicatedMap<u16, WarScheduleAdjustmentReplicatedState>,
    pub influence_race_data: ReplicatedMap<u16, InfluenceRaceData>,
}

impl WarDataComponentReplicatedState {
    pub fn apply_snapshot(&mut self, snapshot: WarDataSnapshot) {
        self.war_data = ReplicatedVec::new(snapshot.war_data_sequence, snapshot.war_data);
        self.war_schedule_adjustments = ReplicatedMap::new(
            snapshot.schedule_adjustments_sequence,
            snapshot.schedule_adjustments,
        );
        self.influence_race_data = ReplicatedMap::new(
            snapshot.influence_race_data_sequence,
            snapshot.influence_race_data,
        );
    }
}
