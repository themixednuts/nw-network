//! Player-housing replication.

use glam::Vec3;

use crate::serialize::{ReplicatedContainer, ReplicatedFieldHandler};
use crate::types::{
    RemoteServerFacetRefHousingPlotComponentServerFacet, RemoteServerGdeRef, WallClockTimePoint,
};
use crate::{Marshaler, az_rtti, replicated_state, type_registry};

/// Owned-house value carried by `PlayerHousingReplicatedState::owned_houses`.
#[derive(Debug, Clone, Default, PartialEq, Marshaler)]
pub struct ReplicatedOwnedHouseData {
    pub housing_plot_remote_ref: RemoteServerFacetRefHousingPlotComponentServerFacet,
    pub house_data_remote_ref: RemoteServerGdeRef,
    pub position: Vec3,
    pub value_a: WallClockTimePoint,
    pub value_b: WallClockTimePoint,
    pub value_c: u16,
    pub flag_a: bool,
    pub values: Vec<u32>,
    pub value_d: f32,
    pub flag_b: bool,
    pub flag_c: bool,
    pub value_e: WallClockTimePoint,
    pub value_f: u32,
}

#[replicated_state]
#[derive(Debug, Clone, Default)]
#[az_rtti("4F70C3BB-8F7D-48C2-A0B6-95431F88F356")]
#[type_registry(2768)]
pub struct PlayerHousingReplicatedState {
    #[replicated_state(name = "hasPurchasedHouse")]
    pub has_purchased_house: ReplicatedFieldHandler<bool>,
    #[replicated_state(name = "ownedHouses")]
    pub owned_houses: ReplicatedContainer<Vec<ReplicatedOwnedHouseData>>,
    #[replicated_state(name = "lastHouseRequestResponse")]
    pub last_house_request_response: ReplicatedFieldHandler<u8>,
    #[replicated_state(name = "m_isWithinAPlot")]
    pub is_within_a_plot: ReplicatedFieldHandler<bool>,
    #[replicated_state(name = "m_isFastTravelChanneling")]
    pub is_fast_travel_channeling: ReplicatedFieldHandler<bool>,
    #[replicated_state(name = "m_phasedHousingPlotEntityId")]
    pub phased_housing_plot_entity_id: ReplicatedFieldHandler<u64>,
    #[replicated_state(name = "m_phasedHouseDataEntityId")]
    pub phased_house_data_entity_id: ReplicatedFieldHandler<u64>,
    #[replicated_state(name = "m_phasedHouseOwnerCharacterId")]
    pub phased_house_owner_character_id: ReplicatedFieldHandler<String>,
    #[replicated_state(name = "m_debugDataString")]
    pub debug_data_string: ReplicatedFieldHandler<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owned_houses_uses_the_native_container_handler() {
        fn assert_type(_: &ReplicatedContainer<Vec<ReplicatedOwnedHouseData>>) {}

        assert_type(&PlayerHousingReplicatedState::default().owned_houses);
    }
}
