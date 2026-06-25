use crate::hub::ReplicatedState;
use crate::serialize::{
    MarshalerError, MaskChain, ReadBuffer, ReplicatedFieldHandler, ReplicatedMap, ReplicatedVec,
    WriteBuffer,
};
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

#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ChunkMarshaler,
    nw_network_derive::AzRtti,
    nw_network_derive::Fragment,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("464EBD37-105F-4790-8C59-C46EBB4C57A6")]
#[type_registry(129)]
pub struct AttributeComponentReplicatedState {
    pub attributes: ReplicatedFieldHandler<CharacterAttributes>,
    pub attribute_bonuses: ReplicatedMap<CharacterAttributeType, u32>,
    pub placing_bonuses: ReplicatedVec<u32, 5>,
    pub persistent_attribute_data: ReplicatedFieldHandler<PersistentAttributeData>,
    pub hub: ReplicatedState,
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

    fn unmarshal_fields(&mut self, rb: &mut ReadBuffer) -> Result<(), MarshalerError> {
        let descriptor_mask = rb.read_u8()?;
        if (descriptor_mask & 0x01) == 0 {
            return Ok(());
        }

        let masks = MaskChain::unmarshal(rb)?;
        if masks.is_field_set(0) {
            self.attributes = ReplicatedFieldHandler::<CharacterAttributes>::unmarshal(rb)?;
        }
        if masks.is_field_set(1) {
            self.attribute_bonuses = ReplicatedMap::unmarshal(rb)?;
        }
        if masks.is_field_set(2) {
            self.placing_bonuses = ReplicatedVec::unmarshal(rb)?;
        }
        if masks.is_field_set(3) {
            self.persistent_attribute_data =
                ReplicatedFieldHandler::<PersistentAttributeData>::unmarshal(rb)?;
        }

        Ok(())
    }

    fn marshal_fields(&self, wb: &mut WriteBuffer) {
        let dirty = [
            self.attributes.is_dirty(),
            self.attribute_bonuses.is_dirty(),
            self.placing_bonuses.is_dirty(),
            self.persistent_attribute_data.is_dirty(),
        ];
        let any_dirty = dirty.iter().any(|dirty| *dirty);
        wb.write_u8(u8::from(any_dirty));
        if !any_dirty {
            return;
        }

        MaskChain::from_dirty_fields(&dirty).marshal(wb);
        if self.attributes.is_dirty() {
            self.attributes.marshal(wb);
        }
        if self.attribute_bonuses.is_dirty() {
            self.attribute_bonuses.marshal(wb);
        }
        if self.placing_bonuses.is_dirty() {
            self.placing_bonuses.marshal(wb);
        }
        if self.persistent_attribute_data.is_dirty() {
            self.persistent_attribute_data.marshal(wb);
        }
    }
}

crate::impl_hub_fragment!(
    AttributeComponentReplicatedState,
    hub = hub,
    marshal = marshal_fields,
    unmarshal = unmarshal_fields,
);
