//! Notification queue replication.

use crate::Marshaler;

pub const MAX_NOTIFICATION_CHANGES: usize = 0x3fff;

#[derive(Marshaler, Debug, Clone, Default, PartialEq, Eq)]
pub struct NotificationEntry {
    pub field_00: u16,
    pub field_08: String,
    pub field_30: String,
}
pub use crate::generated::states::NotificationServiceComponentReplicatedState;
