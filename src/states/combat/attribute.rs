use crate::serialize::{ReplicatedFieldHandler, ReplicatedMap, ReplicatedVec};
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

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("464EBD37-105F-4790-8C59-C46EBB4C57A6")]
#[::nw_network::type_registry(129)]
pub struct AttributeComponentReplicatedState {
    pub attributes: ReplicatedFieldHandler<CharacterAttributes>,
    pub attribute_bonuses: ReplicatedMap<CharacterAttributeType, u32>,
    pub placing_bonuses: ReplicatedVec<u32, 5>,
    pub persistent_attribute_data: ReplicatedFieldHandler<PersistentAttributeData>,
}

impl AttributeComponentReplicatedState {
    pub fn apply_snapshot(&mut self, snapshot: AttributeSnapshot) {
        self.attributes.set_value(snapshot.attributes);
        self.attribute_bonuses = ReplicatedMap::new(
            snapshot.attribute_bonuses_sequence,
            snapshot
                .attribute_bonuses
                .into_iter()
                .map(|entry| (entry.attribute, entry.bonus))
                .collect(),
        );
        self.placing_bonuses =
            ReplicatedVec::new(snapshot.placing_bonuses_sequence, snapshot.placing_bonuses);
        self.persistent_attribute_data
            .set_value(snapshot.persistent_attribute_data);
    }
}
