//! Group, invite, and group-finder replication.

use crate::{az_rtti, replicated_state, type_registry};

use uuid::Uuid;

use crate::serialize::{IndexMap, ReplicatedContainer, ReplicatedFieldHandler};
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

#[replicated_state]
#[derive(Debug, Clone, Default)]
#[az_rtti("CE526687-CA4B-4647-A599-EC026FDC0C6D")]
#[type_registry(1994)]
pub struct GroupsComponentReplicatedState {
    pub group_id: ReplicatedFieldHandler<Uuid>,
    pub raid_id: ReplicatedFieldHandler<u64>,
    pub opposing_raid_id: ReplicatedFieldHandler<u64>,
    #[replicated_state(group = 1)]
    pub raid_type: ReplicatedFieldHandler<u8>,
    #[replicated_state(group = 1)]
    pub group_finder_group_id: ReplicatedFieldHandler<Uuid>,
    #[replicated_state(group = 1)]
    pub is_group_finder_group_creator: ReplicatedFieldHandler<bool>,
    #[replicated_state(group = 1)]
    pub create_source: ReplicatedFieldHandler<u8>,
    #[replicated_state(group = 1)]
    pub group_finder_applications: ReplicatedContainer<IndexMap<Uuid, GroupFinderApplicationData>>,
    #[replicated_state(group = 1)]
    pub inbound_group_invites: ReplicatedContainer<IndexMap<Uuid, GroupInviteData>>,
    #[replicated_state(group = 1)]
    pub outbound_group_invites: ReplicatedContainer<IndexMap<Uuid, GroupInviteData>>,
    #[replicated_state(group = 1)]
    pub next_eligible_abandon_game_mode_vote_time: ReplicatedFieldHandler<u64>,
    #[replicated_state(group = 1)]
    pub game_invite_data: ReplicatedFieldHandler<GameInviteData>,
    #[replicated_state(group = 1)]
    pub is_group_pristine: ReplicatedFieldHandler<bool>,
}
