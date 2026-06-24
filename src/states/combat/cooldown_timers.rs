use crate::hub::ReplicatedState;
use crate::serialize::{
    MarshalerError, MaskChain, ReadBuffer, ReplicatedFieldHandler, ReplicatedMap, VlqU64,
    WriteBuffer,
};
use crate::{GeneralCooldownType, Marshaler};

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct CooldownTimerWindow {
    pub starts_at: u64,
    pub expires_at: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct ConditionalCooldownData {
    pub expires_at: u64,
    pub cooldown_crc: u32,
    pub count: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Marshaler)]
pub struct ReplicatedGeneralCooldown {
    pub cooldown_type: GeneralCooldownType,
    pub cooldown_crc: u32,
    pub expires_at: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CooldownMapKind {
    Map1,
    Map2,
    Map3,
}

impl TryFrom<u8> for CooldownMapKind {
    type Error = ();

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Map1),
            2 => Ok(Self::Map2),
            3 => Ok(Self::Map3),
            _ => Err(()),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CooldownTimerSnapshot {
    pub cooldown_map_1: ReplicatedMap<u32, CooldownTimerWindow>,
    pub cooldown_map_2: ReplicatedMap<u32, CooldownTimerWindow>,
    pub cooldown_map_3: ReplicatedMap<u32, CooldownTimerWindow>,
    pub conditional_cooldowns: ReplicatedMap<u32, ConditionalCooldownData>,
    pub general_cooldowns: ReplicatedMap<VlqU64, ReplicatedGeneralCooldown>,
    pub next_daily_cooldown_micros: u64,
    pub next_weekly_cooldown_micros: u64,
}

#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ChunkMarshaler,
    nw_network_derive::AzRtti,
    nw_network_derive::Fragment,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("6D45EB20-95FE-4420-96C5-3F9367A3FC5C")]
#[type_registry(2932)]
pub struct CooldownTimersComponentReplicatedState {
    pub cooldown_map_1: ReplicatedMap<u32, CooldownTimerWindow>,
    pub cooldown_map_2: ReplicatedMap<u32, CooldownTimerWindow>,
    pub cooldown_map_3: ReplicatedMap<u32, CooldownTimerWindow>,
    pub conditional_cooldowns: ReplicatedMap<u32, ConditionalCooldownData>,
    pub general_cooldowns: ReplicatedMap<VlqU64, ReplicatedGeneralCooldown>,
    pub next_daily_cooldown: ReplicatedFieldHandler<u64>,
    pub next_weekly_cooldown: ReplicatedFieldHandler<u64>,
    pub hub: ReplicatedState,
}

impl CooldownTimersComponentReplicatedState {
    pub fn apply_snapshot(&mut self, snapshot: CooldownTimerSnapshot) {
        self.cooldown_map_1 = snapshot.cooldown_map_1;
        self.cooldown_map_2 = snapshot.cooldown_map_2;
        self.cooldown_map_3 = snapshot.cooldown_map_3;
        self.conditional_cooldowns = snapshot.conditional_cooldowns;
        self.general_cooldowns = snapshot.general_cooldowns;
        self.next_daily_cooldown
            .set_value(snapshot.next_daily_cooldown_micros);
        self.next_weekly_cooldown
            .set_value(snapshot.next_weekly_cooldown_micros);
    }

    fn unmarshal_fields(&mut self, rb: &mut ReadBuffer) -> Result<(), MarshalerError> {
        let descriptor_mask = rb.read_u8()?;
        if (descriptor_mask & 0x01) == 0 {
            return Ok(());
        }

        let masks = MaskChain::unmarshal(rb)?;
        if masks.is_field_set(0) {
            self.cooldown_map_1 = ReplicatedMap::<u32, CooldownTimerWindow>::unmarshal(rb)?;
        }
        if masks.is_field_set(1) {
            self.cooldown_map_2 = ReplicatedMap::<u32, CooldownTimerWindow>::unmarshal(rb)?;
        }
        if masks.is_field_set(2) {
            self.cooldown_map_3 = ReplicatedMap::<u32, CooldownTimerWindow>::unmarshal(rb)?;
        }
        if masks.is_field_set(3) {
            self.conditional_cooldowns =
                ReplicatedMap::<u32, ConditionalCooldownData>::unmarshal(rb)?;
        }
        if masks.is_field_set(4) {
            self.general_cooldowns =
                ReplicatedMap::<VlqU64, ReplicatedGeneralCooldown>::unmarshal(rb)?;
        }
        if masks.is_field_set(5) {
            self.next_daily_cooldown = ReplicatedFieldHandler::<u64>::unmarshal(rb)?;
        }
        if masks.is_field_set(6) {
            self.next_weekly_cooldown = ReplicatedFieldHandler::<u64>::unmarshal(rb)?;
        }

        Ok(())
    }

    fn marshal_fields(&self, wb: &mut WriteBuffer) {
        let dirty = [
            self.cooldown_map_1.is_dirty(),
            self.cooldown_map_2.is_dirty(),
            self.cooldown_map_3.is_dirty(),
            self.conditional_cooldowns.is_dirty(),
            self.general_cooldowns.is_dirty(),
            self.next_daily_cooldown.is_dirty(),
            self.next_weekly_cooldown.is_dirty(),
        ];
        let any_dirty = dirty.iter().any(|dirty| *dirty);
        wb.write_u8(u8::from(any_dirty));
        if !any_dirty {
            return;
        }

        MaskChain::from_dirty_fields(&dirty).marshal(wb);
        if self.cooldown_map_1.is_dirty() {
            self.cooldown_map_1.marshal(wb);
        }
        if self.cooldown_map_2.is_dirty() {
            self.cooldown_map_2.marshal(wb);
        }
        if self.cooldown_map_3.is_dirty() {
            self.cooldown_map_3.marshal(wb);
        }
        if self.conditional_cooldowns.is_dirty() {
            self.conditional_cooldowns.marshal(wb);
        }
        if self.general_cooldowns.is_dirty() {
            self.general_cooldowns.marshal(wb);
        }
        if self.next_daily_cooldown.is_dirty() {
            self.next_daily_cooldown.marshal(wb);
        }
        if self.next_weekly_cooldown.is_dirty() {
            self.next_weekly_cooldown.marshal(wb);
        }
    }
}

crate::impl_hub_fragment!(
    CooldownTimersComponentReplicatedState,
    hub = hub,
    marshal = marshal_fields,
    unmarshal = unmarshal_fields,
);
