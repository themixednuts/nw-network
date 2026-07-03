//! House item, decoration, and placed-object replication.

use crate::{az_rtti, replicated_state, type_registry};

use crate::Marshaler;
use crate::serialize::{QuatSmallestThreeQuantized, ReplicatedContainer};

#[derive(Debug, Clone, Default, PartialEq, Marshaler)]
pub struct HousingItemValue {
    pub field_00: u16,
    pub field_02: u16,
    pub field_04: u16,
    pub rotation: QuatSmallestThreeQuantized,
    pub field_u32: u32,
    pub field_u8: u8,
}

#[replicated_state]
#[derive(Debug, Clone, Default)]
#[az_rtti("C2938FCE-AF7A-447E-BAE7-AFCBFCC852AF")]
#[type_registry(3663)]
pub struct HouseDataReplicatedState {
    pub housing_items: ReplicatedContainer<Vec<HousingItemValue>>,
}
