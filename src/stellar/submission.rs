/// High-throughput Stellar transaction submission engine
///
/// Orchestrates channel pooling, sequence coordination, fee management,
/// and retry logic for parallelized, resilient transaction submissions.

use crate::stellar::channel_pool::ChannelPool;
use crate::stellar::fee_engine::DynamicFeeEngine;
use crate::stellar::horizon::HorizonClient;
use crate::stellar::retry_state_machine::{RetryStateMachine, RetryState};
use crate::stellar::error::{SubmissionError, SubmissionResult, HorizonErrorCode};
use crate::stellar::models::{
    FeeConfiguration, RetryPolicy, TransactionLogEntry, SubmissionMetrics, ChannelExhaustionAlert,
    ConfirmationDelayAlert,
};
use crate::stellar::metrics::{StellarMetrics, MetricsTimer};

use chrono::{DateTime, Utc, Duration};
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

/// Main submission engine coordinating all components
pub struct StellarSubmissionEngine {
    pool: PgPool,
    issuer_id: Uuid,
    channel_pool: Arc<ChannelPool>,
    fee_engine: Arc<DynamicFeeEngine>,
    horizon_client: Arc<HorizonClient>,
    retry_policy: RetryPolicy,
    metrics: Arc<StellarMetrics>,
    stale_threshold: Duration,
    confirmation_check_interval: std::time::Duration,
}

impl StellarSubmissionEngine {
    /// Create a new submission engine
    pub async fn new(
        pool: PgPool,
        issuer_id: Uuid,
        horizon_url: String,
        fee_config: FeeConfiguration,
        retry_policy: RetryPolicy,
        metrics: Arc<StellarMetrics>,
    ) -> SubmissionResult<Self> {
        let channel_pool = Arc::new(ChannelPool::new(
            pool.clone(),
            issuer_id,
            3, // circuit breaker threshold
            1000, // max in-flight per channel
        ).await?);

        let fee_engine = Arc::new(DynamicFeeEngine::new(fee_config, horizon_url.clone()));
        let horizon_client = Arc::new(HorizonClient::new(horizon_url));

        Ok(Self {
            pool,
            issuer_id,
            channel_pool,
            fee_engine,
            horizon_client,
            retry_policy,
            metrics,
            stale_threshold: Duration::seconds(60), // 4 ledgers
            confirmation_check_interval: std::time::Duration::from_secs(5),
        })
    }

    /// Submit a transaction envelope (XDR)
    pub async fn submit_transaction(
        &self,
        tx_envelope_xdr: &str,
        operation_count: i32,
    ) -> SubmissionResult<TransactionLogEntry> {
        let timer = MetricsTimer::new(self.metrics.submission_duration_seconds.clone());

        // Calculate dynamic fee
        let fee = self.fee_engine.calculate_fee(operation_count).await?;
        let surge_percent = self.fee_engine.get_surge_percent().await?;

        // Reserve sequence and select channel
        let (channel, sequence) = self.channel_pool.reserve_sequence().await?;

        // Create transaction envelope hash (XDR-based)
        let tx_hash = self.compute_tx_hash(tx_envelope_xdr)?;

        // Log transaction in database
        let log_entry = sqlx::query_as::<_, TransactionLogEntry>(
            r#"
            INSERT INTO stellar_transaction_logs (
                issuer_id, channel_id, submission_index, tx_envelope_hash,
                tx_envelope_xdr, submission_fee_stroops, surge_fee_percent,
                submitted_at, created_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, NOW(), NOW())
            RETURNING
                id, issuer_id, channel_id, submission_index, tx_envelope_hash,
                tx_envelope_xdr, submission_fee_stroops, surge_fee_percent,
                submission_attempt, submitted_at, confirmed_at, stellar_ledger_hash,
                stellar_ledger_number, stellar_tx_hash, last_error_code, last_error_reason,
                retry_count, next_retry_at, final_status, failure_reason, created_at
            "#,
        )
        .bind(self.issuer_id)
        .bind(channel.db_id)
        .bind(sequence)
        .bind(&tx_hash)
        .bind(tx_envelope_xdr)
        .bind(fee)
        .bind(surge_percent)
        .fetch_one(&self.pool)
        .await?;

        // Submit to Horizon
        match self.horizon_client.submit_transaction(tx_envelope_xdr).await {
            Ok(horizon_tx) => {
                self.channel_pool.mark_channel_success(channel.db_id).await?;
                self.metrics.tx_submitted_total.inc();

                // Update with stellar hash if immediately available
                if let Some(stellar_hash) = horizon_tx.hash.split('/').last() {
                    let _ = sqlx::query(
                        "UPDATE stellar_transaction_logs SET stellar_tx_hash = $1 WHERE id = $2",
                    )
                    .bind(stellar_hash)
                    .bind(log_entry.id)
                    .execute(&self.pool)
                    .await;
                }

                Ok(log_entry)
            }
            Err(e) => {
                self.metrics.tx_failed_total.inc();
                self.channel_pool.mark_channel_failure(channel.db_id).await?;

                // Classify and record error
                let error_code = self.classify_error(&e);
                if let Some(code) = &error_code {
                    match code {
                        HorizonErrorCode::TxBadSeq => self.metrics.sequence_errors_total.inc(),
                        HorizonErrorCode::TxInsufficientFee => self.metrics.fee_errors_total.inc(),
                        _ if code.is_retryable() => {
                            self.metrics.transient_errors_total.inc()
                        }
                        _ => {}
                    }
                }

                // Update log entry with error
                let _ = sqlx::query(
                    r#"
                    UPDATE stellar_transaction_logs
                    SET last_error_code = $1, last_error_reason = $2, last_error_at = NOW()
                    WHERE id = $3
                    "#,
                )
                .bind(format!("{:?}", error_code))
                .bind(e.to_string())
                .bind(log_entry.id)
                .execute(&self.pool)
                .await;

                Err(e)
            }
        }
    }

    /// Poll for transaction confirmation
    pub async fn poll_confirmation(&self, tx_log_id: Uuid) -> SubmissionResult<bool> {
        let log_entry: TransactionLogEntry = sqlx::query_as(
            "SELECT * FROM stellar_transaction_logs WHERE id = $1",
        )
        .bind(tx_log_id)
        .fetch_one(&self.pool)
        .await?;

        if log_entry.confirmed_at.is_some() {
            return Ok(true);
        }

        if let Some(stellar_hash) = &log_entry.stellar_tx_hash {
            // Check if transaction is on-chain
            if let Some(horizon_tx) = self
                .horizon_client
                .poll_transaction_confirmation(stellar_hash, 10)
                .await?
            {
                let confirmation_delay = Utc::now()
                    .signed_duration_since(log_entry.submitted_at)
                    .num_seconds();

                // Check for confirmation delay alert (> 3 ledgers / 15s)
                if confirmation_delay > 15 {
                    let ledgers_to_confirm = (confirmation_delay / 5) as i32;
                    let _ = sqlx::query(
                        r#"
                        INSERT INTO stellar_confirmation_delay_alerts
                        (tx_log_id, submitted_at, ledgers_to_confirm, confirmation_time_seconds, alert_sent_at, created_at)
                        VALUES ($1, $2, $3, $4, NOW(), NOW())
                        "#,
                    )
                    .bind(tx_log_id)
                    .bind(log_entry.submitted_at)
                    .bind(ledgers_to_confirm)
                    .bind(confirmation_delay as f64)
                    .execute(&self.pool)
                    .await;
                }

                // Update transaction log
                sqlx::query(
                    r#"
                    UPDATE stellar_transaction_logs
                    SET confirmed_at = NOW(), stellar_tx_hash = $1, stellar_ledger_number = $2, final_status = 'confirmed'
                    WHERE id = $3
                    "#,
                )
                .bind(&horizon_tx.hash)
                .bind(horizon_tx.ledger)
                .bind(tx_log_id)
                .execute(&self.pool)
                .await?;

                self.metrics.tx_confirmed_total.inc();
                self.metrics
                    .confirmation_delay_seconds
                    .observe(confirmation_delay as f64);

                return Ok(true);
            }
        }

        // Check for stale transactions
        let age = Utc::now().signed_duration_since(log_entry.submitted_at);
        if age > self.stale_threshold {
            sqlx::query(
                r#"
                UPDATE stellar_transaction_logs
                SET final_status = 'stale', failure_reason = 'confirmation timeout'
                WHERE id = $1
                "#,
            )
            .bind(tx_log_id)
            .execute(&self.pool)
            .await?;

            return Err(SubmissionError::LedgerCloseTimeout { attempts: 10 });
        }

        Ok(false)
    }

    /// Get channel pool statistics
    pub async fn get_pool_stats(&self) -> SubmissionResult<Vec<serde_json::Value>> {
        let stats = self.channel_pool.get_channel_stats().await?;

        let json_stats: Vec<_> = stats
            .iter()
            .map(|s| {
                serde_json::json!({
                    "channel_id": s.channel_id.to_string(),
                    "index": s.index,
                    "account_id": s.account_id,
                    "current_sequence": s.current_sequence,
                    "reserved_sequence": s.reserved_sequence,
                    "in_flight": s.in_flight,
                    "total_submitted": s.total_submitted,
                    "total_successful": s.total_successful,
                    "total_failed": s.total_failed,
                    "consecutive_failures": s.consecutive_failures,
                    "is_circuit_broken": s.is_circuit_broken,
                })
            })
            .collect();

        Ok(json_stats)
    }

    /// Compute transaction hash from XDR envelope
    fn compute_tx_hash(&self, tx_xdr: &str) -> SubmissionResult<String> {
        use sha2::{Sha256, Digest};

        let decoded = base64::decode(tx_xdr)
            .map_err(|e| SubmissionError::InvalidEnvelope(format!("XDR decode failed: {}", e)))?;

        let mut hasher = Sha256::new();
        hasher.update(&decoded);
        let hash = hasher.finalize();

        Ok(format!("{:x}", hash))
    }

    /// Classify Horizon error for metrics
    fn classify_error(&self, error: &SubmissionError) -> Option<HorizonErrorCode> {
        match error {
            SubmissionError::BadSequence(_) => Some(HorizonErrorCode::TxBadSeq),
            SubmissionError::InsufficientFee { .. } => Some(HorizonErrorCode::TxInsufficientFee),
            SubmissionError::MalformedTransaction(_) => Some(HorizonErrorCode::TxMalformed),
            SubmissionError::TransientNetworkError { .. } => Some(HorizonErrorCode::Transient),
            SubmissionError::HorizonApi(msg) => Some(HorizonErrorCode::from_str(msg)),
            _ => None,
        }
    }

    /// Get current metrics snapshot
    pub async fn get_metrics_snapshot(&self) -> SubmissionResult<SubmissionMetrics> {
        let pool_capacity = self.channel_pool.get_pool_capacity_percent().await?;
        let stats = self.channel_pool.get_channel_stats().await?;

        let circuit_broken = stats.iter().filter(|s| s.is_circuit_broken).count();

        Ok(SubmissionMetrics {
            timestamp: Utc::now(),
            throughput_tps: self.metrics.tx_throughput_tps.get(),
            avg_submission_duration_ms: 0.0, // Would need histogram quantile
            current_surge_fee_stroops: self.metrics.current_surge_fee_stroops.get() as i64,
            channel_exhaustion_percent: pool_capacity,
            total_channels_active: stats.len() as u32,
            total_channels_inactive: 0,
            pending_confirmations: 0,
            failed_submissions_24h: self.metrics.tx_failed_total.get_value() as u64,
        })
    }
}

// Helper imports
use base64;
use sha2;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_tx_hash() {
        // This test would require a real transaction XDR
        // Tested in integration tests
    }
}
