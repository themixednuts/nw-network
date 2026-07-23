//! Scripted-encounter state machine replication.
//!
//! These states replicate script progression, spawned entity identity, and
//! timer maps used by encounter scripts. The base state and instanced state are
//! intentionally asymmetric: the base state keeps `synced_timers` locally but
//! does not register it as a wire field, while the instanced variant does
//! replicate its timer map.

use crate::{az_rtti, replicated_state, type_registry};

use glam::Vec3;

use crate::serialize::{IndexMap, ReplicatedContainer, ReplicatedFieldHandler, VlqU64};
use crate::types::{Crc32, EntityId};

/// Base scripted-encounter state.
///
/// `synced_timers` is intentionally skipped and is not a registered wire field
/// for this base state. The instanced variant below does replicate timers; this
/// asymmetry is part of the protocol shape and should not be made uniform.
#[replicated_state]
#[derive(Debug, Clone, Default)]
#[az_rtti("B4DB39E2-5054-4604-9855-9A4DC75BDDE4")]
#[type_registry(3362)]
pub struct SlayerScriptReplicatedState {
    /// Current script state id, carried as a signed byte.
    pub cur_script_state_id: ReplicatedFieldHandler<i8>,
    /// Current script id.
    pub cur_script_id: ReplicatedFieldHandler<Crc32>,
    /// Local timer map intentionally omitted from the base state's wire fields.
    #[replicated_state(skip)]
    pub synced_timers: ReplicatedContainer<IndexMap<Crc32, VlqU64>>,
    /// Spawned entity ids keyed by script spawner id.
    pub spawned_entity_ids_by_spawner_id: ReplicatedContainer<IndexMap<Crc32, EntityId>>,
}

/// Instanced scripted-encounter state.
///
/// Group 0 carries script progression and replicated script maps. Group 1
/// carries placement and identity fields for the script instance.
#[replicated_state]
#[derive(Debug, Clone, Default)]
#[az_rtti("B5E124FB-D4D1-4479-9A0B-3623BEF6EF0B")]
#[type_registry(6234)]
pub struct InstancedSlayerScriptReplicatedState {
    /// Current script state id, carried as a signed byte.
    pub cur_script_state_id: ReplicatedFieldHandler<i8>,
    /// Current script id.
    pub cur_script_id: ReplicatedFieldHandler<Crc32>,
    /// Spawned entity ids keyed by script spawner id.
    pub spawned_entity_ids_by_spawner_id: ReplicatedContainer<IndexMap<Crc32, EntityId>>,
    /// Replicated timer map for this script instance.
    pub synced_timers: ReplicatedContainer<IndexMap<Crc32, VlqU64>>,

    /// Group 1 script tag identity.
    #[replicated_state(group = 1)]
    pub script_tag_id: ReplicatedFieldHandler<Crc32>,
    /// Group 1 world placement for the script instance.
    #[replicated_state(group = 1)]
    pub script_location: ReplicatedFieldHandler<Vec3>,
    /// Group 1 active task entity id.
    #[replicated_state(group = 1)]
    pub active_task_id: ReplicatedFieldHandler<EntityId>,
}

/// Snapshot used to update instanced script identity and spawned entities.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InstancedSlayerScriptSnapshot {
    /// Script tag identity to replicate.
    pub script_tag_id: Crc32,
    /// Last-modified sequence for the spawned entity map.
    pub spawned_entity_ids_sequence: u64,
    /// Spawned entity ids keyed by script spawner id.
    pub spawned_entity_ids_by_spawner_id: IndexMap<Crc32, EntityId>,
}

impl InstancedSlayerScriptReplicatedState {
    /// Applies script tag identity and a replacement spawned-entity map.
    pub fn apply_snapshot(&mut self, snapshot: InstancedSlayerScriptSnapshot) {
        self.script_tag_id.set_value(snapshot.script_tag_id);
        self.spawned_entity_ids_by_spawner_id = ReplicatedContainer::new(
            snapshot.spawned_entity_ids_sequence,
            snapshot.spawned_entity_ids_by_spawner_id,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hub::DynFragment;
    use crate::serialize::{CARRIER_ENDIAN, Marshal, ReadBuffer, WriteBuffer};

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
        let mut timers = IndexMap::new();
        timers.insert(Crc32::new(0x1020_3040), VlqU64::new(0x80));

        let state = SlayerScriptReplicatedState {
            synced_timers: ReplicatedContainer::new(7, timers),
            ..Default::default()
        };

        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        assert!(!DynFragment::marshal_contents(&state, &mut wb));
        assert!(wb.as_slice().is_empty());
    }

    #[test]
    fn crc_entity_map_uses_raw_crc_key_and_raw_entity_id_value() {
        let mut values = IndexMap::new();
        values.insert(
            Crc32::new(0x1122_3344),
            EntityId::new(0x0102_0304_0506_0708),
        );
        let map = ReplicatedContainer::<IndexMap<Crc32, EntityId>>::new(7, values);

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
        let mut values = IndexMap::new();
        values.insert(Crc32::new(0x1122_3344), VlqU64::new(0x80));
        let map = ReplicatedContainer::<IndexMap<Crc32, VlqU64>>::new(7, values);

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
