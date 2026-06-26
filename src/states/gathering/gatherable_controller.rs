use crate::Marshaler;
use crate::serialize::ReplicatedFieldHandler;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Marshaler)]
#[repr(transparent)]
pub struct ReplicatedGatherableState(u8);

impl ReplicatedGatherableState {
    #[must_use]
    pub const fn new(value: u8) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn value(self) -> u8 {
        self.0
    }
}

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("CF2B3E22-7FDB-4F06-BC1F-7A4B8912CA73")]
#[::nw_network::type_registry(12)]
pub struct GatherableControllerReplicatedState {
    pub gatherable_state: ReplicatedFieldHandler<ReplicatedGatherableState>,
    pub replenish_time: ReplicatedFieldHandler<u64>,
}

impl GatherableControllerReplicatedState {
    #[must_use]
    pub fn new(gatherable_state: u8, replenish_time: u64) -> Self {
        let mut state = Self::default();
        state
            .gatherable_state
            .set_value(ReplicatedGatherableState::new(gatherable_state));
        state.replenish_time.set_value(replenish_time);
        state
    }
}
