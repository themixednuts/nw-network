//! Platform entitlement balance replication.

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
pub use crate::generated::states::EntitlementComponentReplicatedState;
