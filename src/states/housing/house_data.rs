//! House item, decoration, and placed-object replication.

use crate::Marshaler;
use crate::serialize::QuatSmallestThreeQuantized;

#[derive(Debug, Clone, Default, PartialEq, Marshaler)]
pub struct HousingItemValue {
    pub field_00: u16,
    pub field_02: u16,
    pub field_04: u16,
    pub rotation: QuatSmallestThreeQuantized,
    pub field_u32: u32,
    pub field_u8: u8,
}
pub use crate::generated::states::HouseDataReplicatedState;
