//! Serialization primitives for packet and replicated-state payloads.
//!
//! Lengths use unsigned VLQ encodings. Ordered containers preserve stream
//! order; hash-based containers are available for semantic maps/sets where
//! deterministic byte order is not required.
//!
//! `Marshaler<bool>` is strict: only `0` and `1` are accepted on read.

pub mod buffer;
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
pub use compression_marshal::{
    Float16Marshaler, IntegerQuantizationMarshalerU8, IntegerQuantizationMarshalerU16,
    IntegerQuantizationMarshalerU32, PackedSize, QuatCompMarshaler, QuatCompNorm,
    QuatCompNormMarshaler, QuatCompNormQuantized, QuatCompNormQuantizedAngles,
    QuatCompNormQuantizedMarshaler, QuatSmallestThreeQuantized,
    QuatSmallestThreeQuantizedMarshaler, TransformCompressor, Vec2CompMarshaler, Vec3CompMarshaler,
    Vec3CompNormMarshaler,
};
pub use container_marshal::WIRE_VEC_CAP;
pub use data_marshal::{ConversionMarshaler, MarshalerConversion};
pub use error::MarshalerError;
pub use flat_bitmask::FlatBitmask;
pub use indexmap::{IndexMap, IndexSet};
pub use live_mask::{read_live_mask_batches, write_live_mask_batches};
pub use marshaler::{Codec, DefaultMarshaler, Marshaler};
pub use mask_chain::MaskChain;
pub use replicated_container::{
    Change, ChangeOp, ChangeSet, REPLICATED_CONTAINER_FIXED_JOURNAL_SIZE, ReplicatedContainer,
    ReplicatedIndexMap, ReplicatedMap, ReplicatedVec,
};
pub use replicated_field::{
    DeltaCompressedCounterHandler, DeltaCompressedReplicatedFieldHandler, DeltaIntegerMarshaler,
    DeltaMarshaler, DynamicDeltaReplicatedFieldHandler, FloatTimerDeltaReplicatedField,
    HalfF32Marshaler, HalfVec3Marshaler, IntegerOmitLowerByteMarshaler, PositionAnchorMarshaler,
    QuantizedRelativePosition, ReplicatedFieldHandler, ReplicatedFieldHandlerBase,
};
pub use utility_marshal::{BitSet, HalfF32, RawSequenceNumber};
pub use vlq::{VlqU16, VlqU16Marshaler, VlqU32, VlqU32Marshaler, VlqU64, VlqU64Marshaler};
