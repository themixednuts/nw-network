use crate::serialize::ReplicatedVec;

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("44528310-AAA5-4E1B-B8C1-68135B81CD06")]
#[::nw_network::type_registry(3210)]
pub struct AchievementComponentReplicatedState {
    pub achievements: ReplicatedVec<u8>,
}

impl AchievementComponentReplicatedState {
    pub fn apply_snapshot(&mut self, achievements: ReplicatedVec<u8>) {
        self.achievements = achievements;
    }
}
