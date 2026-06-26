use crate::serialize::{ReplicatedFieldHandler, ReplicatedMap, ReplicatedVec};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EntitlementBalance {
    pub currency_id: u32,
    pub balance: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EntitlementSnapshot {
    pub entitlements_sequence: u64,
    pub balances_sequence: u64,
    pub entitlements: Vec<u8>,
    pub balances: Vec<EntitlementBalance>,
}

#[::nw_network::replicated_state]
#[derive(Debug, Clone, Default)]
#[::nw_network::az_rtti("FEAFABE8-6219-4C4A-9269-261D1E76878E")]
#[::nw_network::type_registry(3133)]
pub struct EntitlementComponentReplicatedState {
    pub entitlements: ReplicatedVec<u8, 0x23f>,
    pub balances: ReplicatedMap<u32, u32, 1000>,
    pub entitlements_received: ReplicatedFieldHandler<bool>,
}

impl EntitlementComponentReplicatedState {
    pub fn apply_snapshot(&mut self, snapshot: EntitlementSnapshot) {
        self.entitlements =
            ReplicatedVec::new(snapshot.entitlements_sequence, snapshot.entitlements);
        self.balances = ReplicatedMap::new(
            snapshot.balances_sequence,
            snapshot
                .balances
                .into_iter()
                .map(|entry| (entry.currency_id, entry.balance))
                .collect(),
        );
    }
}
