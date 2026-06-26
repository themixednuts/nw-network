use std::collections::HashMap;

use glam::Vec3;

use crate::serialize::{ReplicatedFieldHandler, ReplicatedMap, VlqU64};
use crate::types::{Crc32, EntityId};

pub type SlayerScriptEntityMap = ReplicatedMap<Crc32, EntityId>;
pub type SlayerScriptTimerMap = ReplicatedMap<Crc32, VlqU64>;

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("B4DB39E2-5054-4604-9855-9A4DC75BDDE4")]
#[::nw_network::type_registry(3362)]
pub struct SlayerScriptReplicatedState {
    pub cur_script_state_id: ReplicatedFieldHandler<i8>,
    pub cur_script_id: ReplicatedFieldHandler<Crc32>,
    #[replicated_state(skip)]
    pub synced_timers: SlayerScriptTimerMap,
    pub spawned_entity_ids_by_spawner_id: SlayerScriptEntityMap,
}

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("B5E124FB-D4D1-4479-9A0B-3623BEF6EF0B")]
#[::nw_network::type_registry(6234)]
pub struct InstancedSlayerScriptReplicatedState {
    pub cur_script_state_id: ReplicatedFieldHandler<i8>,
    pub cur_script_id: ReplicatedFieldHandler<Crc32>,
    pub spawned_entity_ids_by_spawner_id: SlayerScriptEntityMap,
    pub synced_timers: SlayerScriptTimerMap,

    #[replicated_state(group = 1)]
    pub script_tag_id: ReplicatedFieldHandler<Crc32>,
    #[replicated_state(group = 1)]
    pub script_location: ReplicatedFieldHandler<Vec3>,
    #[replicated_state(group = 1)]
    pub active_task_id: ReplicatedFieldHandler<EntityId>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InstancedSlayerScriptSnapshot {
    pub script_tag_id: Crc32,
    pub spawned_entity_ids_sequence: u64,
    pub spawned_entity_ids_by_spawner_id: HashMap<Crc32, EntityId>,
}

impl InstancedSlayerScriptReplicatedState {
    pub fn apply_snapshot(&mut self, snapshot: InstancedSlayerScriptSnapshot) {
        self.script_tag_id.set_value(snapshot.script_tag_id);
        self.spawned_entity_ids_by_spawner_id = ReplicatedMap::new(
            snapshot.spawned_entity_ids_sequence,
            snapshot.spawned_entity_ids_by_spawner_id,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::DynFragment;
    use crate::serialize::{CARRIER_ENDIAN, Marshaler, ReadBuffer, WriteBuffer};

    #[test]
    fn signed_script_state_id_uses_one_byte_wire_shape() {
        let mut state = SlayerScriptReplicatedState::default();
        state.cur_script_state_id.set_value(-1);

        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        assert!(DynFragment::marshal_contents(&state, &mut wb));
        assert_eq!(wb.as_slice(), &[0x01, 0x01, 0xff]);

        let mut decoded = SlayerScriptReplicatedState::default();
        let mut rb = ReadBuffer::new(CARRIER_ENDIAN, wb.as_slice());
        DynFragment::unmarshal_contents(&mut decoded, &mut rb).unwrap();

        assert_eq!(decoded.cur_script_state_id.value().copied(), Some(-1));
        assert_eq!(rb.left(), 0);
    }

    #[test]
    fn base_timer_map_is_not_a_registered_field() {
        let mut timers = HashMap::new();
        timers.insert(Crc32::new(0x1020_3040), VlqU64::new(0x80));

        let mut state = SlayerScriptReplicatedState::default();
        state.synced_timers = ReplicatedMap::new(7, timers);

        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        assert!(!DynFragment::marshal_contents(&state, &mut wb));
        assert!(wb.as_slice().is_empty());
    }

    #[test]
    fn crc_entity_map_uses_raw_crc_key_and_raw_entity_id_value() {
        let mut values = HashMap::new();
        values.insert(
            Crc32::new(0x1122_3344),
            EntityId::new(0x0102_0304_0506_0708),
        );
        let map = SlayerScriptEntityMap::new(7, values);

        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        map.marshal(&mut wb);

        assert_eq!(
            wb.as_slice(),
            &[
                0x00, // snapshot mode
                0x01, 0x07, // last-modified sequence
                0x01, // entry count
                0x11, 0x22, 0x33, 0x44, // CRC key
                0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, // entity id
            ]
        );
    }

    #[test]
    fn crc_timer_map_uses_raw_crc_key_and_vlq_timer_value() {
        let mut values = HashMap::new();
        values.insert(Crc32::new(0x1122_3344), VlqU64::new(0x80));
        let map = SlayerScriptTimerMap::new(7, values);

        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        map.marshal(&mut wb);

        assert_eq!(
            wb.as_slice(),
            &[
                0x00, // snapshot mode
                0x01, 0x07, // last-modified sequence
                0x01, // entry count
                0x11, 0x22, 0x33, 0x44, // CRC key
                0x80, 0x02, // VLQ u64
            ]
        );
    }
}
