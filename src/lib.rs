//! Packet serialization and network type ports.

extern crate self as nw_network;

pub mod context;
pub mod generated;
pub mod hub;
pub mod messages;
pub mod serialize;
pub mod source {
    //! Generated data types used by packet and state ports.

    pub use nw_network_types::types::*;
}
pub mod network_schema {
    //! Generated network type descriptors and lookup helpers.

    pub use nw_network_types::network_registry::*;
    pub use nw_network_types::network_schema::*;

    #[must_use]
    pub fn type_by_registry_index(registry_index: u32) -> Option<&'static NetworkTypeDescriptor> {
        type_index_for_registry_index(registry_index).and_then(type_by_type_index)
    }
}
pub mod states;
pub mod types;
pub mod validation;

pub use context::{
    ClientContext, ClientGde, ClientReplicationMetrics, GdeFragmentMap, GdeRegistry,
    INTEREST_ID_COUNT, INTEREST_ID_MAPPING_LEN, InterestIdMetadataMap, OfflineGdeReplicatedData,
    ReplicationIndex, RuntimeAddedComponent, SectorId, ServerGde, metadata_replication_category,
};
pub use generated::messages as generated_messages;
pub use hub::{
    ActorId, ActorRef, BandwidthMode, BaselineableFragment, BaselineableFragmentRef,
    ClientActorHash, ClientContextId, CrashTarget, Duration, DynFragment, FieldGroup,
    FieldGroupMut, FieldVector, FieldVectorMut, FixedMergeOutcome, FixedReplicatedState,
    FixedReplicatedStateFields, FixedStateRegister, Fragment, FragmentBase, FragmentCategory,
    FragmentCategoryBitset, FragmentKey, FragmentRegistration, FragmentTypeInfo, GroupBaselines,
    GroupIndex, HubId, InterestId, MAX_REPLICATION_CONTROL_MESSAGE_IDS, MarshalContext,
    MigratedPersistenceMetadata, MovementInteractionId, NamedField, NamedFieldMut,
    ReplicatedDefaultBits, ReplicatedFieldInfo, ReplicatedFieldInfoMut, ReplicatedFilterGroup,
    ReplicatedMergeOutcome, ReplicatedState, ReplicatedStateBundle, ReplicatedStateBundleView,
    ReplicationControl, ReplicationControlData, ReplicationPerformanceData, SequenceNumber,
    StateBundleBuilder, StateFragmentHeaderSpan, StateFragmentIter, StateFragmentView,
    StateRecordHeader, StateRecordWriter, SyncedTimestamp, Timestamp, TypeIndex,
};
pub use messages::{
    AuthToken, ClientVersionTokenMap, ConnTicket, ForceMigrateActorMsg, ForcePersistMsg,
    ForceRespawnMsg, ImpersonatedValues, LoginToken, MigrationTestMsg,
    ProcessDeferredMovementRequestsMsg, RegistrationRequestV3Msg, ScriptGarbageCollectMsg,
    StackConfigChangedMsg, TypeIndexCrc,
};
pub use network_schema::{
    NetworkFieldConfidence, NetworkFieldDescriptor, NetworkRegistryEntry,
    NetworkReplicatedContainerWireShape, NetworkTypeCapability, NetworkTypeDescriptor,
    NetworkTypeIdentity, NetworkWireScalarShape, NetworkWireShape, field_for_type_index,
    fields_for_type_index, is_known_type_index, is_replicated_state_type_index,
    name_for_registry_index, name_for_type_index, non_replicated_state_type_indices,
    registry_entry_by_registry_index, registry_entry_by_type_id, registry_entry_by_type_index,
    registry_index_for_type_id, registry_index_for_type_index, type_by_registry_index,
    type_by_type_id, type_by_type_index, type_index_for_registry_index,
    type_indices_missing_field_wire_shapes, unknown_type_indices,
};
pub use nw_network_derive::{
    FixedReplicatedStateFields, Fragment, Marshaler, az_rtti, fixed_replicated_state,
    replicated_state, type_registry,
};
pub use serialize::{
    Codec, ConversionMarshaler, DefaultMarshaler, Marshaler, MarshalerConversion, MarshalerError,
    ReadBuffer, ReadBufferMark, ReplicatedContainer, ReplicatedFieldHandler,
    ReplicatedFieldHandlerBase, VlqU16, VlqU16Marshaler, VlqU32, VlqU32Marshaler, VlqU64,
    VlqU64Marshaler, WriteBuffer, WriteBufferMark,
};
pub use types::{
    ActorRequestId, AfflictionData, AssetId, AzRtti, CharacterAttributeType, ClientRef,
    ComponentId, Crc32, DyeData, EntityId, EntityRef, GameModeParticipantStatus, GatheringStatus,
    GdeId, GdeRef, GeneralCooldownType, GridSides, PaperdollSlotAlias, RecipeCooldownData,
    RemoteServerContextRef, RemoteServerFacetRefGameModeParticipantComponentServerFacet,
    RemoteServerFacetRefHousingPlotComponentServerFacet, RemoteServerGdeRef,
    RemoteTypelessServerFacetRef, ReplicationCategory, TemporaryAffiliationRelationship,
    TemporaryAffiliationType, TimePoint, TypeRegistryEntry, WallClockTimePoint,
};
pub use validation::{
    ReplicatedStatePortStatus, StateFragmentTypeCoverage, replicated_state_port_statuses,
    validate_state_fragment_type_indices,
};
