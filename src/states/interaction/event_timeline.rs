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
#[az_rtti("9E6EE43B-15F1-497B-9461-1F97E488AA10")]
#[type_registry(363)]
pub struct EventTimelineComponentReplicatedState {
    pub timeline_index: ReplicatedFieldHandler<u16>,
    pub timeline_status: ReplicatedFieldHandler<u8>,

    pub hub: ReplicatedState,
}
