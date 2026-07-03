//! Platform entitlement balance replication.

use crate::{az_rtti, replicated_state, type_registry};

use crate::serialize::{IndexMap, ReplicatedContainer, ReplicatedFieldHandler};

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

#[replicated_state]
#[derive(Debug, Clone, Default)]
#[az_rtti("FEAFABE8-6219-4C4A-9269-261D1E76878E")]
#[type_registry(3133)]
pub struct EntitlementComponentReplicatedState {
    pub entitlements: ReplicatedContainer<Vec<u8>, 0x23f>,
    pub balances: ReplicatedContainer<IndexMap<u32, u32>, 1000>,
    pub entitlements_received: ReplicatedFieldHandler<bool>,
}

impl EntitlementComponentReplicatedState {
    pub fn apply_snapshot(&mut self, snapshot: EntitlementSnapshot) {
        self.entitlements =
            ReplicatedContainer::new(snapshot.entitlements_sequence, snapshot.entitlements);
        self.balances = ReplicatedContainer::new(
            snapshot.balances_sequence,
            snapshot
                .balances
                .into_iter()
                .map(|entry| (entry.currency_id, entry.balance))
                .collect(),
        );
    }
}
