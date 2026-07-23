use uuid::Uuid;

use crate::{
    Fragment, az_rtti,
    hub::{DynFragment, FragmentBase},
    serialize::{Marshal, MarshalerError, ReadBuffer, Unmarshal, WriteBuffer},
    type_registry,
};

#[derive(Debug, Default, Fragment)]
#[az_rtti(uuid = "C65B2E9E-FA45-4804-98D8-EF38C3DB555B", name = "")]
#[type_registry(49)]
pub(crate) struct Type49 {
    base: FragmentBase,
    field_00: u32,
    field_04: Uuid,
}

impl DynFragment for Type49 {
    fn base(&self) -> &FragmentBase {
        &self.base
    }

    fn base_mut(&mut self) -> &mut FragmentBase {
        &mut self.base
    }

    fn marshal_contents(&self, wb: &mut WriteBuffer) -> bool {
        self.field_00.marshal(wb);
        self.field_04.marshal(wb);
        true
    }

    fn unmarshal_contents(&mut self, rb: &mut ReadBuffer<'_>) -> Result<bool, MarshalerError> {
        self.field_00 = u32::unmarshal(rb)?;
        self.field_04 = Uuid::unmarshal(rb)?;
        Ok(true)
    }
}

impl crate::hub::Fragment for Type49 {}
