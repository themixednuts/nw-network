//! General and conditional cooldown timer replication.

use crate::{az_rtti, replicated_state, type_registry};

use crate::Marshaler;
use crate::serialize::{IndexMap, ReplicatedContainer, ReplicatedFieldHandler};

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
    pub data_1: u32,
    pub data_2: u32,
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
    pub cooldown_map_1: ReplicatedContainer<IndexMap<u32, CooldownTimerWindow>>,
    pub cooldown_map_2: ReplicatedContainer<IndexMap<u32, CooldownTimerWindow>>,
    pub cooldown_map_3: ReplicatedContainer<IndexMap<u32, CooldownTimerWindow>>,
    pub conditional_cooldowns: ReplicatedContainer<IndexMap<u32, ConditionalCooldownData>>,
    pub general_cooldowns: ReplicatedContainer<Vec<ReplicatedGeneralCooldown>>,
    pub next_daily_cooldown_micros: u64,
    pub next_weekly_cooldown_micros: u64,
}

#[replicated_state]
#[derive(Debug, Clone, Default)]
#[az_rtti("6D45EB20-95FE-4420-96C5-3F9367A3FC5C")]
#[type_registry(2932)]
pub struct CooldownTimersComponentReplicatedState {
    pub cooldown_map_1: ReplicatedContainer<IndexMap<u32, CooldownTimerWindow>>,
    pub cooldown_map_2: ReplicatedContainer<IndexMap<u32, CooldownTimerWindow>>,
    pub cooldown_map_3: ReplicatedContainer<IndexMap<u32, CooldownTimerWindow>>,
    pub conditional_cooldowns: ReplicatedContainer<IndexMap<u32, ConditionalCooldownData>>,
    pub general_cooldowns: ReplicatedContainer<Vec<ReplicatedGeneralCooldown>>,
    pub next_daily_cooldown: ReplicatedFieldHandler<u64>,
    pub next_weekly_cooldown: ReplicatedFieldHandler<u64>,
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
}
