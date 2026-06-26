use crate::serialize::ReplicatedFieldHandler;
use crate::types::PaperdollSlotAlias;

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("F98E278D-6B34-4DEB-B0D7-3B6786988D65")]
#[::nw_network::type_registry(1528)]
pub struct DamageReceiverComponentReplicatedState {
    pub is_block_active: ReplicatedFieldHandler<bool>,
    pub block_weapon_slot_alias: ReplicatedFieldHandler<PaperdollSlotAlias>,
    pub debug_crit_chance: ReplicatedFieldHandler<f32>,
}
