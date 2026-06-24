use crate::Marshaler;
use crate::hub::ReplicatedState;
use crate::serialize::{QuatSmallestThreeQuantized, ReplicatedVec};

#[derive(Debug, Clone, Default, PartialEq, Marshaler)]
pub struct HousingItemValue {
    pub field_00: u16,
    pub field_02: u16,
    pub field_04: u16,
    pub rotation: QuatSmallestThreeQuantized,
    pub field_u32: u32,
    pub field_u8: u8,
}

#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ReplicatedState,
    nw_network_derive::AzRtti,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("C2938FCE-AF7A-447E-BAE7-AFCBFCC852AF")]
#[type_registry(3663)]
pub struct HouseDataReplicatedState {
    pub housing_items: ReplicatedVec<HousingItemValue>,

    pub hub: ReplicatedState,
}
