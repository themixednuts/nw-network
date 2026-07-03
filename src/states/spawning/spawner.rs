//! Spawner active-count and source identity replication.

use crate::{az_rtti, replicated_state, type_registry};

use crate::RemoteServerGdeRef;
use crate::serialize::ReplicatedFieldHandler;

/// Active spawn count and source-spawner identity state.
#[replicated_state]
#[derive(Debug, Clone, Default)]
#[az_rtti("BE3332FA-B4D9-4832-BCCA-A75DC4F889D2")]
#[type_registry(4005)]
pub struct SpawnerComponentReplicatedState {
    pub num_active_spawns: ReplicatedFieldHandler<u32>,
    pub spawn_tag: ReplicatedFieldHandler<u32>,
    pub source_spawner_gde_ref: ReplicatedFieldHandler<RemoteServerGdeRef>,
}
