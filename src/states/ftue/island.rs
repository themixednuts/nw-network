use crate::serialize::ReplicatedFieldHandler;

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("C2A97536-74E8-46F2-A372-9D16BF072B6A")]
#[::nw_network::type_registry(1055)]
pub struct FtueIslandComponentReplicatedState {
    pub player_entered_trigger: ReplicatedFieldHandler<bool>,
}
