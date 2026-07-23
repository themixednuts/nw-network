//! Serialization primitives for packet and replicated-state payloads.
//!
//! The module exposes byte buffers, value-level [`Marshaler`] impls,
//! field-local [`Codec`] policies, VLQ integers, container count policies,
//! field-mask helpers, and replicated field/container wrappers. Carrier scalar
//! values use big-endian byte order by default; variable counts use unsigned
//! VLQ encodings unless a field selects a raw-count policy.
//!
//! Ordered containers preserve stream order. Hash-based containers are
//! available for semantic maps and sets where deterministic byte order is not
//! required.
//!
//! `Marshaler<bool>` is strict: only `0` and `1` are accepted on read.

pub mod buffer;
pub mod composite_marshal;
pub mod compression_marshal;
pub mod container_marshal;
pub mod data_marshal;
pub mod error;
pub mod flat_bitmask;
pub mod live_mask;
pub mod marshaler;
pub mod mask_chain;
pub mod math_marshal;
mod quantize;
pub mod replicated_container;
pub mod replicated_field;
pub mod utility_marshal;
pub mod vlq;

pub use buffer::{
    CARRIER_ENDIAN, Endian, ReadBuffer, ReadBufferMark, WriteBuffer, WriteBufferMark,
};
pub use composite_marshal::{
    BooleanChoice, BooleanChoiceCodec, DefaultOmittedTupleCodec, OptionalCodec, TupleCodec,
};
pub use compression_marshal::{
    Float16Marshaler, IntegerQuantizationMarshalerU8, IntegerQuantizationMarshalerU16,
    IntegerQuantizationMarshalerU32, NonUniformScaleCompMarshaler, PackedNormalizedVec3Marshaller,
    PackedPositionMarshaller, PackedSize, QuatCompMarshaler, QuatCompNorm, QuatCompNormMarshaler,
    QuatCompNormQuantized, QuatCompNormQuantizedAngles, QuatCompNormQuantizedMarshaler,
    QuatSmallestThreeQuantized, QuatSmallestThreeQuantizedMarshaler, TransformCompressor,
    Vec2CompMarshaler, Vec3CompMarshaler, Vec3CompNormMarshaler,
};
pub use container_marshal::{
    ArrayCodec, ContainerMarshaler, MapContainerMarshaler, MapSequenceCodec, SequenceCodec,
    WIRE_VEC_CAP,
};
pub use data_marshal::{ConversionMarshaler, MarshalerConversion, RemoteServerGdeRefMarshaler};
pub use error::MarshalerError;
pub use flat_bitmask::FlatBitmask;
pub use indexmap::{IndexMap, IndexSet};
pub use live_mask::{read_live_mask_batches, write_live_mask_batches};
pub use marshaler::{Codec, DefaultMarshaler, Marshal, Marshaler, Unmarshal};
pub use mask_chain::MaskChain;
pub use replicated_container::{
    Change, ChangeOp, ChangeSet, REPLICATED_CONTAINER_FIXED_JOURNAL_SIZE, ReplicatedContainer,
};
pub use replicated_field::{
    DeltaCompressedCounterHandler, DeltaCompressedReplicatedFieldHandler, DeltaIntegerMarshaler,
    DeltaMarshaler, DynamicDeltaReplicatedFieldHandler, FloatTimerDeltaReplicatedField,
    HalfF32Marshaler, HalfVec3Marshaler, IntegerOmitLowerByteMarshaler, QuantizedRelativePosition,
    ReplicatedFieldHandler, ReplicatedFieldHandlerBase, quantize_with_range, unquantize_with_range,
};
pub use utility_marshal::{BitSet, HalfF32, RawSequenceNumber};
pub use vlq::{VlqU16, VlqU16Marshaler, VlqU32, VlqU32Marshaler, VlqU64, VlqU64Marshaler};
