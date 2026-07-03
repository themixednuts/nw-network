//! Game-data-entity records and registry bookkeeping.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::context::RuntimeAddedComponent;
use crate::hub::{ClientActorHash, FragmentCategory, SequenceNumber};
use crate::states::world::GdeMetadataReplicatedState;
use crate::types::{AssetId, GdeId, GdeRef, ReplicationCategory};

/// Convert a fragment category to the category value carried by GDE metadata.
///
/// Metadata only has game-object categories. Fragment-only categories that do
/// not have a metadata category are reported as [`ReplicationCategory::Uncategorized`].
#[must_use]
pub const fn metadata_replication_category(category: FragmentCategory) -> ReplicationCategory {
    match category {
        FragmentCategory::Uncategorized => ReplicationCategory::Uncategorized,
        FragmentCategory::PlayerCharacter => ReplicationCategory::PlayerCharacter,
        FragmentCategory::NonPlayerCharacter => ReplicationCategory::NonPlayerCharacter,
        FragmentCategory::ImportantNonPlayerCharacter => {
            ReplicationCategory::ImportantNonPlayerCharacter
        }
        FragmentCategory::Buildable => ReplicationCategory::Buildable,
        FragmentCategory::Spell
        | FragmentCategory::Projectile
        | FragmentCategory::NumCategories => ReplicationCategory::Uncategorized,
    }
}

/// Loaded and pending GDE records keyed by protocol GDE id.
#[derive(Debug, Clone)]
pub struct GdeRegistry<R> {
    loaded: HashMap<GdeId, R>,
    pending: HashMap<GdeId, R>,
}

impl<R> Default for GdeRegistry<R> {
    fn default() -> Self {
        Self {
            loaded: HashMap::new(),
            pending: HashMap::new(),
        }
    }
}

impl<R> GdeRegistry<R> {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn loaded(&self) -> &HashMap<GdeId, R> {
        &self.loaded
    }

    #[must_use]
    pub fn pending(&self) -> &HashMap<GdeId, R> {
        &self.pending
    }

    /// Register a fully loaded GDE.
    ///
    /// If the same id was pending, the pending record is removed so lookup state
    /// remains unambiguous.
    pub fn register(&mut self, id: GdeId, record: R) -> Option<R> {
        self.pending.remove(&id);
        self.loaded.insert(id, record)
    }

    pub fn register_pending(&mut self, id: GdeId, record: R) -> Option<R> {
        if let Some(existing) = self.loaded.get_mut(&id) {
            return Some(std::mem::replace(existing, record));
        }
        self.pending.insert(id, record)
    }

    pub fn unregister(&mut self, id: GdeId) -> Option<R> {
        self.loaded.remove(&id)
    }

    pub fn unregister_pending(&mut self, id: GdeId) -> Option<R> {
        self.pending.remove(&id)
    }

    pub fn promote_pending(&mut self, id: GdeId) -> bool {
        let Some(record) = self.pending.remove(&id) else {
            return false;
        };
        self.loaded.insert(id, record);
        true
    }

    #[must_use]
    pub fn find(&self, id: GdeId) -> Option<&R> {
        self.loaded.get(&id)
    }

    pub fn find_mut(&mut self, id: GdeId) -> Option<&mut R> {
        self.loaded.get_mut(&id)
    }

    #[must_use]
    pub fn find_pending(&self, id: GdeId) -> Option<&R> {
        self.pending.get(&id)
    }

    pub fn find_pending_mut(&mut self, id: GdeId) -> Option<&mut R> {
        self.pending.get_mut(&id)
    }

    /// Find a record in loaded records first and then pending records.
    #[must_use]
    pub fn find_any(&self, id: GdeId) -> Option<&R> {
        self.loaded.get(&id).or_else(|| self.pending.get(&id))
    }

    pub fn find_any_mut(&mut self, id: GdeId) -> Option<&mut R> {
        if self.loaded.contains_key(&id) {
            self.loaded.get_mut(&id)
        } else {
            self.pending.get_mut(&id)
        }
    }

    #[must_use]
    pub fn is_pending(&self, id: GdeId) -> bool {
        self.pending.contains_key(&id)
    }

    #[must_use]
    pub fn len_loaded(&self) -> usize {
        self.loaded.len()
    }

    #[must_use]
    pub fn len_pending(&self) -> usize {
        self.pending.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.loaded.is_empty() && self.pending.is_empty()
    }
}

/// Server-side replication record for one GDE.
#[derive(Debug, Clone)]
pub struct ServerGde {
    asset_id: AssetId,
    gde_ref: GdeRef,
    replication_category: FragmentCategory,
    active_client_listeners: HashSet<ClientActorHash>,
    runtime_added_components: Vec<RuntimeAddedComponent>,
    metadata_fragment: Option<Arc<GdeMetadataReplicatedState>>,
}

impl Default for ServerGde {
    fn default() -> Self {
        Self::new(AssetId::nil(), GdeRef::default())
    }
}

impl ServerGde {
    #[must_use]
    pub fn new(asset_id: AssetId, gde_ref: GdeRef) -> Self {
        Self {
            asset_id,
            gde_ref,
            replication_category: FragmentCategory::Uncategorized,
            active_client_listeners: HashSet::new(),
            runtime_added_components: Vec::new(),
            metadata_fragment: None,
        }
    }

    #[must_use]
    pub const fn asset_id(&self) -> AssetId {
        self.asset_id
    }

    pub fn set_asset_id(&mut self, asset_id: AssetId) {
        if self.asset_id != asset_id {
            self.asset_id = asset_id;
            self.metadata_fragment = None;
        }
    }

    #[must_use]
    pub const fn gde_ref(&self) -> GdeRef {
        self.gde_ref
    }

    pub fn set_gde_ref(&mut self, gde_ref: GdeRef) {
        if self.gde_ref != gde_ref {
            self.gde_ref = gde_ref;
            self.metadata_fragment = None;
        }
    }

    #[must_use]
    pub const fn replication_category(&self) -> FragmentCategory {
        self.replication_category
    }

    pub fn set_replication_category(&mut self, category: FragmentCategory) {
        if self.replication_category != category {
            self.replication_category = category;
            self.metadata_fragment = None;
        }
    }

    #[must_use]
    pub fn active_client_listeners(&self) -> &HashSet<ClientActorHash> {
        &self.active_client_listeners
    }

    pub fn add_active_client_listener(&mut self, client: ClientActorHash) -> bool {
        self.active_client_listeners.insert(client)
    }

    pub fn remove_active_client_listener(&mut self, client: ClientActorHash) -> bool {
        self.active_client_listeners.remove(&client)
    }

    #[must_use]
    pub fn has_active_client_listener(&self, client: ClientActorHash) -> bool {
        self.active_client_listeners.contains(&client)
    }

    #[must_use]
    pub fn runtime_added_components(&self) -> &[RuntimeAddedComponent] {
        &self.runtime_added_components
    }

    pub fn runtime_added_components_mut(&mut self) -> &mut Vec<RuntimeAddedComponent> {
        &mut self.runtime_added_components
    }

    pub fn push_runtime_added_component(&mut self, component: RuntimeAddedComponent) {
        self.runtime_added_components.push(component);
    }

    /// Return the cached metadata fragment, if it has already been produced.
    #[must_use]
    pub fn metadata_fragment(&self) -> Option<Arc<GdeMetadataReplicatedState>> {
        self.metadata_fragment.clone()
    }

    /// Produce and cache the metadata fragment for this GDE.
    ///
    /// The supplied sequence is stamped onto the metadata fields on first
    /// production. Later calls reuse the cached fragment until an input that
    /// affects metadata changes.
    pub fn produce_metadata_fragment(
        &mut self,
        seq: SequenceNumber,
    ) -> Arc<GdeMetadataReplicatedState> {
        if let Some(metadata) = &self.metadata_fragment {
            return Arc::clone(metadata);
        }

        let mut metadata = GdeMetadataReplicatedState::with_asset(self.asset_id, self.gde_ref);
        metadata
            .replication_category
            .set_value(metadata_replication_category(self.replication_category));
        metadata.asset_id.set_last_modified(seq);
        metadata.gde_ref.set_last_modified(seq);
        metadata.replication_category.set_last_modified(seq);

        let metadata = Arc::new(metadata);
        self.metadata_fragment = Some(Arc::clone(&metadata));
        metadata
    }
}

/// Client-side record for one portrayed GDE.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ClientGde {
    replication_category: FragmentCategory,
}

impl ClientGde {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            replication_category: FragmentCategory::Uncategorized,
        }
    }

    #[must_use]
    pub const fn replication_category(&self) -> FragmentCategory {
        self.replication_category
    }

    pub fn set_replication_category(&mut self, category: FragmentCategory) {
        self.replication_category = category;
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use uuid::uuid;

    use super::*;
    use crate::hub::{DynFragment, MarshalContext};
    use crate::serialize::{CARRIER_ENDIAN, WriteBuffer};

    #[test]
    fn registry_tracks_loaded_and_pending_records() {
        let loaded_id = GdeId::new(10);
        let pending_id = GdeId::new(20);
        let mut registry = GdeRegistry::new();

        registry.register(loaded_id, "loaded");
        registry.register_pending(pending_id, "pending");

        assert_eq!(registry.find(loaded_id), Some(&"loaded"));
        assert_eq!(registry.find_pending(pending_id), Some(&"pending"));
        assert_eq!(registry.find_any(loaded_id), Some(&"loaded"));
        assert_eq!(registry.find_any(pending_id), Some(&"pending"));
        assert!(registry.is_pending(pending_id));

        assert!(registry.promote_pending(pending_id));
        assert_eq!(registry.find(pending_id), Some(&"pending"));
        assert!(!registry.is_pending(pending_id));

        assert_eq!(registry.unregister(loaded_id), Some("loaded"));
        assert!(registry.find_any(loaded_id).is_none());
    }

    #[test]
    fn metadata_fragment_is_cached_and_invalidated_by_category() {
        let asset_id = AssetId::from_uuid(uuid!("aaaaaaaa-bbbb-4ccc-8ddd-eeeeeeeeeeee"), 7);
        let gde_ref = GdeRef::new(uuid!("11111111-2222-4333-8444-555555555555"));
        let mut gde = ServerGde::new(asset_id, gde_ref);

        let first = gde.produce_metadata_fragment(SequenceNumber::Seq(9));
        let second = gde.produce_metadata_fragment(SequenceNumber::Seq(10));
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(first.asset_id.value().copied(), Some(asset_id));
        assert_eq!(first.gde_ref.value().copied(), Some(gde_ref));
        assert_eq!(
            first.replication_category.value().copied(),
            Some(ReplicationCategory::Uncategorized)
        );

        let mut wb = WriteBuffer::new(CARRIER_ENDIAN);
        assert!(first.marshal_contents_with(
            &MarshalContext {
                baseline_seq: SequenceNumber::Invalid,
                ..MarshalContext::default()
            },
            &mut wb
        ));
        assert!(!wb.as_slice().is_empty());

        gde.set_replication_category(FragmentCategory::Buildable);
        let third = gde.produce_metadata_fragment(SequenceNumber::Seq(11));
        assert!(!Arc::ptr_eq(&first, &third));
        assert_eq!(
            third.replication_category.value().copied(),
            Some(ReplicationCategory::Buildable)
        );
    }

    #[test]
    fn client_gde_defaults_to_uncategorized() {
        let mut gde = ClientGde::default();

        assert_eq!(gde.replication_category(), FragmentCategory::Uncategorized);

        gde.set_replication_category(FragmentCategory::PlayerCharacter);
        assert_eq!(
            gde.replication_category(),
            FragmentCategory::PlayerCharacter
        );
    }

    #[test]
    fn metadata_category_falls_back_for_fragment_only_categories() {
        assert_eq!(
            metadata_replication_category(FragmentCategory::Projectile),
            ReplicationCategory::Uncategorized
        );
    }
}
