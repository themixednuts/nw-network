//! Client-side replication caches and counters.

use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::Arc;

use crate::hub::{DynFragment, InterestId};
use crate::states::world::GdeMetadataReplicatedState;
use crate::types::{GdeId, RemoteServerGdeRef, TimePoint};

/// Number of assignable interest ids.
///
/// `InterestId` is a `u16` protocol value. The zero slot is reserved for direct
/// lookup convenience, so metadata mappings allocate one additional entry.
pub const INTEREST_ID_COUNT: usize = u16::MAX as usize;

/// Number of entries in the direct interest-id metadata mapping.
pub const INTEREST_ID_MAPPING_LEN: usize = INTEREST_ID_COUNT + 1;

/// Fixed direct-lookup table from interest id to GDE metadata.
pub type InterestIdMetadataMap = [Option<Arc<GdeMetadataReplicatedState>>; INTEREST_ID_MAPPING_LEN];

/// Per-GDE fragment cache keyed by replication index.
pub type GdeFragmentMap = HashMap<ReplicationIndex, Arc<dyn DynFragment>>;

fn empty_interest_id_mapping() -> Box<InterestIdMetadataMap> {
    let mapping = (0..INTEREST_ID_MAPPING_LEN)
        .map(|_| None)
        .collect::<Vec<_>>()
        .into_boxed_slice();

    mapping
        .try_into()
        .unwrap_or_else(|_| unreachable!("interest-id mapping length is fixed"))
}

/// Per-record replication index used to identify a GDE fragment within a
/// replicated entity.
///
/// This is an in-memory cache key only; it deliberately has no wire codec
/// because fragments are identified on the wire by their fragment key and
/// type info, not by this index.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReplicationIndex(u32);

impl ReplicationIndex {
    #[must_use]
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

impl From<u32> for ReplicationIndex {
    fn from(value: u32) -> Self {
        Self::new(value)
    }
}

impl From<ReplicationIndex> for u32 {
    fn from(value: ReplicationIndex) -> Self {
        value.get()
    }
}

impl fmt::Display for ReplicationIndex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Opaque AOI sector key used by client phasing bookkeeping.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SectorId(u64);

impl SectorId {
    #[must_use]
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl From<u64> for SectorId {
    fn from(value: u64) -> Self {
        Self::new(value)
    }
}

impl From<SectorId> for u64 {
    fn from(value: SectorId) -> Self {
        value.get()
    }
}

impl fmt::Display for SectorId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Cached replicated data for an offline portrayed GDE.
#[derive(Debug, Clone, Default)]
pub struct OfflineGdeReplicatedData {
    pub gde_ref: RemoteServerGdeRef,
    pub replicated_state: GdeFragmentMap,
}

impl OfflineGdeReplicatedData {
    #[must_use]
    pub fn new(gde_ref: RemoteServerGdeRef) -> Self {
        Self {
            gde_ref,
            replicated_state: HashMap::new(),
        }
    }

    pub fn insert_fragment(
        &mut self,
        index: impl Into<ReplicationIndex>,
        fragment: Arc<dyn DynFragment>,
    ) -> Option<Arc<dyn DynFragment>> {
        self.replicated_state.insert(index.into(), fragment)
    }

    #[must_use]
    pub fn fragment(&self, index: impl Into<ReplicationIndex>) -> Option<&Arc<dyn DynFragment>> {
        self.replicated_state.get(&index.into())
    }

    pub fn remove_fragment(
        &mut self,
        index: impl Into<ReplicationIndex>,
    ) -> Option<Arc<dyn DynFragment>> {
        self.replicated_state.remove(&index.into())
    }

    #[must_use]
    pub fn peek_fragment<T>(&self) -> Option<&T>
    where
        T: DynFragment + 'static,
    {
        self.replicated_state
            .values()
            .find_map(|fragment| fragment.as_ref().downcast_ref::<T>())
    }
}

/// Client-side counters that are pure replication data.
#[derive(Debug, Clone, PartialEq)]
pub struct ClientReplicationMetrics {
    pub replication_bytes_recv: usize,
    pub replication_kilobits_per_sec: f32,
    pub total_gdes: i32,
    pub previous_total_gdes: i32,
    pub delta_gdes: i32,
    pub new_gdes: usize,
    pub removed_gdes: usize,
    pub previous_new_gdes: usize,
    pub previous_removed_gdes: usize,
    pub new_update_arrived: bool,
    pub paused_gdes: usize,
    pub gdes_updated_per_sec: f32,
    pub consecutive_seconds_over_new_gde_limit: usize,
    pub consecutive_updates_over_new_gde_limit: usize,
    pub replication_performance_data_valid: bool,
    pub fragments_updated_per_sec: f32,
    pub emit_gde_telemetry: bool,
    pub replication_control_bundled: bool,
    pub reliable_bundles: bool,
    pub replicated_state_handlers_called: usize,
    pub replicated_state_merges_called: usize,
}

impl Default for ClientReplicationMetrics {
    fn default() -> Self {
        Self {
            replication_bytes_recv: 0,
            replication_kilobits_per_sec: 0.0,
            total_gdes: 0,
            previous_total_gdes: 0,
            delta_gdes: 0,
            new_gdes: 0,
            removed_gdes: 0,
            previous_new_gdes: 0,
            previous_removed_gdes: 0,
            new_update_arrived: false,
            paused_gdes: 0,
            gdes_updated_per_sec: 0.0,
            consecutive_seconds_over_new_gde_limit: 0,
            consecutive_updates_over_new_gde_limit: 0,
            replication_performance_data_valid: false,
            fragments_updated_per_sec: 0.0,
            emit_gde_telemetry: true,
            replication_control_bundled: false,
            reliable_bundles: false,
            replicated_state_handlers_called: 0,
            replicated_state_merges_called: 0,
        }
    }
}

/// Client replication bookkeeping for portrayed and offline GDEs.
#[derive(Debug)]
pub struct ClientContext {
    offline_portrayal_cache: HashMap<GdeId, OfflineGdeReplicatedData>,
    interest_id_mapping: Box<InterestIdMetadataMap>,
    gde_fragment_cache: HashMap<GdeId, GdeFragmentMap>,
    bandwidth_mode_from_server: u8,
    metrics: ClientReplicationMetrics,
    fragments_updated: HashMap<GdeId, i32>,
    gdes_per_sector: HashMap<SectorId, HashSet<GdeId>>,
    parent_gde_listeners: HashMap<GdeId, HashSet<GdeId>>,
    gdes_pending_phase_handle: HashSet<GdeId>,
    dungeon_phase: u64,
    is_phasing_enabled: bool,
    current_time_point: TimePoint,
}

impl Default for ClientContext {
    fn default() -> Self {
        Self {
            offline_portrayal_cache: HashMap::new(),
            interest_id_mapping: empty_interest_id_mapping(),
            gde_fragment_cache: HashMap::new(),
            bandwidth_mode_from_server: 0,
            metrics: ClientReplicationMetrics::default(),
            fragments_updated: HashMap::new(),
            gdes_per_sector: HashMap::new(),
            parent_gde_listeners: HashMap::new(),
            gdes_pending_phase_handle: HashSet::new(),
            dungeon_phase: 0,
            is_phasing_enabled: false,
            current_time_point: TimePoint::default(),
        }
    }
}

impl ClientContext {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn offline_portrayal_cache(&self) -> &HashMap<GdeId, OfflineGdeReplicatedData> {
        &self.offline_portrayal_cache
    }

    #[must_use]
    pub fn has_offline_gde_replication_data(&self, id: GdeId) -> bool {
        self.offline_portrayal_cache.contains_key(&id)
    }

    #[must_use]
    pub fn offline_gde_replication_data(&self, id: GdeId) -> Option<&OfflineGdeReplicatedData> {
        self.offline_portrayal_cache.get(&id)
    }

    pub fn offline_gde_replication_data_mut(
        &mut self,
        id: GdeId,
    ) -> Option<&mut OfflineGdeReplicatedData> {
        self.offline_portrayal_cache.get_mut(&id)
    }

    pub fn insert_offline_gde_replication_data(
        &mut self,
        id: GdeId,
        data: OfflineGdeReplicatedData,
    ) -> Option<OfflineGdeReplicatedData> {
        self.offline_portrayal_cache.insert(id, data)
    }

    pub fn remove_offline_gde_replication_data(
        &mut self,
        id: GdeId,
    ) -> Option<OfflineGdeReplicatedData> {
        self.offline_portrayal_cache.remove(&id)
    }

    pub fn store_offline_fragment(
        &mut self,
        id: GdeId,
        index: impl Into<ReplicationIndex>,
        fragment: Arc<dyn DynFragment>,
    ) -> Option<Arc<dyn DynFragment>> {
        self.offline_portrayal_cache
            .entry(id)
            .or_default()
            .insert_fragment(index, fragment)
    }

    #[must_use]
    pub fn peek_offline_gde_replication_data<T>(&self, id: GdeId) -> Option<&T>
    where
        T: DynFragment + 'static,
    {
        self.offline_portrayal_cache
            .get(&id)
            .and_then(OfflineGdeReplicatedData::peek_fragment::<T>)
    }

    #[must_use]
    pub fn interest_id_mapping(&self) -> &InterestIdMetadataMap {
        self.interest_id_mapping.as_ref()
    }

    #[must_use]
    pub fn metadata_for_interest_id(
        &self,
        interest_id: impl Into<InterestId>,
    ) -> Option<&Arc<GdeMetadataReplicatedState>> {
        self.interest_id_mapping
            .get(usize::from(interest_id.into().get()))
            .and_then(Option::as_ref)
    }

    pub fn set_interest_metadata(
        &mut self,
        interest_id: impl Into<InterestId>,
        metadata: Arc<GdeMetadataReplicatedState>,
    ) -> bool {
        let idx = usize::from(interest_id.into().get());
        let Some(slot) = self.interest_id_mapping.get_mut(idx) else {
            return false;
        };
        *slot = Some(metadata);
        true
    }

    pub fn clear_interest_metadata(&mut self, interest_id: impl Into<InterestId>) -> bool {
        let idx = usize::from(interest_id.into().get());
        let Some(slot) = self.interest_id_mapping.get_mut(idx) else {
            return false;
        };
        let had_value = slot.is_some();
        *slot = None;
        had_value
    }

    #[must_use]
    pub fn gde_fragment_cache(&self) -> &HashMap<GdeId, GdeFragmentMap> {
        &self.gde_fragment_cache
    }

    pub fn get_or_create_gde_fragments(&mut self, id: GdeId) -> &mut GdeFragmentMap {
        self.gde_fragment_cache.entry(id).or_default()
    }

    #[must_use]
    pub fn gde_fragments(&self, id: GdeId) -> Option<&GdeFragmentMap> {
        self.gde_fragment_cache.get(&id)
    }

    pub fn remove_gde_fragments(&mut self, id: GdeId) -> Option<GdeFragmentMap> {
        self.gde_fragment_cache.remove(&id)
    }

    #[must_use]
    pub const fn bandwidth_mode_from_server(&self) -> u8 {
        self.bandwidth_mode_from_server
    }

    pub fn set_bandwidth_mode_from_server(&mut self, mode: u8) {
        self.bandwidth_mode_from_server = mode;
    }

    #[must_use]
    pub const fn metrics(&self) -> &ClientReplicationMetrics {
        &self.metrics
    }

    pub fn metrics_mut(&mut self) -> &mut ClientReplicationMetrics {
        &mut self.metrics
    }

    #[must_use]
    pub fn fragments_updated(&self) -> &HashMap<GdeId, i32> {
        &self.fragments_updated
    }

    pub fn set_fragments_updated(&mut self, id: GdeId, count: i32) -> Option<i32> {
        self.fragments_updated.insert(id, count)
    }

    #[must_use]
    pub fn gdes_per_sector(&self) -> &HashMap<SectorId, HashSet<GdeId>> {
        &self.gdes_per_sector
    }

    pub fn add_gde_to_sector(&mut self, sector: impl Into<SectorId>, id: GdeId) -> bool {
        self.gdes_per_sector
            .entry(sector.into())
            .or_default()
            .insert(id)
    }

    pub fn remove_gde_from_sector(&mut self, sector: impl Into<SectorId>, id: GdeId) -> bool {
        let sector = sector.into();
        let Some(gdes) = self.gdes_per_sector.get_mut(&sector) else {
            return false;
        };
        let removed = gdes.remove(&id);
        if gdes.is_empty() {
            self.gdes_per_sector.remove(&sector);
        }
        removed
    }

    #[must_use]
    pub fn sector_gdes(&self, sector: impl Into<SectorId>) -> Option<&HashSet<GdeId>> {
        self.gdes_per_sector.get(&sector.into())
    }

    #[must_use]
    pub fn parent_gde_listeners(&self) -> &HashMap<GdeId, HashSet<GdeId>> {
        &self.parent_gde_listeners
    }

    pub fn add_parent_gde_listener(&mut self, parent: GdeId, listener: GdeId) -> bool {
        self.parent_gde_listeners
            .entry(parent)
            .or_default()
            .insert(listener)
    }

    pub fn remove_parent_gde_listener(&mut self, parent: GdeId, listener: GdeId) -> bool {
        let Some(listeners) = self.parent_gde_listeners.get_mut(&parent) else {
            return false;
        };
        let removed = listeners.remove(&listener);
        if listeners.is_empty() {
            self.parent_gde_listeners.remove(&parent);
        }
        removed
    }

    #[must_use]
    pub fn listeners_for_parent_gde(&self, parent: GdeId) -> Option<&HashSet<GdeId>> {
        self.parent_gde_listeners.get(&parent)
    }

    #[must_use]
    pub fn gdes_pending_phase_handle(&self) -> &HashSet<GdeId> {
        &self.gdes_pending_phase_handle
    }

    pub fn mark_gde_pending_phase_handle(&mut self, id: GdeId) -> bool {
        self.gdes_pending_phase_handle.insert(id)
    }

    pub fn clear_gde_pending_phase_handle(&mut self, id: GdeId) -> bool {
        self.gdes_pending_phase_handle.remove(&id)
    }

    #[must_use]
    pub fn is_gde_pending_phase_handle(&self, id: GdeId) -> bool {
        self.gdes_pending_phase_handle.contains(&id)
    }

    #[must_use]
    pub const fn dungeon_phase(&self) -> u64 {
        self.dungeon_phase
    }

    pub fn set_dungeon_phase(&mut self, dungeon_phase: u64) {
        self.dungeon_phase = dungeon_phase;
    }

    #[must_use]
    pub const fn is_phasing_enabled(&self) -> bool {
        self.is_phasing_enabled
    }

    pub fn set_phasing_enabled(&mut self, enabled: bool) {
        self.is_phasing_enabled = enabled;
    }

    #[must_use]
    pub const fn current_time_point(&self) -> TimePoint {
        self.current_time_point
    }

    pub fn set_current_time_point(&mut self, current_time_point: TimePoint) {
        self.current_time_point = current_time_point;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use uuid::uuid;

    use super::*;
    use crate::hub::{DynFragment, FragmentBase};
    use crate::serialize::{Marshal, MarshalerError, ReadBuffer, Unmarshal, WriteBuffer};
    use crate::types::{GdeRef, RemoteServerContextRef};

    fn remote_ref(target_id: GdeId) -> RemoteServerGdeRef {
        RemoteServerGdeRef::new(
            RemoteServerContextRef::from_uuid(uuid!("aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee")),
            target_id,
        )
    }

    #[derive(Debug, Default)]
    struct TestFragment {
        base: FragmentBase,
        value: u8,
    }

    impl DynFragment for TestFragment {
        fn base(&self) -> &FragmentBase {
            &self.base
        }

        fn base_mut(&mut self) -> &mut FragmentBase {
            &mut self.base
        }

        fn marshal_contents(&self, wb: &mut WriteBuffer) -> bool {
            self.value.marshal(wb);
            true
        }

        fn unmarshal_contents(&mut self, rb: &mut ReadBuffer) -> Result<bool, MarshalerError> {
            self.value = u8::unmarshal(rb)?;
            Ok(true)
        }
    }

    #[test]
    fn offline_portrayal_cache_inserts_peeks_and_removes() {
        let mut context = ClientContext::new();
        let gde_id = GdeId::new(42);
        let data = OfflineGdeReplicatedData::new(remote_ref(gde_id));

        assert!(!context.has_offline_gde_replication_data(gde_id));
        assert!(
            context
                .insert_offline_gde_replication_data(gde_id, data)
                .is_none()
        );
        assert!(context.has_offline_gde_replication_data(gde_id));

        context.store_offline_fragment(
            gde_id,
            ReplicationIndex::new(7),
            Arc::new(TestFragment {
                value: 99,
                ..Default::default()
            }),
        );

        let fragment = context
            .peek_offline_gde_replication_data::<TestFragment>(gde_id)
            .unwrap();
        assert_eq!(fragment.value, 99);

        let removed = context.remove_offline_gde_replication_data(gde_id).unwrap();
        assert_eq!(removed.gde_ref.target_id, gde_id);
        assert!(!context.has_offline_gde_replication_data(gde_id));
    }

    #[test]
    fn interest_mapping_uses_direct_interest_id_slot() {
        let mut context = ClientContext::new();
        let mut metadata = GdeMetadataReplicatedState::with_asset(
            uuid!("bbbbbbbb-cccc-4ddd-8eee-ffffffffffff").into(),
            GdeRef::new(uuid!("11111111-2222-4333-8444-555555555555")),
        );
        metadata
            .replication_category
            .set_value(crate::types::ReplicationCategory::Buildable);
        let metadata = Arc::new(metadata);

        assert_eq!(context.interest_id_mapping().len(), INTEREST_ID_MAPPING_LEN);
        assert!(context.set_interest_metadata(InterestId::new(3), Arc::clone(&metadata)));
        assert!(Arc::ptr_eq(
            context
                .metadata_for_interest_id(InterestId::new(3))
                .unwrap(),
            &metadata
        ));

        assert!(context.clear_interest_metadata(InterestId::new(3)));
        assert!(
            context
                .metadata_for_interest_id(InterestId::new(3))
                .is_none()
        );
    }

    #[test]
    fn sector_and_phase_maps_update_as_plain_sets() {
        let mut context = ClientContext::new();
        let sector = SectorId::new(5);
        let parent = GdeId::new(10);
        let child = GdeId::new(11);

        assert!(context.add_gde_to_sector(sector, child));
        assert!(context.sector_gdes(sector).unwrap().contains(&child));
        assert!(context.remove_gde_from_sector(sector, child));
        assert!(context.sector_gdes(sector).is_none());

        assert!(context.add_parent_gde_listener(parent, child));
        assert!(
            context
                .listeners_for_parent_gde(parent)
                .unwrap()
                .contains(&child)
        );
        assert!(context.remove_parent_gde_listener(parent, child));
        assert!(context.listeners_for_parent_gde(parent).is_none());

        assert!(context.mark_gde_pending_phase_handle(child));
        assert!(context.is_gde_pending_phase_handle(child));
        assert!(context.clear_gde_pending_phase_handle(child));
        assert!(!context.is_gde_pending_phase_handle(child));
    }
}
