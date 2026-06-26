use crate::serialize::ReplicatedFieldHandler;

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("CB2BE398-151A-42BB-ACDF-1C5CF871BE84")]
#[::nw_network::type_registry(6786)]
pub struct EncounterManagerComponentReplicatedState {
    pub status: ReplicatedFieldHandler<i32>,
}
