//! Platform-service replicated state modules.

pub mod entitlement;

pub use crate::generated::states::TwitchStreamReplicatedState;
pub use entitlement::{
    EntitlementBalance, EntitlementComponentReplicatedState, EntitlementSnapshot,
};
