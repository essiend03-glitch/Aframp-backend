//! Database repository for the Risk Management module — Issue #494.

use crate::risk::models::*;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

pub struct RiskRepository {
    pool: PgPool,
}

impl RiskRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // ── Corridor profiles ─────────────────────────────────────────────────────

    pub async fn list_profiles(&self) -> sqlx::Result<Vec<RiskCorridorProfile>> {
        sqlx::query_as!(
            RiskCorridorProfile,
            "SELECT * FROM risk_corridor_profiles WHERE enabled = TRUE ORDER BY corridor_id"
        )
        .fetch_all(&self.pool)
        .await
    }

    // ── Circuit breaker events ────────────────────────────────────────────────

    pub async fn open_circuit_breaker(
        &self,
        corridor_id: &str,
        scope: &IsolationScope,
        trigger_metric: &str,
        trigger_value: f64,
        trigger_threshold: f64,
    ) -> sqlx::Result<CircuitBreakerEvent> {
        use sqlx::types::BigDecimal;
        use std::str::FromStr;

        let val = BigDecimal::from_str(&trigger_value.to_string()).unwrap_or_default();
        let thr = BigDecimal::from_str(&trigger_threshold.to_string()).unwrap_or_default();

        // Cryptographically sign the trigger payload for audit immutability
        let audit_payload = format!(
            "{}:{}:{}:{}:{}",
            corridor_id,
            scope,
            trigger_metric,
            trigger_value,
            chrono::Utc::now().timestamp()
        );
        let audit_hash = hex::encode(Sha256::digest(audit_payload.as_bytes()));

        sqlx::query_as!(
            CircuitBreakerEvent,
            r#"INSERT INTO circuit_breaker_events
               (corridor_id, scope, trigger_metric, trigger_value, trigger_threshold, audit_hash)
               VALUES ($1, $2, $3, $4, $5, $6)
               RETURNING *"#,
            corridor_id,
            scope.to_string(),
            trigger_metric,
            val,
            thr,
            audit_hash,
        )
        .fetch_one(&self.pool)
        .await
    }

    pub async fn list_active_circuit_breakers(&self) -> sqlx::Result<Vec<CircuitBreakerEvent>> {
        sqlx::query_as!(
            CircuitBreakerEvent,
            "SELECT * FROM circuit_breaker_events WHERE status = 'active' ORDER BY triggered_at DESC"
        )
        .fetch_all(&self.pool)
        .await
    }

    pub async fn add_release_approval(
        &self,
        event_id: Uuid,
        officer_id: &str,
        signature: &str,
    ) -> sqlx::Result<CircuitBreakerEvent> {
        sqlx::query_as!(
            CircuitBreakerEvent,
            r#"UPDATE circuit_breaker_events SET
               release_approvals = release_approvals || $2::jsonb,
               status = CASE
                 WHEN jsonb_array_length(release_approvals || $2::jsonb) >= 2 THEN 'released'
                 ELSE status
               END,
               released_at = CASE
                 WHEN jsonb_array_length(release_approvals || $2::jsonb) >= 2 THEN NOW()
                 ELSE released_at
               END
               WHERE id = $1
               RETURNING *"#,
            event_id,
            serde_json::json!([{"officer": officer_id, "sig": signature, "at": chrono::Utc::now().to_rfc3339()}]),
        )
        .fetch_one(&self.pool)
        .await
    }

    // ── Heartbeats ────────────────────────────────────────────────────────────

    pub async fn record_heartbeat(
        &self,
        bank_id: &str,
        latency_ms: i32,
        status_code: i32,
        error: Option<&str>,
    ) -> sqlx::Result<()> {
        sqlx::query!(
            "INSERT INTO api_health_heartbeats (bank_id, latency_ms, status_code, error) VALUES ($1, $2, $3, $4)",
            bank_id,
            latency_ms,
            status_code,
            error,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn recent_heartbeats(&self, limit: i64) -> sqlx::Result<Vec<ApiHealthHeartbeat>> {
        sqlx::query_as!(
            ApiHealthHeartbeat,
            "SELECT * FROM api_health_heartbeats ORDER BY recorded_at DESC LIMIT $1",
            limit
        )
        .fetch_all(&self.pool)
        .await
    }
}
