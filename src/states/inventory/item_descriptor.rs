use arrayvec::ArrayVec;
use uuid::Uuid;

use crate::Marshaler;
use crate::serialize::{VlqU16, VlqU32};

/// Generated serialization shape.
#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct ReplicatedItemDescriptor {
    pub packed_item_id: u64,
    pub item_key_name: VlqU32,
    pub item_instance_id: VlqU32,
    pub item_count: VlqU16,
    pub item_source: VlqU16,
    pub storage_actor_id_a: VlqU16,
    pub storage_actor_id_b: VlqU16,
    pub storage_actor_id_c: VlqU16,
    pub paperdoll_slot: VlqU16,
    pub is_non_removable_from_player: u8,
    pub is_bound_to_player: u8,
    pub is_bind_on_pickup: u8,
    pub is_bind_on_equip: u8,
    pub descriptor_uuid: Uuid,
    pub descriptor_flags: u8,
    pub durability: u32,
    pub max_durability: u8,
    pub gear_score_tier: u8,
    pub gear_score_bucket: u8,
    pub gear_score_flags: u8,
}

/// Generated serialization shape.
#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "generated serialization shape keeps independent bool fields in wire order"
)]
pub struct SimpleItemDescriptor {
    pub field_10: u32,
    pub field_0e: u16,
    pub field_14: u32,
    pub field_0c: bool,
    pub field_50: u32,
    pub field_54: u32,
    pub field_0b: bool,
    pub field_09: bool,
    pub field_18: ArrayVec<u32, 5>,
    pub field_80: ArrayVec<u32, 5>,
    pub field_0a: bool,
    pub field_38: ArrayVec<u32, 3>,
}
