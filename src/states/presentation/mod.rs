//! Presentation-only replicated state modules.

pub mod audio_proxy;
pub mod notification_service;

pub use crate::generated::states::{
    MarkerComponentReplicatedState, PvPSpectatorCamControllerReplicatedState,
};
pub use audio_proxy::AudioProxyComponentReplicatedState;
pub use notification_service::{
    MAX_NOTIFICATION_CHANGES, NotificationEntry, NotificationServiceComponentReplicatedState,
};
