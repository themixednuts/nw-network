use crate::RemoteServerGdeRef;
use crate::hub::ReplicatedState;
use crate::serialize::ReplicatedFieldHandler;

/// Active spawn count and source-spawner identity state.
#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ReplicatedState,
    nw_network_derive::AzRtti,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("BE3332FA-B4D9-4832-BCCA-A75DC4F889D2")]
#[type_registry(4005)]
pub struct SpawnerComponentReplicatedState {
    pub num_active_spawns: ReplicatedFieldHandler<u32>,
    pub spawn_tag: ReplicatedFieldHandler<u32>,
    pub source_spawner_gde_ref: ReplicatedFieldHandler<RemoteServerGdeRef>,

    pub hub: ReplicatedState,
}
