use crate::Marshaler;
use crate::hub::ReplicatedState;
use crate::serialize::{ReplicatedFieldHandler, ReplicatedMap};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Marshaler)]
pub struct MountDyeData {
    pub channels: [u8; 4],
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Marshaler)]
pub struct SummonAuthorization {
    pub authorized: bool,
    pub value: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct PersistentMountDataValue {
    pub dye_data: MountDyeData,
    pub name: String,
}

#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ReplicatedState,
    nw_network_derive::AzRtti,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("2C20C9F7-2500-496A-89E2-5ADA1053B5C2")]
#[type_registry(5620)]
pub struct MountComponentReplicatedState {
    #[replicated_state(group = 2)]
    pub mount_id: ReplicatedFieldHandler<u32>,
    #[replicated_state(group = 1)]
    pub is_mounted: ReplicatedFieldHandler<bool>,
    #[replicated_state(group = 1)]
    pub summon_cooldown_end_time: ReplicatedFieldHandler<u64>,
    #[replicated_state(group = 1)]
    pub is_server_forcing_walk: ReplicatedFieldHandler<bool>,
    #[replicated_state(group = 1)]
    pub is_in_server_exclusion_volume: ReplicatedFieldHandler<bool>,
    #[replicated_state(group = 1)]
    pub summon_authorization: ReplicatedFieldHandler<SummonAuthorization>,
    #[replicated_state(group = 1)]
    pub persistent_mount_data: ReplicatedMap<u32, PersistentMountDataValue>,
    #[replicated_state(group = 1)]
    pub stamina_cur: ReplicatedFieldHandler<f32>,
    #[replicated_state(group = 1)]
    pub stamina_max: ReplicatedFieldHandler<f32>,
    #[replicated_state(group = 1)]
    pub stamina_regen_delay: ReplicatedFieldHandler<f32>,
    #[replicated_state(group = 1)]
    pub stamina_regen_rate: ReplicatedFieldHandler<f32>,
    #[replicated_state(group = 1)]
    pub stamina_drain_rate: ReplicatedFieldHandler<f32>,
    #[replicated_state(group = 1)]
    pub mult_max_stamina: ReplicatedFieldHandler<f32>,
    #[replicated_state(group = 1)]
    pub mult_stamina_regen_rate: ReplicatedFieldHandler<f32>,
    #[replicated_state(group = 3)]
    pub mount_remote_flags: ReplicatedFieldHandler<u8>,
    #[replicated_state(group = 3)]
    pub remote_dye_data: ReplicatedFieldHandler<MountDyeData>,

    pub hub: ReplicatedState,
}
