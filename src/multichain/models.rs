use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SwapStatus {
    Initiated,
    AssetLocked,
    Claimed,
    Refunded,
    TimedOut,
    HeldForManualReconciliation,
}

impl std::fmt::Display for SwapStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            SwapStatus::Initiated => "INITIATED",
            SwapStatus::AssetLocked => "ASSET_LOCKED",
            SwapStatus::Claimed => "CLAIMED",
            SwapStatus::Refunded => "REFUNDED",
            SwapStatus::TimedOut => "TIMED_OUT",
            SwapStatus::HeldForManualReconciliation => "HELD_FOR_MANUAL_RECONCILIATION",
        };
        write!(f, "{s}")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AtomicSwap {
    pub id:              Uuid,
    pub tenant_id:       Uuid,
    pub src_chain_id:    u64,
    pub dst_chain_id:    u64,
    /// H(secret) – keccak256 or sha256 hex string.
    pub hashlock:        String,
    pub timelock_expiry: DateTime<Utc>,
    /// Amount in base units (wei / stroops) as 10^-18 precision string.
    pub amount_wei:      String,
    pub status:          SwapStatus,
    pub src_tx_hash:     Option<String>,
    pub dst_tx_hash:     Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateProof {
    pub swap_id:      Uuid,
    pub chain_id:     u64,
    pub block_number: u64,
    pub state_root:   String,
    pub merkle_proof: Vec<String>,
    pub verified:     bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CrossChainGateway {
    pub id:                 Uuid,
    pub chain_name:         String,
    pub chain_id:           u64,
    pub rpc_endpoint:       String,
    pub htlc_contract_addr: Option<String>,
    pub min_confirmations:  u32,
    pub enabled:            bool,
}
