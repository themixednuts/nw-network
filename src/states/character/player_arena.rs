//! Player arena, dungeon, and trial participation state.
//!
//! This is a single-group state: every field is registered in group 0 in
//! declaration order. It combines queue/presence flags, per-dungeon rankings,
//! entry counters, refresh timers, and single-player instance state so clients
//! can render activity availability without additional state fragments.

/// Replicated arena and dungeon participation state.
pub use crate::generated::states::PlayerArenaReplicatedState;
