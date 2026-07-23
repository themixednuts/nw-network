//! Generated network types and protocol glue.

pub mod states {
    #![allow(clippy::type_complexity, dead_code, unused_imports)]

    include!(concat!(
        env!("NW_NETWORK_GENERATED_DIR"),
        "/generated_states.rs"
    ));
}

pub mod messages {
    include!(concat!(
        env!("NW_NETWORK_GENERATED_DIR"),
        "/generated_messages.rs"
    ));

    pub use crate::messages::actor_mover::{
        CheckMovementStatusMsg, CrashMoveActorMsg, CrashMoveActorToHubMsg, MoveActorMsg,
        MoveCoordinatorInitMsg,
    };
}

mod conversions {
    include!(concat!(
        env!("NW_NETWORK_GENERATED_DIR"),
        "/generated_conversions.rs"
    ));
}

pub use states::*;
