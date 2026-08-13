//! Character attribute and attribute-bonus replication.

use crate::{CharacterAttributeType, Marshaler};

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct CharacterAttributeValue {
    pub attribute: CharacterAttributeType,
    pub points: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct CharacterAttributes {
    pub entries: Vec<CharacterAttributeValue>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AttributeBonus {
    pub attribute: CharacterAttributeType,
    pub bonus: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AttributeSnapshot {
    pub attributes: CharacterAttributes,
    pub attribute_bonuses_sequence: u64,
    pub attribute_bonuses: Vec<AttributeBonus>,
    pub placing_bonuses_sequence: u64,
    pub placing_bonuses: Vec<u32>,
    pub persistent_attribute_data: PersistentAttributeData,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct PersistentAttributeData {
    pub spent_points: u32,
    pub has_spent_points: bool,
    pub has_pre_reload_attributes: bool,
    pub pre_reload_attributes: CharacterAttributes,
    pub unspent_attribute_points: u32,
}
pub use crate::generated::states::AttributeComponentReplicatedState;
