use crate::Marshaler;
use crate::hub::{ReplicatedState, SequenceNumber};
use crate::serialize::{Change, ReplicatedFieldHandler, ReplicatedVec, VlqU64};

const CONTAINER_ITEM_MAX_COUNT: usize = 0x1f4;

pub type ContainerItemDescriptor = super::item_descriptor::ReplicatedItemDescriptor;

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct ContainerItemClasses {
    pub values: Vec<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContainerInventorySettings {
    pub item_classes: Vec<u64>,
    pub bonus_max_encumbrance: u32,
    pub can_transfer_items: bool,
    pub container_was_emptied: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerSnapshot {
    pub container_sequence: SequenceNumber,
    pub items: Vec<ContainerItemDescriptor>,
    pub inventory_settings: Option<ContainerInventorySettings>,
}

impl Default for ContainerSnapshot {
    fn default() -> Self {
        Self {
            container_sequence: SequenceNumber::Invalid,
            items: Vec::new(),
            inventory_settings: None,
        }
    }
}

#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ReplicatedState,
    nw_network_derive::AzRtti,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("EF1A20F2-F6CF-439F-A1AC-63460F803134")]
#[type_registry(1755)]
pub struct ContainerComponentReplicatedState {
    pub container: ReplicatedVec<ContainerItemDescriptor, CONTAINER_ITEM_MAX_COUNT>,
    pub item_class: ReplicatedFieldHandler<ContainerItemClasses>,
    pub bonus_max_encumbrance: ReplicatedFieldHandler<u32>,
    pub can_transfer_items: ReplicatedFieldHandler<bool>,
    pub container_was_emptied: ReplicatedFieldHandler<bool>,

    pub hub: ReplicatedState,
}

impl ContainerComponentReplicatedState {
    #[must_use]
    pub fn initial_baseline() -> Self {
        let mut state = Self {
            container: ReplicatedVec::new(3, vec![Self::empty_item_descriptor(); 5]),
            ..Self::default()
        };
        state.bonus_max_encumbrance.set_value(0);
        state.container_was_emptied.set_value(true);
        state
    }

    #[must_use]
    pub fn continuation_delta() -> Self {
        Self {
            container: ReplicatedVec::delta(vec![Change::update(
                VlqU64::new(0),
                Self::empty_item_descriptor(),
                9,
            )]),
            ..Self::default()
        }
    }

    pub fn apply_snapshot(&mut self, snapshot: ContainerSnapshot) {
        self.container = ReplicatedVec::new(snapshot.container_sequence, snapshot.items);

        if let Some(settings) = snapshot.inventory_settings {
            self.item_class.set_value(ContainerItemClasses {
                values: settings.item_classes,
            });
            self.bonus_max_encumbrance
                .set_value(settings.bonus_max_encumbrance);
            self.can_transfer_items
                .set_value(settings.can_transfer_items);
            self.container_was_emptied
                .set_value(settings.container_was_emptied);
        }
    }

    fn empty_item_descriptor() -> ContainerItemDescriptor {
        ContainerItemDescriptor {
            packed_item_id: 21_049_648_349_184,
            ..Default::default()
        }
    }
}
