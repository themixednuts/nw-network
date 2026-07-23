//! Wire data carried by runtime migration and persistence support.

use uuid::Uuid;

use crate::types::{ComponentId, EntityId};
use crate::{Marshaler, az_rtti};

/// Component identity captured when a server entity gains a component at
/// runtime.
///
/// The local component id is part of the replicated identity for the component.
/// It must be unique and deterministic for the entity that owns it; otherwise a
/// later migration, persistence restore, or component-targeted message can bind
/// to the wrong component.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Marshaler)]
#[az_rtti("C89C3758-4D0C-4BB4-9AA1-8BD7B51B2C97")]
pub struct RuntimeAddedComponent {
    /// Entity that owns the runtime component.
    pub entity_id: EntityId,
    /// Runtime type id of the component.
    pub component_type_id: Uuid,
    /// Local component id on `entity_id`.
    pub component_id: ComponentId,
}

impl RuntimeAddedComponent {
    #[must_use]
    pub const fn new(
        entity_id: EntityId,
        component_type_id: Uuid,
        component_id: ComponentId,
    ) -> Self {
        Self {
            entity_id,
            component_type_id,
            component_id,
        }
    }
}

// TODO: wire values unconfirmed

#[cfg(test)]
mod tests {
    use uuid::uuid;

    use super::*;
    use crate::serialize::{ReadBuffer, WriteBuffer, buffer::CARRIER_ENDIAN};
    use crate::{Marshal, Unmarshal};

    #[test]
    fn runtime_added_component_uses_entity_uuid_component_order() {
        let value = RuntimeAddedComponent::new(
            EntityId::new(0x0102_0304_0506_0708),
            uuid!("10203040-5060-7080-90a0-b0c0d0e0f001"),
            ComponentId::new(0x1112_1314_1516_1718),
        );

        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        value.marshal(&mut wb);

        assert_eq!(
            wb.as_slice(),
            &[
                0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x10, 0x20, 0x30, 0x40, 0x50, 0x60,
                0x70, 0x80, 0x90, 0xa0, 0xb0, 0xc0, 0xd0, 0xe0, 0xf0, 0x01, 0x11, 0x12, 0x13, 0x14,
                0x15, 0x16, 0x17, 0x18,
            ]
        );

        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, wb.as_slice());
        let decoded = RuntimeAddedComponent::unmarshal(&mut rb).unwrap();

        assert_eq!(decoded, value);
        assert_eq!(rb.left(), 0);
    }
}
