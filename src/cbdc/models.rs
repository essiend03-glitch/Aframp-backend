use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::{BigDecimal, Uuid};
use std::collections::HashMap;

// ── DLT System Type ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DltSystem {
    HyperledgerBesu,
    Corda,
    Quorum,
    HyperledgerFabric,
}

impl DltSystem {
    pub fn as_str(&self) -> &'static str {
        match self {
            DltSystem::HyperledgerBesu => "Hyperledger Besu",
            DltSystem::Corda => "Corda",
            DltSystem::Quorum => "Quorum",
            DltSystem::HyperledgerFabric => "Hyperledger Fabric",
        }
    }
}

// ── CBDC Gateway ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CbdcGateway {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub dlt_system: String,
    pub network_type: String,
    pub rpc_endpoint: String,
    pub ws_endpoint: Option<String>,
    pub chain_id: Option<i64>,
    pub mtls_certificate_footprint: Option<String>,
    pub mtls_ca_cert_pem: Option<String>,
    pub mtls_client_cert_pem: Option<String>,
    pub node_identity: Option<String>,
    pub connection_timeout_ms: i32,
    pub max_retries: i32,
    pub retry_backoff_ms: i32,
    pub rate_limit_rps: i32,
    pub is_active: bool,
    pub last_health_check_at: Option<DateTime<Utc>>,
    pub last_healthy_at: Option<DateTime<Utc>>,
    pub health_status: String,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct RegisterGatewayRequest {
    pub name: String,
    pub description: Option<String>,
    pub dlt_system: DltSystem,
    pub network_type: Option<String>,
    pub rpc_endpoint: String,
    pub ws_endpoint: Option<String>,
    pub chain_id: Option<i64>,
    pub mtls_ca_cert_pem: Option<String>,
    pub mtls_client_cert_pem: Option<String>,
    pub node_identity: Option<String>,
    pub connection_timeout_ms: Option<i32>,
    pub max_retries: Option<i32>,
    pub metadata: Option<serde_json::Value>,
}

// ── Swap Record ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CbdcSwapRecord {
    pub id: Uuid,
    pub swap_type: String,
    pub status: String,
    pub stellar_transaction_hash: Option<String>,
    pub stellar_asset_code: String,
    pub stellar_asset_issuer: Option<String>,
    pub stellar_amount: BigDecimal,
    pub stellar_source_account: Option<String>,
    pub stellar_destination_account: Option<String>,
    pub stellar_trustline: Option<String>,
    pub stellar_sequence_number: Option<i64>,
    pub stellar_ledger: Option<i64>,
    pub cbdc_gateway_id: Option<Uuid>,
    pub cbdc_transaction_id: Option<String>,
    pub cbdc_block_id: Option<String>,
    pub cbdc_block_number: Option<i64>,
    pub cbdc_confirmations: Option<i32>,
    pub cbdc_sender: Option<String>,
    pub cbdc_recipient: Option<String>,
    pub cbdc_amount: BigDecimal,
    pub cbdc_currency: String,
    pub cbdc_raw_payload: Option<serde_json::Value>,
    pub two_phase_state: String,
    pub two_phase_lock_id: Option<String>,
    pub two_phase_prepared_at: Option<DateTime<Utc>>,
    pub two_phase_committed_at: Option<DateTime<Utc>>,
    pub aml_screening_id: Option<String>,
    pub aml_screening_result: Option<String>,
    pub compliance_metadata: serde_json::Value,
    pub worker_id: Option<String>,
    pub worker_attempts: i32,
    pub worker_last_error: Option<String>,
    pub worker_scheduled_at: Option<DateTime<Utc>>,
    pub worker_completed_at: Option<DateTime<Utc>>,
    pub required_approvals: i32,
    pub current_approvals: i32,
    pub approval_threshold_met: bool,
    pub error_message: Option<String>,
    pub error_code: Option<String>,
    pub idempotency_key: String,
    pub reversal_of: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct InitiateSwapRequest {
    pub swap_type: SwapType,
    pub stellar_asset_code: String,
    pub stellar_asset_issuer: Option<String>,
    pub stellar_amount: BigDecimal,
    pub stellar_destination_account: String,
    pub cbdc_gateway_id: Uuid,
    pub cbdc_recipient: String,
    pub cbdc_currency: String,
    pub cbdc_amount: BigDecimal,
    pub idempotency_key: String,
    pub compliance_metadata: Option<serde_json::Value>,
    pub required_approvals: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SwapType {
    Mint,
    Burn,
    CrossRailSettlement,
}

impl SwapType {
    pub fn as_str(&self) -> &'static str {
        match self {
            SwapType::Mint => "mint",
            SwapType::Burn => "burn",
            SwapType::CrossRailSettlement => "cross_rail_settlement",
        }
    }
}

// ── 2PC Lock ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TwoPcLock {
    pub id: Uuid,
    pub lock_key: String,
    pub swap_record_id: Uuid,
    pub gateway_id: Option<Uuid>,
    pub lock_state: String,
    pub lock_holder: String,
    pub lock_acquired_at: DateTime<Utc>,
    pub lock_expires_at: DateTime<Utc>,
    pub prepared_payload: Option<serde_json::Value>,
    pub commit_payload: Option<serde_json::Value>,
    pub rollback_payload: Option<serde_json::Value>,
    pub node_failure_count: i32,
    pub last_heartbeat_at: Option<DateTime<Utc>>,
    pub recovered_at: Option<DateTime<Utc>>,
    pub error_detail: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ── Signatory Vault ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CryptographicSignatory {
    pub id: Uuid,
    pub swap_record_id: Uuid,
    pub signatory_type: String,
    pub signatory_identity: String,
    pub signing_key_id: Option<String>,
    pub signing_algorithm: String,
    pub signature_value: Option<String>,
    pub signature_payload: Option<String>,
    pub signature_hash: Option<String>,
    pub approval_action: String,
    pub approval_order: i32,
    pub is_required: bool,
    pub approved_at: Option<DateTime<Utc>>,
    pub rejected_at: Option<DateTime<Utc>>,
    pub rejection_reason: Option<String>,
    pub expiry_at: Option<DateTime<Utc>>,
    pub data_residency_region: String,
    pub audit_metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ── API Response Types ─────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct SwapStatusResponse {
    pub id: Uuid,
    pub swap_type: String,
    pub status: String,
    pub two_phase_state: String,
    pub stellar_transaction_hash: Option<String>,
    pub cbdc_transaction_id: Option<String>,
    pub cbdc_block_id: Option<String>,
    pub cbdc_confirmations: Option<i32>,
    pub aml_screening_result: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct GatewayHealthResponse {
    pub id: Uuid,
    pub name: String,
    pub dlt_system: String,
    pub network_type: String,
    pub health_status: String,
    pub is_active: bool,
    pub last_health_check_at: Option<DateTime<Utc>>,
    pub last_healthy_at: Option<DateTime<Utc>>,
    pub metrics: GatewayHealthMetrics,
}

#[derive(Debug, Serialize)]
pub struct GatewayHealthMetrics {
    pub rpc_latency_ms: f64,
    pub block_height: Option<i64>,
    pub peer_count: Option<i32>,
    pub is_syncing: Option<bool>,
}

// ── Worker Configuration ───────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CbdcWorkerConfig {
    pub settlement_poll_interval_secs: u64,
    pub settlement_batch_size: usize,
    pub reversal_retry_interval_secs: u64,
    pub gateway_health_interval_secs: u64,
    pub two_phase_lock_ttl_secs: u64,
    pub two_phase_heartbeat_interval_secs: u64,
    pub max_reversal_attempts: u32,
}

impl Default for CbdcWorkerConfig {
    fn default() -> Self {
        Self {
            settlement_poll_interval_secs: 10,
            settlement_batch_size: 50,
            reversal_retry_interval_secs: 30,
            gateway_health_interval_secs: 60,
            two_phase_lock_ttl_secs: 300,
            two_phase_heartbeat_interval_secs: 15,
            max_reversal_attempts: 5,
        }
    }
}

impl CbdcWorkerConfig {
    pub fn from_env() -> Self {
        Self {
            settlement_poll_interval_secs: std::env::var("CBDC_SETTLEMENT_POLL_INTERVAL_SECS")
                .ok().and_then(|v| v.parse().ok()).unwrap_or(10),
            settlement_batch_size: std::env::var("CBDC_SETTLEMENT_BATCH_SIZE")
                .ok().and_then(|v| v.parse().ok()).unwrap_or(50),
            reversal_retry_interval_secs: std::env::var("CBDC_REVERSAL_RETRY_INTERVAL_SECS")
                .ok().and_then(|v| v.parse().ok()).unwrap_or(30),
            gateway_health_interval_secs: std::env::var("CBDC_GATEWAY_HEALTH_INTERVAL_SECS")
                .ok().and_then(|v| v.parse().ok()).unwrap_or(60),
            two_phase_lock_ttl_secs: std::env::var("CBDC_2PC_LOCK_TTL_SECS")
                .ok().and_then(|v| v.parse().ok()).unwrap_or(300),
            two_phase_heartbeat_interval_secs: std::env::var("CBDC_2PC_HEARTBEAT_INTERVAL_SECS")
                .ok().and_then(|v| v.parse().ok()).unwrap_or(15),
            max_reversal_attempts: std::env::var("CBDC_MAX_REVERSAL_ATTEMPTS")
                .ok().and_then(|v| v.parse().ok()).unwrap_or(5),
        }
    }
}
