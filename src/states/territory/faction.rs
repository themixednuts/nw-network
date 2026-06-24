use crate::hub::ReplicatedState;
use crate::serialize::ReplicatedFieldHandler;

#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ReplicatedState,
    nw_network_derive::AzRtti,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("80A502FD-67EB-4A1B-87E3-B85004919249")]
#[type_registry(3152)]
pub struct FactionComponentReplicatedState {
    #[replicated_state(group = 1)]
    pub pvp_flag_pending: ReplicatedFieldHandler<bool>,
    #[replicated_state(group = 1)]
    pub notify_pending: ReplicatedFieldHandler<bool>,
    #[replicated_state(group = 1)]
    pub pvp_flag_pending_end_time: ReplicatedFieldHandler<u64>,
    #[replicated_state(group = 1)]
    pub last_faction_change_timepoint: ReplicatedFieldHandler<u64>,
    #[replicated_state(group = 1)]
    pub faction_change_count: ReplicatedFieldHandler<u16>,
    #[replicated_state(group = 1)]
    pub ffa_pending_end_time: ReplicatedFieldHandler<u64>,
    #[replicated_state(group = 1)]
    pub ffa_anti_grouping_is_cursing: ReplicatedFieldHandler<bool>,
    #[replicated_state(group = 2)]
    pub faction: ReplicatedFieldHandler<u8>,
    #[replicated_state(group = 2)]
    pub pvp_flag: ReplicatedFieldHandler<bool>,
    #[replicated_state(group = 2)]
    pub has_sanctuary: ReplicatedFieldHandler<bool>,
    #[replicated_state(group = 2)]
    pub ffa_flag: ReplicatedFieldHandler<bool>,
    #[replicated_state(group = 2)]
    pub ffa_anti_grouping_curse_stacks: ReplicatedFieldHandler<u8>,
    #[replicated_state(group = 3)]
    pub time_at_flag_start: ReplicatedFieldHandler<u64>,
    #[replicated_state(group = 3)]
    pub pvp_value: ReplicatedFieldHandler<f32>,
    #[replicated_state(group = 3)]
    pub is_accumulating_pvp_value: ReplicatedFieldHandler<bool>,

    pub hub: ReplicatedState,
}
