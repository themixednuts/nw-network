//! Combat status flag replication.

use crate::{az_rtti, replicated_state, type_registry};

use crate::serialize::ReplicatedFieldHandler;
use crate::types::WallClockTimePoint;

#[replicated_state]
#[derive(Debug, Clone, Default)]
#[az_rtti("150498EC-431A-4B9C-9895-E26B3D709F01")]
#[type_registry(2913)]
pub struct CombatStatusComponentReplicatedState {
    pub in_combat: ReplicatedFieldHandler<bool>,
    pub in_pvp_combat: ReplicatedFieldHandler<bool>,
    pub combat_logged_out_time: ReplicatedFieldHandler<WallClockTimePoint>,
    pub combat_concluded_time: ReplicatedFieldHandler<WallClockTimePoint>,
}
