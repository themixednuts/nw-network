use crate::hub::ReplicatedState;
use crate::serialize::ReplicatedFieldHandler;
use crate::types::{AssetId, GdeRef, ReplicationCategory};

#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ReplicatedState,
    nw_network_derive::AzRtti,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("203DC8C7-0C60-454B-A46F-566114314B84")]
#[type_registry(10)]
pub struct GdeMetadataReplicatedState {
    pub asset_id: ReplicatedFieldHandler<AssetId>,
    pub gde_ref: ReplicatedFieldHandler<GdeRef>,
    pub replication_category: ReplicatedFieldHandler<ReplicationCategory>,

    pub hub: ReplicatedState,
}

impl GdeMetadataReplicatedState {
    #[must_use]
    pub fn with_asset(asset_id: AssetId, gde_ref: GdeRef) -> Self {
        let mut state = Self::default();
        state.asset_id.set_value(asset_id);
        state.gde_ref.set_value(gde_ref);
        state
    }
}
