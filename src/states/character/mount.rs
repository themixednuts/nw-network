//! Mount ownership, dye, summon, and mount-mode replication.

use crate::Marshaler;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Marshaler)]
pub struct MountDyeData {
    pub channels: [u8; 4],
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Marshaler)]
pub struct SummonAuthorization {
    pub authorized: bool,
    pub value: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct PersistentMountDataValue {
    pub dye_data: MountDyeData,
    pub name: String,
}
pub use crate::generated::states::MountComponentReplicatedState;
