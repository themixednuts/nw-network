//! Buildable controller completion and committed-resource replication.

use crate::states::inventory::SimpleItemDescriptor;

pub type CommittedResourceValue = SimpleItemDescriptor;
pub use crate::generated::states::BuildableControllerReplicatedState;
