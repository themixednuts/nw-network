use std::fmt;

use crate::serialize::{Marshaler, MarshalerError, ReadBuffer, WriteBuffer};

macro_rules! protocol_time {
    (
        $(#[$meta:meta])*
        $name:ident
    ) => {
        $(#[$meta])*
        #[repr(transparent)]
        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u64);

        impl $name {
            #[must_use]
            pub const fn new(value: u64) -> Self {
                Self(value)
            }

            #[must_use]
            pub const fn get(self) -> u64 {
                self.0
            }
        }

        impl From<u64> for $name {
            fn from(value: u64) -> Self {
                Self::new(value)
            }
        }

        impl From<$name> for u64 {
            fn from(value: $name) -> Self {
                value.get()
            }
        }

        impl PartialEq<u64> for $name {
            fn eq(&self, other: &u64) -> bool {
                self.0 == *other
            }
        }

        impl PartialEq<$name> for u64 {
            fn eq(&self, other: &$name) -> bool {
                *self == other.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }

        impl Marshaler for $name {
            const MARSHAL_SIZE: usize = <u64 as Marshaler>::MARSHAL_SIZE;

            #[inline]
            fn marshal(&self, wb: &mut WriteBuffer) {
                self.0.marshal(wb);
            }

            #[inline]
            fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
                Ok(Self(u64::unmarshal(rb)?))
            }
        }
    };
}

protocol_time!(
    /// Protocol duration carried as one raw 64-bit tick value.
    Duration
);

protocol_time!(
    /// Absolute protocol timestamp carried as one raw 64-bit tick value.
    Timestamp
);

protocol_time!(
    /// Clock-synchronized timestamp carried as one raw 64-bit tick value.
    SyncedTimestamp
);
