//! Grit resource replication.

use crate::serialize::{HalfF32Marshaler, ReplicatedFieldHandler};

pub type GritHalfFloatField = ReplicatedFieldHandler<f32, HalfF32Marshaler>;
pub use crate::generated::states::GritReplicatedState;
