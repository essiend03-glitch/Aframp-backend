# High-Throughput Stellar Transaction Submission Engine

## Overview

The Aframp Stellar submission engine provides a resilient, high-performance transaction submission pipeline optimized for massive transaction spikes across African payment corridors. It supports 50+ transactions per second (TPS) with dynamic fee management, in-memory sequence coordination, and multi-channel account pooling.

## Architecture

### Core Components

#### 1. **Channel Pool** (`channel_pool.rs`)
- Maintains multiple signing accounts (channels) for parallelized transaction submission
- Round-robin load balancing across active channels
- Circuit breaker pattern with configurable failure thresholds
- Independent sequence number tracking per channel

#### 2. **Sequence Coordinator** (`sequence_coordinator.rs`)
- Lock-free atomic counters for sequence number management
- Prevents duplicate sequence exceptions across parallel Tokio threads
- Compare-and-swap (CAS) operations for thread-safe allocation
- Supports concurrent reservation and confirmation of sequences

#### 3. **Dynamic Fee Engine** (`fee_engine.rs`)
- Queries Horizon's `/fee_stats` endpoint for network congestion
- Implements surge pricing with configurable multipliers
- Caches fee stats to avoid excessive Horizon calls
- Automatically adjusts fees to guarantee immediate ledger inclusion

#### 4. **Retry State Machine** (`retry_state_machine.rs`)
- Exponential backoff for transient errors
- Automatic channel rotation on sequence/exhaustion errors
- Stale transaction detection and marking
- Error classification for metrics and alerting

#### 5. **Horizon Client** (`horizon.rs`)
- HTTP wrapper for Stellar Horizon API
- Transaction submission with error classification
- Confirmation polling with exponential backoff
- Account sequence synchronization

#### 6. **Metrics** (`metrics.rs`)
- Prometheus metrics for throughput, latency, and errors
- Real-time channel utilization tracking
- Surge fee and confirmation delay monitoring

## Database Schema

### `stellar_submission_channels`
Tracks channel accounts with:
- Current and reserved sequence numbers
- Balance and capacity thresholds
- Submission statistics (total, successful, failed)
- Circuit breaker state

### `stellar_transaction_logs`
Immutable audit trail with:
- Transaction envelope XDR and hash
- Submission fee and surge percentage
- Stellar ledger hash and transaction hash (on confirmation)
- Retry state and error tracking
- Full audit history

### `stellar_channel_exhaustion_events`
Alerts when channel capacity drops below 30%:
- Timestamp of exhaustion event
- Available vs total slots
- Utilization percentage

### `stellar_confirmation_delay_alerts`
Tracks transactions exceeding 3-ledger (15s) confirmation time:
- Submitted vs confirmed times
- Ledger count to confirmation
- Alert timestamp for monitoring

### `stellar_channel_topup_queue`
Queues operator-initiated balance top-ups:
- Channel to replenish
- Amount in XLM
- Status tracking (pending → processing → completed)

## Usage

### Initialize the Engine

```rust
use aframp_backend::stellar::{
    StellarSubmissionEngine,
    models::{FeeConfiguration, RetryPolicy},
    metrics::StellarMetrics,
};
use prometheus::Registry;
use std::sync::Arc;

// Create metrics registry
let registry = Arc::new(Registry::new());
let metrics = StellarMetrics::new(registry).unwrap();

// Configure fee engine
let fee_config = FeeConfiguration {
    base_fee: 100,              // stroops
    min_fee: 100,
    max_fee: 10_000,
    surge_threshold: 0.8,       // 80% capacity
    surge_multiplier: 1.5,      // 50% increase during surge
    low_capacity_fee: 1_000,
};

// Configure retry policy
let retry_policy = RetryPolicy {
    max_retries: 5,
    base_backoff_ms: 100,
    max_backoff_ms: 30_000,
    backoff_multiplier: 2.0,
};

// Create engine
let engine = StellarSubmissionEngine::new(
    pool,
    issuer_id,
    "https://horizon-testnet.stellar.org".to_string(),
    fee_config,
    retry_policy,
    Arc::new(metrics),
).await?;
```

### Submit a Transaction

```rust
// Submit transaction envelope (XDR-encoded)
let tx_log = engine.submit_transaction(
    &tx_envelope_xdr,
    1  // operation count
).await?;

println!("Submitted: {} with fee {} stroops", 
    tx_log.id, 
    tx_log.submission_fee_stroops);
```

### Poll for Confirmation

```rust
// Poll for on-chain confirmation
loop {
    if engine.poll_confirmation(tx_log.id).await? {
        println!("Confirmed!");
        break;
    }
    tokio::time::sleep(Duration::from_secs(5)).await;
}
```

### Get Channel Status

```rust
let stats = engine.get_pool_stats().await?;
for stat in stats {
    println!("Channel {}: {} in-flight, {} successful, {} failed",
        stat["index"],
        stat["in_flight"],
        stat["total_successful"],
        stat["total_failed"]
    );
}
```

## Admin Endpoints

### GET `/api/v1/admin/infra/stellar/channels`
Returns status of all submission channels:
```json
{
  "success": true,
  "data": [
    {
      "channel_id": "uuid",
      "index": 0,
      "account_id": "GXXX...",
      "balance_xlm": 1000.5,
      "current_sequence": 12345,
      "reserved_sequence": 12445,
      "in_flight_transactions": 100,
      "total_submitted": 50000,
      "total_successful": 49950,
      "total_failed": 50,
      "consecutive_failures": 0,
      "is_circuit_broken": false,
      "status": "healthy"
    }
  ]
}
```

### POST `/api/v1/admin/infra/stellar/channels/:index/top-up`
Queue a top-up for a channel:
```json
{
  "amount_xlm": 500.0,
  "description": "Routine balance replenishment"
}
```

## Performance Characteristics

### Throughput
- **Sustained**: 50+ TPS across 5-10 channels
- **Peak**: 200+ TPS with optimal network conditions
- **Per Channel**: ~5-10 TPS (depends on fee stats queries)

### Latency
- **Submission to Horizon**: 50-200ms (network dependent)
- **Confirmation polling**: 5-30s (depends on network load)
- **Exponential backoff**: 100ms → 200ms → 400ms → ... (max 30s)

### Resource Usage
- **Memory**: ~50MB per 1000 in-flight transactions
- **CPU**: <5% overhead for submission/polling
- **Database**: ~100 rows/second logged (with archival)

## Error Handling

### Retryable Errors
- `TxInsufficientFee`: Retry with higher fee
- `TransientNetworkError`: Exponential backoff + retry
- `StaleLedgerVersion`: Retry with new ledger

### Non-Retryable Errors
- `TxBadSeq`: Rotate to different channel
- `TxMalformed`: Log and mark as failed
- `NoActiveChannels`: Alert operator

### Circuit Breaker
- Opens after 3 consecutive failures per channel
- Prevents cascading failures across pool
- Automatically recovers on successful submission

## Alerts & Monitoring

### Critical Alerts
- **Channel Exhaustion**: Pool capacity < 30%
  - Suggests load > channel throughput
  - Action: Add more channels or reduce load
  
- **Confirmation Delay**: > 3 ledgers (15 seconds)
  - Suggests network congestion or fee underestimation
  - Action: Check Horizon fee_stats, increase base fee
  
- **Circuit Breaker Open**: Channel failures threshold exceeded
  - Suggests persistent issues with channel account
  - Action: Investigate channel balance, sequence state

### Prometheus Metrics
```
stellar_tx_submitted_total          # Cumulative submissions
stellar_tx_confirmed_total          # Cumulative confirmations
stellar_tx_failed_total             # Cumulative failures
stellar_channel_rotations_total     # Rotations due to errors
stellar_sequence_errors_total       # Bad sequence errors
stellar_fee_errors_total            # Insufficient fee errors
stellar_transient_errors_total      # Transient errors

stellar_tx_throughput_tps           # Current TPS
stellar_channel_pool_utilization_percent  # 0-100
stellar_channels_active             # Count of active channels
stellar_channels_circuit_broken     # Count of broken channels
stellar_in_flight_transactions      # Current in-flight count
stellar_surge_fee_stroops           # Current fee

stellar_submission_duration_seconds # Histogram
stellar_confirmation_delay_seconds  # Histogram
stellar_retry_attempts              # Histogram
```

## Testing

### Unit Tests
```bash
cargo test -p aframp_backend -- stellar
```

### Integration Tests
```bash
# Requires DATABASE_URL and test database
cargo test --test stellar_submission_integration -- --ignored --nocapture
```

### Load Testing
```bash
# See load-tests/stellar_submission_load.rs
cargo run --release --example stellar_load_test -- --channels 5 --tps 100 --duration 60
```

## Best Practices

1. **Channel Management**
   - Maintain 5-10 channels for 50+ TPS
   - Monitor balance closely; set alerts at 100 XLM
   - Rotate channels weekly to manage account sequence drift

2. **Fee Management**
   - Set surge multiplier to 1.5-2.0 for high traffic
   - Monitor Horizon fee_stats; adjust base fee quarterly
   - Cache fee stats for 10+ seconds to reduce API calls

3. **Error Handling**
   - Log all errors with full context (hash, sequence, fee)
   - Use circuit breaker thresholds of 3-5 failures
   - Implement operator alert aggregation (avoid spam)

4. **Monitoring**
   - Dashboard: TPS, latency, success rate, channel utilization
   - Alerts: Exhaustion, delays, circuit breaks, confirmation timeouts
   - Weekly review: Failure patterns, fee trends, channel performance

5. **Deployment**
   - Start with 5 channels in testnet
   - Scale to 10+ channels for mainnet production
   - Provision 50-100 XLM per channel for base reserve + operations
   - Test channel rotation procedure monthly

## Recovery Procedures

### High Confirmation Latency
1. Check Horizon fee_stats
2. Increase base fee in FeeConfiguration
3. Manually bump surge multiplier temporarily
4. Monitor for recovery (should see < 3 ledger confirmations)

### Channel Exhaustion
1. Check pool capacity percentage
2. Review in-flight transaction count
3. Reduce submission rate or add channels
4. Monitor recovery

### Circuit Breaker Triggered
1. Check channel balance (may need top-up)
2. Verify channel account sequence on Horizon
3. If sequence mismatch: manual recovery procedure (restart engine)
4. Resume submissions after verification

## Future Enhancements

- [ ] Soroban contract deployment optimization
- [ ] Multi-signature transaction coordination
- [ ] Payment routing with channel selection heuristics
- [ ] Real-time dashboard with WebSocket updates
- [ ] Advanced analytics (fee correlation, time-series forecasting)
