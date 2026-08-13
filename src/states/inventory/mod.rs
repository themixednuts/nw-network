//! Inventory, equipment, loot, and item-related replicated state modules.

pub mod container;
pub mod crafting;
pub mod item_descriptor;
pub mod item_generation;
pub mod item_management;
pub mod item_skinning;
pub mod item_transform;
pub mod loot_tracker;
pub mod paperdoll;
pub mod transmog;

pub use crate::generated::states::{
    CurrencyComponentReplicatedState, GlobalStorageComponentReplicatedState,
    LootDropReplicatedState,
};
pub use crate::types::RecipeCooldownData;
pub use container::{
    ContainerComponentReplicatedState, ContainerInventorySettings, ContainerItemClasses,
    ContainerItemDescriptor, ContainerSnapshot,
};
pub use crafting::{
    CraftingComponentReplicatedState, MAX_CRAFTING_GS_BONUSES, MAX_CRAFTING_RECIPE_COOLDOWNS,
};
pub use item_descriptor::{ReplicatedItemDescriptor, SimpleItemDescriptor};
pub use item_generation::ItemGenerationComponentReplicatedState;
pub use item_management::{
    ItemManagementComponentReplicatedState, ItemManagementSnapshot, ItemManagementStorageKey,
    ItemStorageItems,
};
pub use item_skinning::{
    ItemSkinDyeData, ItemSkinningComponentReplicatedState, ItemSkinningSnapshot, SkinDyeEntry,
};
pub use item_transform::{ItemTransformItemDescriptor, ItemTransformSnapshot, OwnedItemEntry};
pub use loot_tracker::{
    LootDivertEntry, LootDivertMapValue, LootLimitStateData, LootRollData,
    LootTrackerComponentReplicatedState, LootTrackerKey, LootTrackerSnapshot, SlayerScriptLootData,
};
pub use paperdoll::{
    ItemVisualData, LinkedLoadoutItem, LoadedAmmoData, LoadoutAttribute,
    PaperdollComponentReplicatedState, PaperdollItemDescriptor, PaperdollLoadout,
    PaperdollSlotFlags, PaperdollSnapshot,
};
pub use transmog::{TransmogComponentReplicatedState, TransmogSnapshot};
