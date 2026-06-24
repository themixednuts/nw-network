use crate::Marshaler;
use crate::hub::{ReplicatedState, SequenceNumber};
use crate::serialize::{
    MarshalerError, ReadBuffer, ReplicatedFieldHandler, ReplicatedMap, ReplicatedVec,
    VlqU16Marshaler, VlqU32, WriteBuffer,
};

pub type PaperdollItemDescriptor = super::item_descriptor::ReplicatedItemDescriptor;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Marshaler)]
pub struct PaperdollSlotFlags {
    pub bytes: [u8; 7],
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ItemVisualData {
    pub visual_id: VlqU32,
    pub flags: u8,
    pub variant_id: Option<u16>,
    pub color_channels: Option<[u8; 4]>,
    pub entitlement_version: u32,
    pub entitlement_flags: [u8; 4],
}

impl Marshaler for ItemVisualData {
    fn marshal(&self, wb: &mut WriteBuffer) {
        self.visual_id.marshal(wb);
        self.flags.marshal(wb);
        if (self.flags & 0x01) != 0 {
            VlqU16Marshaler.marshal(wb, self.variant_id.unwrap_or_default());
        }
        if (self.flags & 0x02) != 0 {
            wb.write_bytes(&self.color_channels.unwrap_or_default());
        }
        self.entitlement_version.marshal(wb);
        wb.write_bytes(&self.entitlement_flags);
    }

    fn unmarshal(rb: &mut ReadBuffer) -> Result<Self, MarshalerError> {
        let visual_id = VlqU32::unmarshal(rb)?;
        let flags = u8::unmarshal(rb)?;
        let variant_id = if (flags & 0x01) != 0 {
            Some(VlqU16Marshaler.unmarshal(rb)?)
        } else {
            None
        };
        let color_channels = if (flags & 0x02) != 0 {
            let mut bytes = [0u8; 4];
            bytes.copy_from_slice(rb.read_bytes(4)?);
            Some(bytes)
        } else {
            None
        };
        let entitlement_version = u32::unmarshal(rb)?;
        let mut entitlement_flags = [0u8; 4];
        entitlement_flags.copy_from_slice(rb.read_bytes(4)?);

        Ok(Self {
            visual_id,
            flags,
            variant_id,
            color_channels,
            entitlement_version,
            entitlement_flags,
        })
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct LoadedAmmoData {
    pub ammo_type: u8,
    pub item_id: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct LinkedLoadoutItem {
    pub slot_type: u32,
    pub item_slot: u16,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct LoadoutAttribute {
    pub attribute_id: u32,
    pub level: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct PaperdollLoadout {
    pub name: String,
    pub linked_loadout_items: Vec<LinkedLoadoutItem>,
    pub is_for_game_mode: bool,
    pub attributes: Vec<LoadoutAttribute>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PaperdollSnapshot {
    pub visible_durability: Option<ReplicatedVec<u32>>,
    pub non_visible_durability: Option<ReplicatedVec<u32>>,
    pub visible_paperdoll_slots: Option<ReplicatedVec<u32>>,
    pub non_visible_paperdoll_slots: Option<ReplicatedVec<u32>>,
    pub visible_full_item_data: Option<ReplicatedVec<PaperdollItemDescriptor>>,
    pub non_visible_full_item_data: Option<ReplicatedVec<PaperdollItemDescriptor>>,
    pub visible_item_visual_data: Option<ReplicatedVec<ItemVisualData>>,
    pub loadouts: Option<ReplicatedVec<PaperdollLoadout>>,
    pub hide_skins: bool,
    pub sheathe_map: Option<PaperdollSlotFlags>,
    pub item_slots_attachment_status: Option<PaperdollSlotFlags>,
    pub active_map: Option<PaperdollSlotFlags>,
    pub bonus_equip_load: Option<u16>,
    pub is_local_player: Option<bool>,
    pub is_loadout_panel_open: Option<bool>,
    pub loadout_swap_increment: Option<u8>,
    pub main_hand_option1_loaded_ammo_data: Option<LoadedAmmoData>,
    pub main_hand_option2_loaded_ammo_data: Option<LoadedAmmoData>,
}

#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ReplicatedState,
    nw_network_derive::AzRtti,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("B258A60E-FC21-40CF-8B86-57B7F6083D32")]
#[type_registry(3183)]
pub struct PaperdollComponentReplicatedState {
    #[replicated_state(group = 1)]
    pub visible_durability: ReplicatedVec<u32>,
    #[replicated_state(group = 1)]
    pub visible_full_item_data: ReplicatedVec<PaperdollItemDescriptor>,
    #[replicated_state(group = 1)]
    pub non_visible_full_item_data: ReplicatedVec<PaperdollItemDescriptor>,
    #[replicated_state(group = 1)]
    pub non_visible_paperdoll_slots: ReplicatedVec<u32>,
    #[replicated_state(group = 1)]
    pub non_visible_durability: ReplicatedVec<u32>,
    #[replicated_state(group = 1)]
    pub bonus_equip_load: ReplicatedFieldHandler<u16>,
    #[replicated_state(group = 1)]
    pub is_local_player: ReplicatedFieldHandler<bool>,
    #[replicated_state(group = 1)]
    pub is_loadout_panel_open: ReplicatedFieldHandler<bool>,
    #[replicated_state(group = 1)]
    pub main_hand_option1_loaded_ammo_data: ReplicatedFieldHandler<LoadedAmmoData>,
    #[replicated_state(group = 1)]
    pub main_hand_option2_loaded_ammo_data: ReplicatedFieldHandler<LoadedAmmoData>,
    #[replicated_state(group = 1)]
    pub loadouts: ReplicatedVec<PaperdollLoadout>,
    #[replicated_state(group = 1)]
    pub hide_skins: ReplicatedMap<u32, bool>,
    #[replicated_state(group = 2)]
    pub sheathe_map: ReplicatedFieldHandler<PaperdollSlotFlags>,
    #[replicated_state(group = 2)]
    pub item_slots_attachment_status: ReplicatedFieldHandler<PaperdollSlotFlags>,
    #[replicated_state(group = 2)]
    pub active_map: ReplicatedFieldHandler<PaperdollSlotFlags>,
    #[replicated_state(group = 2)]
    pub visible_paperdoll_slots: ReplicatedVec<u32>,
    #[replicated_state(group = 2)]
    pub visible_item_visual_data: ReplicatedVec<ItemVisualData>,
    #[replicated_state(group = 2)]
    pub loadout_swap_increment: ReplicatedFieldHandler<u8>,

    pub hub: ReplicatedState,
}

impl PaperdollComponentReplicatedState {
    pub fn apply_snapshot(&mut self, snapshot: PaperdollSnapshot) {
        if let Some(values) = snapshot.visible_durability {
            self.visible_durability = values;
        }
        if let Some(values) = snapshot.non_visible_durability {
            self.non_visible_durability = values;
        }
        if let Some(values) = snapshot.visible_paperdoll_slots {
            self.visible_paperdoll_slots = values;
        }
        if let Some(values) = snapshot.non_visible_paperdoll_slots {
            self.non_visible_paperdoll_slots = values;
        }
        if let Some(values) = snapshot.visible_full_item_data {
            self.visible_full_item_data = values;
        }
        if let Some(values) = snapshot.non_visible_full_item_data {
            self.non_visible_full_item_data = values;
        }
        if let Some(values) = snapshot.visible_item_visual_data {
            self.visible_item_visual_data = values;
        }
        if let Some(values) = snapshot.loadouts {
            self.loadouts = values;
        }

        if snapshot.hide_skins {
            self.hide_skins = ReplicatedMap::default();
        }
        if let Some(value) = snapshot.sheathe_map {
            self.sheathe_map.set_value(value);
        }
        if let Some(value) = snapshot.item_slots_attachment_status {
            self.item_slots_attachment_status.set_value(value);
        }
        if let Some(value) = snapshot.active_map {
            self.active_map.set_value(value);
        }
        if let Some(value) = snapshot.bonus_equip_load {
            self.bonus_equip_load.set_value(value);
        }
        if let Some(value) = snapshot.is_local_player {
            self.is_local_player.set_value(value);
        }
        if let Some(value) = snapshot.is_loadout_panel_open {
            self.is_loadout_panel_open.set_value(value);
        }
        if let Some(value) = snapshot.loadout_swap_increment {
            self.loadout_swap_increment.set_value(value);
        }
        if let Some(value) = snapshot.main_hand_option1_loaded_ammo_data {
            self.main_hand_option1_loaded_ammo_data.set_value(value);
        }
        if let Some(value) = snapshot.main_hand_option2_loaded_ammo_data {
            self.main_hand_option2_loaded_ammo_data.set_value(value);
        }
    }

    #[must_use]
    pub fn loadout_delta_for_slots(&self, slot_types: &[u32]) -> Self {
        let mut state = Self::default();
        let non_visible_index = self
            .non_visible_paperdoll_slots
            .has_value()
            .then(|| {
                self.non_visible_paperdoll_slots
                    .values()
                    .iter()
                    .position(|slot| slot_types.contains(slot))
                    .or_else(|| self.non_visible_paperdoll_slots.values().first().map(|_| 0))
            })
            .flatten();

        if let Some(index) = non_visible_index {
            if let Some(values) = Self::single_value(&self.non_visible_full_item_data, index) {
                state.non_visible_full_item_data = values;
            }
            if let Some(values) = Self::single_value(&self.non_visible_paperdoll_slots, index) {
                state.non_visible_paperdoll_slots = values;
            }
            if let Some(values) = Self::single_value(&self.non_visible_durability, index) {
                state.non_visible_durability = values;
            }
        }

        if self.loadouts.has_value() {
            let values = self
                .loadouts
                .values()
                .iter()
                .take(1)
                .map(|loadout| {
                    let mut linked_loadout_items = Vec::new();
                    for slot_type in slot_types {
                        if let Some(item) = loadout
                            .linked_loadout_items
                            .iter()
                            .find(|item| item.slot_type == *slot_type)
                        {
                            linked_loadout_items.push(item.clone());
                        }
                    }
                    if linked_loadout_items.is_empty() {
                        linked_loadout_items = loadout
                            .linked_loadout_items
                            .iter()
                            .take(slot_types.len())
                            .cloned()
                            .collect();
                    }

                    PaperdollLoadout {
                        name: loadout.name.clone(),
                        linked_loadout_items,
                        is_for_game_mode: loadout.is_for_game_mode,
                        attributes: Vec::new(),
                    }
                })
                .collect();
            state.loadouts = Self::indexed_values(self.loadouts.last_modified(), values);
        }
        state
    }

    fn single_value<T>(source: &ReplicatedVec<T>, index: usize) -> Option<ReplicatedVec<T>>
    where
        T: Clone + Marshaler,
    {
        if !source.has_value() {
            return None;
        }
        let value = source.values().get(index)?.clone();
        Some(Self::indexed_values(source.last_modified(), vec![value]))
    }

    fn indexed_values<T>(sequence: impl Into<SequenceNumber>, values: Vec<T>) -> ReplicatedVec<T>
    where
        T: Marshaler,
    {
        ReplicatedVec::new(sequence, values)
    }
}
