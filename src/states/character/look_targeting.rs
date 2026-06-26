use crate::serialize::ReplicatedFieldHandler;

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("490DB5F1-4E39-483A-9897-78FA312E45B5")]
#[::nw_network::type_registry(670)]
pub struct LookTargetingComponentReplicatedState {
    pub enabled: ReplicatedFieldHandler<bool>,
}
