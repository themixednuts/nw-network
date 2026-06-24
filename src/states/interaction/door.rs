use crate::Marshaler;
use crate::hub::ReplicatedState;
use crate::serialize::ReplicatedFieldHandler;

/// Generated door-state value carried as one byte in replicated state.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Marshaler)]
#[repr(u8)]
pub enum DoorState {
    #[default]
    Open = 0,
    Closed = 1,
    Count = 2,
}

impl DoorState {
    #[must_use]
    pub const fn from_value(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Open),
            1 => Some(Self::Closed),
            2 => Some(Self::Count),
            _ => None,
        }
    }

    #[must_use]
    pub const fn value(self) -> u8 {
        self as u8
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Open => "Open",
            Self::Closed => "Closed",
            Self::Count => "Count",
        }
    }
}

impl From<DoorState> for u8 {
    fn from(value: DoorState) -> Self {
        value.value()
    }
}

#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ReplicatedState,
    nw_network_derive::AzRtti,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("8D68FB93-B087-474F-9B5B-3FE33A8434AE")]
#[type_registry(2330)]
pub struct DoorComponentReplicatedState {
    pub door_state: ReplicatedFieldHandler<DoorState>,

    pub hub: ReplicatedState,
}
