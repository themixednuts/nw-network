//! Notification queue replication.

use crate::{Marshaler, az_rtti, replicated_state, type_registry};

use crate::serialize::{IndexMap, ReplicatedContainer, VlqU64};

pub const MAX_NOTIFICATION_CHANGES: usize = 0x3fff;

#[derive(Marshaler, Debug, Clone, Default, PartialEq, Eq)]
pub struct NotificationEntry {
    pub field_00: u16,
    pub field_08: String,
    pub field_30: String,
}

#[replicated_state]
#[derive(Debug, Clone, Default)]
#[az_rtti("243A5629-72DB-4229-BBFA-ED6F09FDEFCA")]
#[type_registry(3340)]
pub struct NotificationServiceComponentReplicatedState {
    pub notifications:
        ReplicatedContainer<IndexMap<VlqU64, NotificationEntry>, MAX_NOTIFICATION_CHANGES>,
}
