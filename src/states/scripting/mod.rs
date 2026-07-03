//! Scripted-encounter replicated state modules.

pub mod slayer_script;

pub use slayer_script::{
    InstancedSlayerScriptReplicatedState, InstancedSlayerScriptSnapshot,
    SlayerScriptReplicatedState,
};
