//! Runtime context data for replicated game-data entities.
//!
//! A context is the runtime environment that owns replicated game-data entities
//! (GDEs). This module models the replication-facing data only: server records
//! track per-entity listeners, runtime-added components, and the cached metadata
//! fragment used to announce a GDE to clients; client records cache portrayal
//! data, offline replicated fragments, and interest-id metadata mappings.
//!
//! Scheduling, event delivery, entity activation, and engine integration are the
//! responsibility of the embedding application. A Bevy ECS, a custom simulation
//! loop, or another host can use these structures as protocol bookkeeping while
//! deciding independently when to tick, spawn, destroy, or dispatch events.

pub mod client;
pub mod gde;
pub mod migration;

pub use client::{
    ClientContext, ClientReplicationMetrics, GdeFragmentMap, INTEREST_ID_COUNT,
    INTEREST_ID_MAPPING_LEN, InterestIdMetadataMap, OfflineGdeReplicatedData, ReplicationIndex,
    SectorId,
};
pub use gde::{ClientGde, GdeRegistry, ServerGde, metadata_replication_category};
pub use migration::RuntimeAddedComponent;
