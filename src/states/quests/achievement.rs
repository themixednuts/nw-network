use crate::hub::ReplicatedState;
use crate::serialize::ReplicatedVec;

#[derive(
    Debug,
    Clone,
    Default,
    nw_network_derive::ReplicatedState,
    nw_network_derive::AzRtti,
    nw_network_derive::TypeRegistry,
)]
#[az_rtti("44528310-AAA5-4E1B-B8C1-68135B81CD06")]
#[type_registry(3210)]
pub struct AchievementComponentReplicatedState {
    pub achievements: ReplicatedVec<u8>,

    pub hub: ReplicatedState,
}

impl AchievementComponentReplicatedState {
    pub fn apply_snapshot(&mut self, achievements: ReplicatedVec<u8>) {
        self.achievements = achievements;
    }
}
