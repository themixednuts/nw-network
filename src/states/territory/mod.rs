//! Territory, faction, land-claim, and war-data replicated state modules.

pub mod faction;
pub mod land_claim_manager;
pub mod war_data;

pub use crate::generated::states::LandClaimComponentReplicatedState;
pub use faction::FactionComponentReplicatedState;
pub use land_claim_manager::{
    LandClaimGovernanceData, LandClaimManagerComponentReplicatedState, LandClaimOwnerData,
    LandClaimProgressionData, LandClaimProgressionPair, LandClaimProgressionTriple,
    LandClaimSnapshot,
};
pub use war_data::{
    InfluenceRaceData, WarDataAssetReference, WarDataComponentReplicatedState, WarDataDetailBlock,
    WarDataParticipantBlock, WarDataSnapshot, WarDataUuidList, WarDataValue,
    WarScheduleAdjustmentReplicatedState,
};
