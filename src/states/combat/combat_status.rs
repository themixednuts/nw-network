use crate::hub::ReplicatedState;
use crate::serialize::ReplicatedFieldHandler;
use crate::types::WallClockTimePoint;

#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ReplicatedState,
    nw_network_derive::AzRtti,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("150498EC-431A-4B9C-9895-E26B3D709F01")]
#[type_registry(2913)]
pub struct CombatStatusComponentReplicatedState {
    pub in_combat: ReplicatedFieldHandler<bool>,
    pub in_pvp_combat: ReplicatedFieldHandler<bool>,
    pub combat_logged_out_time: ReplicatedFieldHandler<WallClockTimePoint>,
    pub combat_concluded_time: ReplicatedFieldHandler<WallClockTimePoint>,
    pub hub: ReplicatedState,
}
