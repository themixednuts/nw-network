use crate::hub::ReplicatedState;
use crate::serialize::{ReplicatedFieldHandler, ReplicatedMap};
use crate::states::inventory::SimpleItemDescriptor;
use uuid::Uuid;

pub type CommittedResourceValue = SimpleItemDescriptor;

#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ReplicatedState,
    nw_network_derive::AzRtti,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("D12C9F8F-0564-464E-A78D-A8D56C91714C")]
#[type_registry(1594)]
pub struct BuildableControllerReplicatedState {
    pub current_state: ReplicatedFieldHandler<u8>,
    pub check_foundation_events: ReplicatedFieldHandler<bool>,
    pub committed_resources: ReplicatedMap<String, CommittedResourceValue>,
    pub ruin_damage_type: ReplicatedFieldHandler<u8>,
    pub territory_owner_guild_id: ReplicatedFieldHandler<Uuid>,
    pub upgrade_time_point: ReplicatedFieldHandler<u64>,
    pub crafting_station_type_crc: ReplicatedFieldHandler<u32>,
    pub camp_tier_buildable_id: ReplicatedFieldHandler<u32>,
    pub is_pvp_camp: ReplicatedFieldHandler<bool>,

    pub hub: ReplicatedState,
}
