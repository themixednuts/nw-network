use crate::serialize::marshaler::{Marshal, Unmarshal};
use std::fmt;

use crate::serialize::{MarshalerError, ReadBuffer, WriteBuffer};
use uuid::Uuid;

macro_rules! id {
    (
        $(#[$meta:meta])*
        $name:ident($inner:ty)
    ) => {
        $(#[$meta])*
        #[repr(transparent)]
        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name($inner);

        impl $name {
            #[must_use]
            pub const fn new(value: $inner) -> Self {
                Self(value)
            }

            #[must_use]
            pub const fn get(self) -> $inner {
                self.0
            }
        }

        impl From<$inner> for $name {
            fn from(value: $inner) -> Self {
                Self::new(value)
            }
        }

        impl From<$name> for $inner {
            fn from(value: $name) -> Self {
                value.get()
            }
        }

        impl PartialEq<$inner> for $name {
            fn eq(&self, other: &$inner) -> bool {
                self.0 == *other
            }
        }

        impl PartialEq<$name> for $inner {
            fn eq(&self, other: &$name) -> bool {
                *self == other.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }

        impl Marshal for $name {
            const MARSHAL_SIZE: usize = <$inner as Marshal>::MARSHAL_SIZE;

            #[inline]
            fn marshal(&self, wb: &mut WriteBuffer) {
                self.0.marshal(wb);
            }
        }

        impl Unmarshal for $name {

            #[inline]
            fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
                Ok(Self(<$inner as Unmarshal>::unmarshal(rb)?))
            }
        }
    };
}

id!(
    /// Hashed client actor identity used by replicated-state filter groups.
    ClientActorHash(u64)
);

id!(
    /// Actor interest identifier used by state-bundle records and replication control.
    InterestId(u16)
);

id!(
    /// Per-record fragment key written before fragment type info.
    FragmentKey(u32)
);

id!(
    /// Compact registered type index used when a type does not need a raw UUID on the wire.
    TypeIndex(u32)
);

id!(
    /// Client-context instance selected for a state-bundle stream.
    ClientContextId(u8)
);

id!(
    /// Server-selected bandwidth mode carried in a state-bundle header.
    BandwidthMode(u8)
);

id!(
    /// Stable actor identity carried by actor movement and routing messages.
    ActorId(Uuid)
);

id!(
    /// Hub identity used by cross-hub routing and movement coordination.
    HubId(u32)
);
