//! Packet serialization and network type ports.

extern crate self as nw_network;

pub mod hub;
pub mod serialize;
pub mod source {
    //! Generated data types used by packet and state ports.

    pub use nw_network_types::types::*;
}
pub mod states;
pub mod types;

pub use hub::{
    BandwidthMode, ClientContextId, DynFragment, FieldGroup, FieldGroupMut, FieldVector,
    FieldVectorMut, FixedMergeOutcome, FixedReplicatedState, FixedReplicatedStateFields,
    FixedStateRegister, Fragment, FragmentBase, FragmentCategory, FragmentCategoryBitset,
    FragmentKey, FragmentRegistration, GroupBaselines, GroupIndex, InterestId,
    MAX_REPLICATION_CONTROL_MESSAGE_IDS, MarshalContext, NamedField, NamedFieldMut,
    ReplicatedDefaultBits, ReplicatedFieldInfo, ReplicatedFieldInfoMut, ReplicatedFilterGroup,
    ReplicatedMergeOutcome, ReplicatedState, ReplicatedStateBundle, ReplicatedStateBundleView,
    ReplicationControl, ReplicationControlData, ReplicationPerformanceData, SequenceNumber,
    StateBundleBuilder, StateFragmentHeaderSpan, StateFragmentIter, StateFragmentTypeId,
    StateFragmentView, StateRecordHeader, StateRecordWriter, TypeIndex,
};
pub use nw_network_derive::{
    AzRtti, ChunkMarshaler, FixedReplicatedStateFields, Marshaler, ReplicatedState, TypeRegistry,
};
pub use serialize::{
    Codec, DefaultMarshaler, Marshaler, MarshalerError, ReadBuffer, ReadBufferMark,
    ReplicatedContainer, ReplicatedFieldHandler, ReplicatedFieldHandlerBase, VlqU16,
    VlqU16Marshaler, VlqU32, VlqU32Marshaler, VlqU64, VlqU64Marshaler, WriteBuffer,
    WriteBufferMark,
};
pub use types::{
    ActorRequestId, AfflictionData, AssetId, AzRtti, Bounds2, CharacterAttributeType, ComponentId,
    Crc32, EntityId, EntityRef, GameModeParticipantStatus, GatheringStatus, GdeId, GdeRef,
    GeneralCooldownType, PaperdollSlotAlias, RemoteServerContextRef,
    RemoteServerFacetRefGameModeParticipantComponentServerFacet, RemoteServerGdeRef,
    RemoteTypelessServerFacetRef, ReplicationCategory, TimePoint, TypeRegistryEntry,
    WallClockTimePoint,
};
