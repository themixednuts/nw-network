use std::hash::{Hash, Hasher};

use crate::Marshaler;
use crate::hub::{Duration, HubId, SyncedTimestamp};

/// Correlates the phases of one actor handoff between hubs.
///
/// Equality compares all fields. Hashing is intentionally narrowed to source
/// hub and movement sequence id, which keeps lookups centered on the peer that
/// originated the movement attempt while still satisfying Rust's hash contract.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Marshaler)]
pub struct MovementInteractionId {
    pub src_hub_id: HubId,
    pub dest_hub_id: HubId,
    pub movement_sequence_id: u64,
    pub start_time_on_source: SyncedTimestamp,
}

impl Hash for MovementInteractionId {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.src_hub_id.hash(state);
        self.movement_sequence_id.hash(state);
    }
}

/// Persistence bookkeeping carried with actors that migrated between hubs.
#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct MigratedPersistenceMetadata {
    pub seq_num: u64,
    pub is_dirty: bool,
    pub duration_since_marked_dirty_at: Duration,
    pub duration_until_rate_limited_until: Duration,
    pub chunk_count_max_ever: [u32; 2],
}

/// Debug target used by actor-movement fault injection messages.
#[repr(u32)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Marshaler)]
pub enum CrashTarget {
    Source = 0,
    Destination = 1,
    #[default]
    None = 2,
}

#[cfg(test)]
mod tests {
    use std::collections::hash_map::DefaultHasher;

    use uuid::uuid;

    use crate::hub::ActorId;
    use crate::serialize::{CARRIER_ENDIAN, Marshal, ReadBuffer, WriteBuffer};

    use super::*;

    fn round_trip<T>(value: &T) -> T
    where
        T: Marshaler + PartialEq + std::fmt::Debug,
    {
        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        value.marshal(&mut wb);
        let data = wb.into_vec();

        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, &data);
        let decoded = T::unmarshal(&mut rb).expect("value should unmarshal");
        assert_eq!(rb.left(), 0);
        decoded
    }

    #[test]
    fn actor_id_is_a_fixed_uuid_payload() {
        let actor_id = ActorId::new(uuid!("00112233-4455-6677-8899-aabbccddeeff"));

        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        actor_id.marshal(&mut wb);

        assert_eq!(ActorId::MARSHAL_SIZE, 16);
        assert_eq!(
            wb.into_vec(),
            [
                0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
                0xee, 0xff
            ]
        );
    }

    #[test]
    fn movement_interaction_id_round_trips_in_declared_order() {
        let id = MovementInteractionId {
            src_hub_id: HubId::new(0x1122_3344),
            dest_hub_id: HubId::new(0xaabb_ccdd),
            movement_sequence_id: 0x0102_0304_0506_0708,
            start_time_on_source: SyncedTimestamp::new(0x1020_3040_5060_7080),
        };

        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        id.marshal(&mut wb);

        assert_eq!(
            wb.into_vec(),
            [
                0x11, 0x22, 0x33, 0x44, 0xaa, 0xbb, 0xcc, 0xdd, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06,
                0x07, 0x08, 0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80
            ]
        );

        assert_eq!(round_trip(&id), id);
    }

    #[test]
    fn movement_interaction_hash_uses_source_hub_and_sequence() {
        let base = MovementInteractionId {
            src_hub_id: HubId::new(7),
            dest_hub_id: HubId::new(1),
            movement_sequence_id: 42,
            start_time_on_source: SyncedTimestamp::new(100),
        };
        let same_hash_different_value = MovementInteractionId {
            dest_hub_id: HubId::new(2),
            start_time_on_source: SyncedTimestamp::new(200),
            ..base
        };

        let mut lhs = DefaultHasher::new();
        let mut rhs = DefaultHasher::new();
        base.hash(&mut lhs);
        same_hash_different_value.hash(&mut rhs);

        assert_ne!(base, same_hash_different_value);
        assert_eq!(lhs.finish(), rhs.finish());
    }

    #[test]
    fn migrated_persistence_metadata_round_trips() {
        let metadata = MigratedPersistenceMetadata {
            seq_num: 9,
            is_dirty: true,
            duration_since_marked_dirty_at: Duration::new(10),
            duration_until_rate_limited_until: Duration::new(20),
            chunk_count_max_ever: [3, 4],
        };

        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        metadata.marshal(&mut wb);
        assert_eq!(
            wb.as_slice(),
            &[
                0, 0, 0, 0, 0, 0, 0, 9, // seq_num
                1, // is_dirty
                0, 0, 0, 0, 0, 0, 0, 10, // duration_since_marked_dirty_at
                0, 0, 0, 0, 0, 0, 0, 20, // duration_until_rate_limited_until
                0, 0, 0, 3, 0, 0, 0, 4, // chunk_count_max_ever
            ]
        );
        assert_eq!(round_trip(&metadata), metadata);
    }
}
