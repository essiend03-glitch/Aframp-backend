/// Example: Using the Stellar High-Throughput Submission Engine
///
/// This example demonstrates how to initialize and use the submission engine
/// for high-volume transaction processing.

#[cfg(all(feature = "database", not(test)))]
pub mod example {
    use aframp_backend::stellar::{
        StellarSubmissionEngine, DynamicFeeEngine, ChannelPool,
        models::{FeeConfiguration, RetryPolicy},
        metrics::StellarMetrics,
        error::SubmissionResult,
    };
    use prometheus::Registry;
    use std::sync::Arc;
    use sqlx::PgPool;
    use uuid::Uuid;

    /// Initialize the Stellar submission engine
    pub async fn initialize_submission_engine(
        pool: PgPool,
        issuer_id: Uuid,
    ) -> SubmissionResult<Arc<StellarSubmissionEngine>> {
        // Create metrics registry
        let registry = Arc::new(Registry::new());
        let metrics = Arc::new(StellarMetrics::new(registry)?);

        // Configure fee engine for surge pricing
        let fee_config = FeeConfiguration {
            base_fee: 100,              // stroops
            min_fee: 100,
            max_fee: 10_000,
            surge_threshold: 0.8,       // 80% network capacity
            surge_multiplier: 1.5,      // 50% increase during surge
            low_capacity_fee: 1_000,
        };

        // Configure retry policy for fault tolerance
        let retry_policy = RetryPolicy {
            max_retries: 5,
            base_backoff_ms: 100,
            max_backoff_ms: 30_000,
            backoff_multiplier: 2.0,
        };

        // Create submission engine
        let engine = Arc::new(StellarSubmissionEngine::new(
            pool,
            issuer_id,
            "https://horizon-testnet.stellar.org".to_string(),
            fee_config,
            retry_policy,
            metrics,
        ).await?);

        Ok(engine)
    }

    /// Example: Submit a single transaction
    pub async fn submit_single_transaction(
        engine: Arc<StellarSubmissionEngine>,
    ) -> SubmissionResult<()> {
        // Assume we have a transaction envelope (XDR-encoded)
        let tx_envelope_xdr = "AAAAAgAAAAB..."; // Real XDR data

        // Submit the transaction
        let tx_log = engine
            .submit_transaction(tx_envelope_xdr, 1) // 1 operation
            .await?;

        println!("Transaction submitted!");
        println!("  ID: {}", tx_log.id);
        println!("  Fee: {} stroops", tx_log.submission_fee_stroops);
        println!("  Sequence: {}", tx_log.submission_index);

        Ok(())
    }

    /// Example: Submit with confirmation polling
    pub async fn submit_and_wait_for_confirmation(
        engine: Arc<StellarSubmissionEngine>,
    ) -> SubmissionResult<()> {
        let tx_envelope_xdr = "AAAAAgAAAAB...";

        // Submit transaction
        let tx_log = engine
            .submit_transaction(tx_envelope_xdr, 1)
            .await?;

        println!("Transaction submitted: {}", tx_log.id);

        // Poll for confirmation
        let mut attempts = 0;
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;

            match engine.poll_confirmation(tx_log.id).await {
                Ok(true) => {
                    println!("✓ Transaction confirmed on-chain!");
                    break;
                }
                Ok(false) => {
                    attempts += 1;
                    println!("  Polling... (attempt {})", attempts);
                    if attempts > 12 {
                        println!("✗ Confirmation timeout");
                        break;
                    }
                }
                Err(e) => {
                    println!("✗ Error polling: {}", e);
                    break;
                }
            }
        }

        Ok(())
    }

    /// Example: High-volume submission loop
    pub async fn high_volume_submission_loop(
        engine: Arc<StellarSubmissionEngine>,
        transactions: Vec<String>,
    ) -> SubmissionResult<()> {
        let mut submission_ids = Vec::new();

        // Submit all transactions
        println!("Submitting {} transactions...", transactions.len());
        for (i, tx_envelope) in transactions.iter().enumerate() {
            match engine.submit_transaction(tx_envelope, 1).await {
                Ok(tx_log) => {
                    submission_ids.push(tx_log.id);
                    if (i + 1) % 100 == 0 {
                        println!("  Submitted {} transactions", i + 1);
                    }
                }
                Err(e) => {
                    eprintln!("  Error on transaction {}: {}", i, e);
                }
            }
        }

        println!("\nMonitoring {} submissions for confirmation...", submission_ids.len());

        // Poll for confirmations in batches
        let mut confirmed = 0;
        for tx_id in submission_ids {
            match engine.poll_confirmation(tx_id).await {
                Ok(true) => confirmed += 1,
                Ok(false) => {} // Still pending
                Err(e) => eprintln!("  Error: {}", e),
            }

            if confirmed % 100 == 0 {
                println!("  Confirmed {} transactions", confirmed);
            }
        }

        println!("\n✓ Submission loop complete!");
        Ok(())
    }

    /// Example: Monitor channel health and performance
    pub async fn monitor_channel_status(
        engine: Arc<StellarSubmissionEngine>,
    ) -> SubmissionResult<()> {
        // Get current channel statistics
        let stats = engine.get_pool_stats().await?;

        println!("Channel Status Report");
        println!("=====================\n");

        for stat in stats {
            let status = if stat["is_circuit_broken"].as_bool().unwrap_or(false) {
                "🔴 BROKEN"
            } else if stat["in_flight"].as_i64().unwrap_or(0) > 900 {
                "🟡 EXHAUSTED"
            } else {
                "🟢 HEALTHY"
            };

            println!("Channel {}: {}", stat["index"], status);
            println!("  Account: {}", stat["account_id"]);
            println!("  In-flight: {}/{}", 
                stat["in_flight"].as_i64().unwrap_or(0),
                1000
            );
            println!("  Success: {}/{}", 
                stat["total_successful"],
                stat["total_submitted"]
            );
            println!(
                "  Failures: {} (consecutive: {})",
                stat["total_failed"],
                stat["consecutive_failures"]
            );
            println!();
        }

        // Get metrics snapshot
        let metrics = engine.get_metrics_snapshot().await?;
        println!("Performance Metrics");
        println!("===================");
        println!("  Throughput: {:.2} TPS", metrics.throughput_tps);
        println!("  Pool Utilization: {:.1}%", metrics.channel_exhaustion_percent);
        println!("  Current Surge Fee: {} stroops", metrics.current_surge_fee_stroops);
        println!("  Failed (24h): {}", metrics.failed_submissions_24h);

        Ok(())
    }

    /// Example: Operator intervention - queue channel top-up
    pub async fn queue_channel_topup(
        pool: sqlx::PgPool,
        channel_index: i32,
        amount_xlm: f64,
    ) -> SubmissionResult<()> {
        let operation_id = uuid::Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO stellar_channel_topup_queue (
                id, channel_index, amount_xlm, status, created_at, updated_at
            ) VALUES ($1, $2, $3, 'pending', NOW(), NOW())
            "#,
        )
        .bind(operation_id)
        .bind(channel_index)
        .bind(sqlx::types::Decimal::from_f64_retain(amount_xlm).unwrap_or_default())
        .execute(&pool)
        .await?;

        println!("✓ Top-up queued: {} XLM for channel {} (op: {})", 
            amount_xlm, channel_index, operation_id);

        Ok(())
    }

    /// Example: Handle submission errors gracefully
    pub async fn submit_with_error_handling(
        engine: Arc<StellarSubmissionEngine>,
        tx_envelope: &str,
    ) -> SubmissionResult<()> {
        match engine.submit_transaction(tx_envelope, 1).await {
            Ok(tx_log) => {
                println!("✓ Submitted: {}", tx_log.id);
            }
            Err(e) => {
                match e {
                    aframp_backend::stellar::error::SubmissionError::NoActiveChannels => {
                        eprintln!("✗ No active channels - alert operator!");
                        // Trigger alert, pause submission queue
                    }
                    aframp_backend::stellar::error::SubmissionError::ChannelExhausted(_) => {
                        eprintln!("✗ All channels exhausted - queue for retry");
                        // Add to retry queue, exponential backoff
                    }
                    aframp_backend::stellar::error::SubmissionError::TransientNetworkError {
                        source,
                        attempt,
                    } => {
                        eprintln!("✗ Transient error (attempt {}): {}", attempt, source);
                        // Will be retried automatically
                    }
                    other => {
                        eprintln!("✗ Error: {}", other);
                    }
                }
            }
        }

        Ok(())
    }

    /// Example: Integration with Aframp payment flow
    pub async fn process_payment_via_stellar(
        engine: Arc<StellarSubmissionEngine>,
        payment: PaymentRequest,
    ) -> SubmissionResult<PaymentResponse> {
        // 1. Validate payment
        validate_payment(&payment)?;

        // 2. Generate transaction envelope
        let tx_envelope = build_stellar_transaction(&payment)?;

        // 3. Submit to Stellar
        let tx_log = engine
            .submit_transaction(&tx_envelope, payment.operations)
            .await?;

        println!("Payment submitted to Stellar: {}", tx_log.id);

        // 4. Return response with tracking info
        Ok(PaymentResponse {
            transaction_id: tx_log.id.to_string(),
            stellar_tx_hash: tx_log.stellar_tx_hash.clone(),
            fee_stroops: tx_log.submission_fee_stroops,
            status: "submitted".to_string(),
        })
    }

    // Helper types
    pub struct PaymentRequest {
        pub from: String,
        pub to: String,
        pub amount: f64,
        pub operations: i32,
    }

    pub struct PaymentResponse {
        pub transaction_id: String,
        pub stellar_tx_hash: Option<String>,
        pub fee_stroops: i64,
        pub status: String,
    }

    // Helper functions (stubs)
    fn validate_payment(payment: &PaymentRequest) -> SubmissionResult<()> {
        if payment.amount <= 0.0 {
            return Err(aframp_backend::stellar::error::SubmissionError::ConfigurationError(
                "Invalid amount".to_string(),
            ));
        }
        Ok(())
    }

    fn build_stellar_transaction(payment: &PaymentRequest) -> SubmissionResult<String> {
        // This would use stellar_sdk to build the actual transaction
        Ok("AAAAAgAAAAB...".to_string())
    }
}

#[cfg(all(feature = "database", not(test)))]
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    use example::*;

    // Initialize
    let pool = sqlx::PgPool::connect(&std::env::var("DATABASE_URL").unwrap()).await?;
    let issuer_id = uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000")?;

    let engine = initialize_submission_engine(pool.clone(), issuer_id).await?;

    // Monitor status
    monitor_channel_status(engine.clone()).await?;

    println!("\nExamples:");
    println!("  - submit_single_transaction()");
    println!("  - submit_and_wait_for_confirmation()");
    println!("  - high_volume_submission_loop()");

    Ok(())
}
