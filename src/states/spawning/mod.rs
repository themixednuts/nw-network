//! Spawning and encounter replicated state modules.

pub mod clear_encounter_zones;
pub mod encounter;
pub mod encounter_manager;
pub mod spawner;

pub use crate::generated::states::VariationComponentReplicatedState;
pub use clear_encounter_zones::ClearEncounterZonesReplicatedState;
pub use encounter::{
    EncounterComponentReplicatedState, EncounterStatusEntry, MAX_ENCOUNTER_STATUS_ENTRIES,
};
pub use encounter_manager::EncounterManagerComponentReplicatedState;
pub use spawner::SpawnerComponentReplicatedState;
