//! Proof-of-Reserves (PoR) public API — Issue #297
//!
//! Provides the unauthenticated public endpoint:
//!   GET /v1/transparency/por
//!
//! Returns a signed JSON object containing:
//!   * Current cNGN circulating supply
//!   * Total NGN custodian bank assets
//!   * Collateralization ratio
//!   * Proof-of-Solvency timestamp from the custodian bank
//!   * Ed25519 signature for offline verification
//!
//! The response is cached for 60 seconds (CDN-friendly).

use axum::{
    extract::State,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use tracing::info;

// ── State ─────────────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct PorState {
    pub db: PgPool,
}

// ── DB row ────────────────────────────────────────────────────────────────────

#[derive(sqlx::FromRow)]
struct PorSnapshotRow {
    total_on_chain_supply: sqlx::types::BigDecimal,
    total_bank_assets: sqlx::types::BigDecimal,
    collateralization_ratio: sqlx::types::BigDecimal,
    is_fully_collateralized: bool,
    custodian_solvency_ts: DateTime<Utc>,
    signature: String,
    signing_key: String,
    recorded_at: DateTime<Utc>,
}

#[derive(sqlx::FromRow)]
struct PorBankRow {
    bank_label: String,
    settled_balance: sqlx::types::BigDecimal,
    currency: String,
    balance_as_of: DateTime<Utc>,
}

// ── Response types ────────────────────────────────────────────────────────────

/// Per-bank balance entry in the PoR response.
#[derive(Debug, Serialize, Deserialize)]
pub struct PorBankBalance {
    /// Anonymised label, e.g. "Reserve Vault A".
    pub label: String,
    /// Settled NGN balance (string to preserve precision).
    pub settled_balance: String,
    pub currency: String,
    /// Timestamp from the bank API confirming this balance.
    pub balance_as_of: DateTime<Utc>,
}

/// The canonical Proof-of-Reserves payload.
///
/// This is the object that is signed with the platform's Ed25519 key.
/// Consumers can verify the signature offline using `signing_key`.
#[derive(Debug, Serialize, Deserialize)]
pub struct ProofOfReservesResponse {
    /// Total cNGN in circulation on the Stellar network.
    pub total_on_chain_supply: String,
    /// Total NGN held in regulated custodian bank Reserve Vault Accounts.
    pub total_bank_assets: String,
    /// `(total_bank_assets / total_on_chain_supply) × 100`.
    /// 100.00 = exactly 1:1 backed.
    pub collateralization_ratio: String,
    /// `true` when `collateralization_ratio >= 100.01`.
    pub is_fully_collateralized: bool,
    /// ISO-8601 timestamp from the custodian bank confirming the settled balance
    /// (Proof of Solvency).
    pub custodian_solvency_timestamp: DateTime<Utc>,
    /// ISO-8601 timestamp of this PoR snapshot.
    pub recorded_at: DateTime<Utc>,
    /// Per-bank breakdown of settled balances.
    pub bank_balances: Vec<PorBankBalance>,
    /// Ed25519 signature over the canonical payload bytes.
    pub signature: String,
    /// Hex-encoded Ed25519 public key used to produce `signature`.
    pub signing_key: String,
}

// ── Handler ───────────────────────────────────────────────────────────────────

/// GET /v1/transparency/por
///
/// Public, unauthenticated endpoint.  Returns the most recent signed
/// Proof-of-Reserves snapshot.
pub async fn get_por(State(state): State<Arc<PorState>>) -> Response {
    info!("📊 GET /v1/transparency/por accessed");

    let snap: Option<PorSnapshotRow> = sqlx::query_as(
        r#"
        SELECT total_on_chain_supply, total_bank_assets, collateralization_ratio,
               is_fully_collateralized, custodian_solvency_ts, signature, signing_key,
               recorded_at
        FROM por_snapshots
        ORDER BY recorded_at DESC
        LIMIT 1
        "#,
    )
    .fetch_optional(&state.db)
    .await
    .unwrap_or(None);

    let snap = match snap {
        Some(s) => s,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({
                    "error": "No Proof-of-Reserves snapshot available yet. \
                              The PoR worker runs every 60 minutes."
                })),
            )
                .into_response();
        }
    };

    // Fetch per-bank balances for this snapshot
    let banks: Vec<PorBankRow> = sqlx::query_as(
        r#"
        SELECT bank_label, settled_balance, currency, balance_as_of
        FROM por_bank_balances
        WHERE snapshot_id = (
            SELECT id FROM por_snapshots ORDER BY recorded_at DESC LIMIT 1
        )
        ORDER BY bank_label
        "#,
    )
    .fetch_all(&state.db)
    .await
    .unwrap_or_default();

    let payload = ProofOfReservesResponse {
        total_on_chain_supply: snap.total_on_chain_supply.to_string(),
        total_bank_assets: snap.total_bank_assets.to_string(),
        collateralization_ratio: snap.collateralization_ratio.to_string(),
        is_fully_collateralized: snap.is_fully_collateralized,
        custodian_solvency_timestamp: snap.custodian_solvency_ts,
        recorded_at: snap.recorded_at,
        bank_balances: banks
            .into_iter()
            .map(|b| PorBankBalance {
                label: b.bank_label,
                settled_balance: b.settled_balance.to_string(),
                currency: b.currency,
                balance_as_of: b.balance_as_of,
            })
            .collect(),
        signature: snap.signature,
        signing_key: snap.signing_key,
    };

    (
        StatusCode::OK,
        [
            (
                header::CACHE_CONTROL,
                "public, max-age=60, stale-while-revalidate=30",
            ),
            (header::CONTENT_TYPE, "application/json"),
        ],
        Json(payload),
    )
        .into_response()
}

// ── Router ────────────────────────────────────────────────────────────────────

pub fn por_routes(state: Arc<PorState>) -> Router {
    Router::new()
        .route("/v1/transparency/por", get(get_por))
        .with_state(state)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn por_response_serializes_correctly() {
        let resp = ProofOfReservesResponse {
            total_on_chain_supply: "1000000.000000".to_string(),
            total_bank_assets: "1001000.000000".to_string(),
            collateralization_ratio: "100.100000".to_string(),
            is_fully_collateralized: true,
            custodian_solvency_timestamp: Utc::now(),
            recorded_at: Utc::now(),
            bank_balances: vec![PorBankBalance {
                label: "Reserve Vault A".to_string(),
                settled_balance: "1001000.000000".to_string(),
                currency: "NGN".to_string(),
                balance_as_of: Utc::now(),
            }],
            signature: "aabbcc".to_string(),
            signing_key: "ddeeff".to_string(),
        };

        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("total_on_chain_supply"));
        assert!(json.contains("collateralization_ratio"));
        assert!(json.contains("custodian_solvency_timestamp"));
        assert!(json.contains("is_fully_collateralized"));
        assert!(json.contains("Reserve Vault A"));
    }

    #[test]
    fn por_bank_balance_serializes_correctly() {
        let b = PorBankBalance {
            label: "Reserve Vault B".to_string(),
            settled_balance: "500000.00".to_string(),
            currency: "NGN".to_string(),
            balance_as_of: Utc::now(),
        };
        let json = serde_json::to_string(&b).unwrap();
        assert!(json.contains("Reserve Vault B"));
        assert!(json.contains("500000.00"));
    }
}
