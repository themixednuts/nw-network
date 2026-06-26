use crate::serialize::ReplicatedFieldHandler;

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("4A6A34E3-FE18-4746-972D-D48DCE3DB1E3")]
#[::nw_network::type_registry(4297)]
pub struct StaminaComponentReplicatedState {
    pub cur: ReplicatedFieldHandler<f32>,
    pub max: ReplicatedFieldHandler<f32>,
    pub winded: ReplicatedFieldHandler<f32>,
    pub regen: ReplicatedFieldHandler<f32>,
    pub mult_max: ReplicatedFieldHandler<f32>,
    pub mult_regen: ReplicatedFieldHandler<f32>,
}
