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
#[az_rtti("4641852C-9CE9-4FB6-AE2E-529758E74C58")]
#[type_registry(2850)]
pub struct AlignToTerrainComponentReplicatedState {
    pub alignment_mode: ReplicatedFieldHandler<u8>,

    pub hub: ReplicatedState,
}
