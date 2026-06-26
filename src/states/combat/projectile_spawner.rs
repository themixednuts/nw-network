use crate::serialize::ReplicatedFieldHandler;

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("60D09696-C193-4F9D-8252-F1062BF21379")]
#[::nw_network::type_registry(57)]
pub struct ProjectileSpawnerReplicatedState {
    pub cur_ammo: ReplicatedFieldHandler<u16>,
    pub is_firing_blocked: ReplicatedFieldHandler<bool>,
}
