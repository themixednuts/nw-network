use crate::serialize::{ReplicatedMap, VlqU64};

pub const MAX_NOTIFICATION_CHANGES: usize = 0x3fff;

#[derive(::nw_network::Marshaler, Debug, Clone, Default, PartialEq, Eq)]
pub struct NotificationEntry {
    pub field_00: u16,
    pub field_08: String,
    pub field_30: String,
}

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("243A5629-72DB-4229-BBFA-ED6F09FDEFCA")]
#[::nw_network::type_registry(3340)]
pub struct NotificationServiceComponentReplicatedState {
    pub notifications: ReplicatedMap<VlqU64, NotificationEntry, MAX_NOTIFICATION_CHANGES>,
}
