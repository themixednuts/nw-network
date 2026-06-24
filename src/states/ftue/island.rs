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
#[az_rtti("C2A97536-74E8-46F2-A372-9D16BF072B6A")]
#[type_registry(1055)]
pub struct FtueIslandComponentReplicatedState {
    pub player_entered_trigger: ReplicatedFieldHandler<bool>,

    pub hub: ReplicatedState,
}
