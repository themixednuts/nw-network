//! Descriptor-dispatched polymorphic values.

use std::{collections::HashMap, sync::LazyLock};

use uuid::Uuid;

use crate::{
    Marshaler,
    types::{AzRtti, TypeRegistryEntry},
};

use super::{Marshal, MarshalerError, ReadBuffer, Unmarshal, VlqU32Marshaler, WriteBuffer};

type ConsumeBodyFn = for<'a> fn(&mut ReadBuffer<'a>) -> Result<(), MarshalerError>;

/// Runtime decoder registration for one reflected network class.
pub struct NetworkClassRegistration {
    uuid: fn() -> Uuid,
    name: fn() -> &'static str,
    type_index: fn() -> u32,
    consume_body: ConsumeBodyFn,
}

impl NetworkClassRegistration {
    #[doc(hidden)]
    #[must_use]
    pub const fn of<T>() -> Self
    where
        T: AzRtti + TypeRegistryEntry + Unmarshal + 'static,
    {
        Self {
            uuid: || T::TYPE_ID,
            name: || T::TYPE_NAME,
            type_index: || T::TYPE_INDEX,
            consume_body: |rb| T::unmarshal(rb).map(|_| ()),
        }
    }
}

inventory::collect!(NetworkClassRegistration);

static REGISTRATIONS_BY_UUID: LazyLock<HashMap<Uuid, &'static NetworkClassRegistration>> =
    LazyLock::new(|| {
        let mut registrations = HashMap::new();
        for registration in inventory::iter::<NetworkClassRegistration> {
            let previous = registrations.insert((registration.uuid)(), registration);
            debug_assert!(
                previous.is_none(),
                "duplicate network class UUID registration"
            );
        }
        registrations
    });

static REGISTRATIONS_BY_TYPE_INDEX: LazyLock<HashMap<u32, &'static NetworkClassRegistration>> =
    LazyLock::new(|| {
        let mut registrations = HashMap::new();
        for registration in inventory::iter::<NetworkClassRegistration> {
            let previous = registrations.insert((registration.type_index)(), registration);
            debug_assert!(
                previous.is_none(),
                "duplicate network class type-index registration"
            );
        }
        registrations
    });

/// A reflected class value with the exact descriptor-consumed wire body.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClassValue {
    uuid: Uuid,
    type_index: u32,
    body: Vec<u8>,
}

impl ClassValue {
    #[must_use]
    pub fn from_marshaled<T>(value: &T) -> Self
    where
        T: AzRtti + TypeRegistryEntry + Marshaler,
    {
        let mut body = WriteBuffer::carrier();
        value.marshal(&mut body);
        Self {
            uuid: T::TYPE_ID,
            type_index: T::TYPE_INDEX,
            body: body.into_vec(),
        }
    }

    #[must_use]
    pub fn from_raw_uuid(uuid: Uuid, body: Vec<u8>) -> Self {
        Self {
            uuid,
            type_index: 0,
            body,
        }
    }

    #[must_use]
    pub const fn uuid(&self) -> Uuid {
        self.uuid
    }

    #[must_use]
    pub const fn type_index(&self) -> u32 {
        self.type_index
    }

    #[must_use]
    pub fn body(&self) -> &[u8] {
        &self.body
    }

    #[must_use]
    pub fn class_name(&self) -> Option<&'static str> {
        REGISTRATIONS_BY_UUID
            .get(&self.uuid)
            .map(|registration| (registration.name)())
    }
}

impl Marshal for ClassValue {
    fn marshal(&self, wb: &mut WriteBuffer) {
        VlqU32Marshaler.marshal(wb, self.type_index);
        if self.type_index == 0 {
            self.uuid.marshal(wb);
        }
        wb.write_bytes(&self.body);
    }
}

impl Unmarshal for ClassValue {
    fn unmarshal(rb: &mut ReadBuffer<'_>) -> Result<Self, MarshalerError> {
        let type_index = VlqU32Marshaler.unmarshal(rb)?;
        let (uuid, registration) = if type_index == 0 {
            let uuid = Uuid::unmarshal(rb)?;
            let registration = REGISTRATIONS_BY_UUID
                .get(&uuid)
                .copied()
                .ok_or(MarshalerError::UnknownClassUuid { uuid })?;
            (uuid, registration)
        } else {
            let registration = REGISTRATIONS_BY_TYPE_INDEX
                .get(&type_index)
                .copied()
                .ok_or(MarshalerError::UnknownTypeIndex { type_index })?;
            ((registration.uuid)(), registration)
        };

        let body_start = rb.position();
        (registration.consume_body)(rb)?;
        let body = rb.range(body_start..rb.position())?.to_vec();
        Ok(Self {
            uuid,
            type_index,
            body,
        })
    }
}
