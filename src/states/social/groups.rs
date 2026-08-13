//! Group, invite, and group-finder replication.

use uuid::Uuid;

use crate::types::{RemoteServerFacetRefHousingPlotComponentServerFacet, RemoteServerGdeRef};
use crate::{EntityRef, Marshaler};

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct GroupFinderApplicationData {
    pub application_kind: u8,
    pub status_kind: u8,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "wire shape keeps independent bool fields in order"
)]
pub struct GroupInviteData {
    pub invite_id: Uuid,
    pub group_id: Uuid,
    pub created_at: u64,
    pub invite_kind: u8,
    pub recipient: EntityRef,
    pub sender: EntityRef,
    pub expires_at: u64,
    pub optional_raid_id: Option<u64>,
    pub request_id: Uuid,
    pub source_kind: u8,
    pub auto_join: bool,
    pub is_cross_world: bool,
    pub activity_id: u32,
    pub has_activity_id: bool,
    pub is_declined: bool,
    pub is_removed: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct GameInviteData {
    pub invite_id: Uuid,
    pub entity: EntityRef,
    pub expires_at: u64,
}

/// Compact house references carried by group-data replication.
#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct GroupMemberHouseId {
    pub housing_plot_remote_ref: RemoteServerFacetRefHousingPlotComponentServerFacet,
    pub house_data_remote_ref: RemoteServerGdeRef,
}
pub use crate::generated::states::GroupsComponentReplicatedState;
