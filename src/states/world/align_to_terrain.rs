use crate::serialize::ReplicatedFieldHandler;

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("4641852C-9CE9-4FB6-AE2E-529758E74C58")]
#[::nw_network::type_registry(2850)]
pub struct AlignToTerrainComponentReplicatedState {
    pub alignment_mode: ReplicatedFieldHandler<u8>,
}
