use crate::hub::ReplicatedState;
use crate::serialize::ReplicatedFieldHandler;

#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ReplicatedState,
    nw_network_derive::AzRtti,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("60D09696-C193-4F9D-8252-F1062BF21379")]
#[type_registry(57)]
pub struct ProjectileSpawnerReplicatedState {
    pub cur_ammo: ReplicatedFieldHandler<u16>,
    pub is_firing_blocked: ReplicatedFieldHandler<bool>,
    pub hub: ReplicatedState,
}
