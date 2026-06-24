use std::fmt;

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

    };
}

id!(
    /// Actor interest identifier used by state-bundle records and replication control.
    InterestId(u16)
);

id!(
    /// Per-record fragment key written before a state fragment type id.
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
