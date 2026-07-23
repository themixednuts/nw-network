//! Polymorphic parameter values attached to an actor instantiation request.

use indexmap::{IndexMap, map::Entry};
use uuid::Uuid;

use crate::serialize::{ClassValue, Marshal, MarshalerError, ReadBuffer, Unmarshal, WriteBuffer};

/// Insertion-ordered actor parameter values keyed by their reflected class UUID.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ActorInstantiationParameters {
    values: IndexMap<Uuid, ClassValue>,
}

impl ActorInstantiationParameters {
    #[must_use]
    pub fn len(&self) -> usize {
        self.values.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn insert(&mut self, value: ClassValue) -> Option<ClassValue> {
        self.values.insert(value.uuid(), value)
    }

    #[must_use]
    pub fn get(&self, uuid: &Uuid) -> Option<&ClassValue> {
        self.values.get(uuid)
    }

    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&Uuid, &ClassValue)> {
        self.values.iter()
    }
}

impl Marshal for ActorInstantiationParameters {
    fn marshal(&self, wb: &mut WriteBuffer) {
        let count = u16::try_from(self.values.len())
            .expect("actor instantiation parameter count exceeds u16");
        count.marshal(wb);
        for value in self.values.values() {
            true.marshal(wb);
            value.marshal(wb);
        }
    }
}

impl Unmarshal for ActorInstantiationParameters {
    fn unmarshal(rb: &mut ReadBuffer<'_>) -> Result<Self, MarshalerError> {
        let count = usize::from(u16::unmarshal(rb)?);
        let mut values = IndexMap::with_capacity(count);
        for _ in 0..count {
            if !bool::unmarshal(rb)? {
                return Err(MarshalerError::NullPolymorphicValue);
            }
            let value = ClassValue::unmarshal(rb)?;
            if let Entry::Vacant(entry) = values.entry(value.uuid()) {
                entry.insert(value);
            }
        }
        Ok(Self { values })
    }
}

#[cfg(test)]
mod tests {
    use crate::{Marshaler, az_rtti, type_registry};

    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Marshaler)]
    #[az_rtti("D3154937-EDB0-4CD6-9C34-6288DA7BB07E")]
    #[type_registry(65_001, class)]
    struct TestParameter {
        value: u32,
    }

    #[test]
    fn round_trips_descriptor_dispatched_values() {
        let mut parameters = ActorInstantiationParameters::default();
        parameters.insert(ClassValue::from_marshaled(&TestParameter {
            value: 0x1234_5678,
        }));

        let mut wb = WriteBuffer::carrier();
        parameters.marshal(&mut wb);
        let mut rb = ReadBuffer::carrier(wb.as_slice());
        let decoded = ActorInstantiationParameters::unmarshal(&mut rb).expect("parameters");

        assert_eq!(decoded, parameters);
        assert!(rb.is_empty());
    }
}
