//! Combat, resource, ability, projectile, and status-effect replicated states.

pub mod ability;
pub mod attribute;
pub mod boss_phase;
pub mod combat_status;
pub mod cooldown_timers;
pub mod damage_receiver;
pub mod grit;
pub mod projectile;
pub mod reaction_tracking;
pub mod spell;
pub mod stat_multiplier_table;
pub mod status_effect;
pub mod vitals;

pub use crate::generated::states::{
    AbilityInstanceTrackingComponentReplicatedState, ArenaReplicatedState,
    ChargeComponentReplicatedState, ManaComponentReplicatedState, StaminaComponentReplicatedState,
};
pub use ability::{
    AbilityComponentReplicatedState, AbilitySnapshot, AbilityU32Pair, PersistentAbilityData,
    PersistentAbilityEntry,
};
pub use attribute::{
    AttributeBonus, AttributeComponentReplicatedState, AttributeSnapshot, CharacterAttributeValue,
    CharacterAttributes, PersistentAttributeData,
};
pub use boss_phase::BossPhaseComponentReplicatedState;
pub use combat_status::CombatStatusComponentReplicatedState;
pub use cooldown_timers::{
    ConditionalCooldownData, CooldownMapKind, CooldownTimerSnapshot, CooldownTimerWindow,
    CooldownTimersComponentReplicatedState, ReplicatedGeneralCooldown,
};
pub use damage_receiver::DamageReceiverComponentReplicatedState;
pub use grit::{GritHalfFloatField, GritReplicatedState};
pub use projectile::{PiercingHitData, ProjectileReplicatedState};
pub use reaction_tracking::{ReactionHalfVec3, ReactionTrackingReplicatedState};
pub use spell::SpellComponentReplicatedState;
pub use stat_multiplier_table::{
    StatMultiplierSnapshot, StatMultiplierTableComponentReplicatedState, StatMultiplierValue,
};
pub use status_effect::{
    ActiveTrayIconData, DynamicScalingStatusEffectData, LightweightStatusEffectData,
    RemoteStatusEffectData, StatusEffectInstanceData, StatusEffectsComponentReplicatedState,
    StatusEffectsSnapshot, TerritoryStatusEffect,
};
pub use vitals::{
    ColdAfflictionData, HotAfflictionData, VitalsComponentReplicatedState, VitalsStateData,
};
