use uuid::Uuid;

use crate::serialize::{Marshaler, MarshalerError, ReadBuffer, WriteBuffer};

/// Routing reference used by replication control messages.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ActorRef {
    pub world_id: u32,
    pub context_id: Uuid,
    pub correlation_id: Uuid,
}

impl ActorRef {
    pub const MARSHAL_SIZE: usize = 36;

    #[must_use]
    pub const fn new(world_id: u32, context_id: Uuid, correlation_id: Uuid) -> Self {
        Self {
            world_id,
            context_id,
            correlation_id,
        }
    }
}

impl Marshaler for ActorRef {
    const MARSHAL_SIZE: usize = Self::MARSHAL_SIZE;

    #[inline]
    fn marshal(&self, wb: &mut WriteBuffer) {
        self.world_id.marshal(wb);
        self.context_id.marshal(wb);
        self.correlation_id.marshal(wb);
    }

    #[inline]
    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        Ok(Self {
            world_id: u32::unmarshal(rb)?,
            context_id: Uuid::unmarshal(rb)?,
            correlation_id: Uuid::unmarshal(rb)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use uuid::uuid;

    use crate::serialize::{CARRIER_ENDIAN, ReadBuffer, WriteBuffer};

    use super::*;

    #[test]
    fn actor_ref_round_trips_as_fixed_width_payload() {
        let address = ActorRef::new(
            0xDEAD_BEEF,
            uuid!("11223344-5566-7788-99AA-BBCCDDEEFF00"),
            uuid!("AABBCCDD-EEFF-0011-2233-445566778899"),
        );
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);

        address.marshal(&mut wb);

        let bytes = wb.into_vec();
        assert_eq!(bytes.len(), ActorRef::MARSHAL_SIZE);
        assert_eq!(&bytes[..4], &0xDEAD_BEEFu32.to_be_bytes());

        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &bytes);
        assert_eq!(ActorRef::unmarshal(&mut rb).unwrap(), address);
        assert_eq!(rb.left(), 0);
    }
}
