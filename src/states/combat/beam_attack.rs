use crate::types::ActorRequestId;

#[derive(::nw_network::Marshaler, Debug, Clone, Default, PartialEq, Eq)]
#[::nw_network::az_rtti("90DC7C45-4851-4484-9188-A5B8E9C4194D")]
#[::nw_network::type_registry(4294)]
pub struct BeamAttackComponentReplicatedState {
    pub request_id: ActorRequestId,
    pub value: bool,
}
