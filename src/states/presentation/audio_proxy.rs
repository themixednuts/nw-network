use crate::serialize::ReplicatedFieldHandler;

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("27830472-00A7-4204-A0BC-D1179F6EE4A2")]
#[::nw_network::type_registry(603)]
pub struct AudioProxyComponentReplicatedState {
    pub script_list_for_joints: ReplicatedFieldHandler<Vec<u32>>,
}
