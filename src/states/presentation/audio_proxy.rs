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
#[az_rtti("27830472-00A7-4204-A0BC-D1179F6EE4A2")]
#[type_registry(603)]
pub struct AudioProxyComponentReplicatedState {
    pub script_list_for_joints: ReplicatedFieldHandler<Vec<u32>>,

    pub hub: ReplicatedState,
}
