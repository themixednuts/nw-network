use std::{num::ParseIntError, str::Utf8Error};

use thiserror::Error;
use uuid::Error as UuidError;

#[derive(Debug, Error)]
pub enum MarshalerError {
    #[error("buffer underrun (needed {needed} bytes, had {available})")]
    BufferUnderrun { needed: usize, available: usize },

    #[error("invalid enum discriminant {value}")]
    InvalidDiscriminant { value: u8 },

    #[error("value {value} is outside valid range {min}..={max}")]
    InvalidRange { value: u64, min: u64, max: u64 },

    #[error("container length {len} exceeds capacity {capacity}")]
    ContainerOverflow { len: usize, capacity: usize },

    #[error("string length {len} exceeds capacity {capacity}")]
    StringOverflow { len: usize, capacity: usize },

    #[error("utf8 error: {0}")]
    Utf8(#[from] Utf8Error),

    #[error("uuid error: {0}")]
    Uuid(#[from] UuidError),

    #[error("parse int error: {0}")]
    ParseInt(#[from] ParseIntError),

    #[error("crc mismatch expected={expected:08x} actual={actual:08x}")]
    CrcMismatch { expected: u32, actual: u32 },

    #[error("payload truncated declared={declared} available={available}")]
    TruncatedPayload { declared: usize, available: usize },

    #[error("invalid message envelope flags {flags}")]
    InvalidEnvelopeFlags { flags: u8 },

    /// `envelope_flags == 0` — the wire said "metadata only, no message
    /// body." Distinct from [`MarshalerError::InvalidEnvelopeFlags`] so
    /// streaming callers can match-and-skip empty records without
    /// terminating the read loop.
    #[error("empty message envelope (envelope_flags=0)")]
    EmptyEnvelope,

    #[error("message type mismatch expected={expected} actual={actual}")]
    MessageTypeMismatch { expected: u32, actual: u32 },

    /// A polymorphic value field referenced an unknown compact type id.
    #[error("unknown compact type_index {type_index}")]
    UnknownTypeIndex { type_index: u32 },

    /// A polymorphic value field carried a UUID that is not declared as a
    /// variant of the receiving enum.
    #[error("polymorphic value carried a UUID with no matching variant")]
    UnknownClassUuid,
}

impl MarshalerError {
    #[inline]
    #[must_use]
    pub fn buffer_underrun(available: usize, needed: usize) -> Self {
        MarshalerError::BufferUnderrun { needed, available }
    }
}
