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
#[az_rtti("CB2BE398-151A-42BB-ACDF-1C5CF871BE84")]
#[type_registry(6786)]
pub struct EncounterManagerComponentReplicatedState {
    pub status: ReplicatedFieldHandler<i32>,

    pub hub: ReplicatedState,
}
