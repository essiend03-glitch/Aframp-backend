/// Prometheus metrics for Stellar submission engine
use prometheus::{Counter, Gauge, Histogram, Registry, core::Collector};
use std::sync::Arc;

/// Metrics collector for the submission engine
pub struct StellarMetrics {
    // Counters
    pub tx_submitted_total: Counter,
    pub tx_confirmed_total: Counter,
    pub tx_failed_total: Counter,
    pub channel_rotations_total: Counter,
    pub sequence_errors_total: Counter,
    pub fee_errors_total: Counter,
    pub transient_errors_total: Counter,

    // Gauges
    pub tx_throughput_tps: Gauge,
    pub channel_pool_utilization_percent: Gauge,
    pub channels_active: Gauge,
    pub channels_circuit_broken: Gauge,
    pub in_flight_transactions: Gauge,
    pub current_surge_fee_stroops: Gauge,

    // Histograms
    pub submission_duration_seconds: Histogram,
    pub confirmation_delay_seconds: Histogram,
    pub retry_attempts: Histogram,

    registry: Arc<Registry>,
}

impl StellarMetrics {
    /// Create a new metrics collector
    pub fn new(registry: Arc<Registry>) -> prometheus::Result<Self> {
        let tx_submitted_total = Counter::new(
            "stellar_tx_submitted_total",
            "Total Stellar transactions submitted",
        )?;
        registry.register(Box::new(tx_submitted_total.clone()))?;

        let tx_confirmed_total = Counter::new(
            "stellar_tx_confirmed_total",
            "Total Stellar transactions confirmed on-chain",
        )?;
        registry.register(Box::new(tx_confirmed_total.clone()))?;

        let tx_failed_total = Counter::new(
            "stellar_tx_failed_total",
            "Total Stellar transaction submissions failed",
        )?;
        registry.register(Box::new(tx_failed_total.clone()))?;

        let channel_rotations_total = Counter::new(
            "stellar_channel_rotations_total",
            "Total channel rotations due to errors",
        )?;
        registry.register(Box::new(channel_rotations_total.clone()))?;

        let sequence_errors_total = Counter::new(
            "stellar_sequence_errors_total",
            "Total bad sequence errors",
        )?;
        registry.register(Box::new(sequence_errors_total.clone()))?;

        let fee_errors_total = Counter::new(
            "stellar_fee_errors_total",
            "Total insufficient fee errors",
        )?;
        registry.register(Box::new(fee_errors_total.clone()))?;

        let transient_errors_total = Counter::new(
            "stellar_transient_errors_total",
            "Total transient errors (retryable)",
        )?;
        registry.register(Box::new(transient_errors_total.clone()))?;

        let tx_throughput_tps = Gauge::new(
            "stellar_tx_throughput_tps",
            "Current transaction throughput (transactions per second)",
        )?;
        registry.register(Box::new(tx_throughput_tps.clone()))?;

        let channel_pool_utilization_percent = Gauge::new(
            "stellar_channel_pool_utilization_percent",
            "Channel pool utilization percentage (0-100)",
        )?;
        registry.register(Box::new(channel_pool_utilization_percent.clone()))?;

        let channels_active = Gauge::new(
            "stellar_channels_active",
            "Number of active submission channels",
        )?;
        registry.register(Box::new(channels_active.clone()))?;

        let channels_circuit_broken = Gauge::new(
            "stellar_channels_circuit_broken",
            "Number of channels with open circuit breaker",
        )?;
        registry.register(Box::new(channels_circuit_broken.clone()))?;

        let in_flight_transactions = Gauge::new(
            "stellar_in_flight_transactions",
            "Number of in-flight transactions",
        )?;
        registry.register(Box::new(in_flight_transactions.clone()))?;

        let current_surge_fee_stroops = Gauge::new(
            "stellar_surge_fee_stroops",
            "Current Stellar surge fee in stroops",
        )?;
        registry.register(Box::new(current_surge_fee_stroops.clone()))?;

        let submission_duration_seconds = Histogram::new(
            "stellar_submission_duration_seconds",
            "Time spent in transaction submission (seconds)",
        )?;
        registry.register(Box::new(submission_duration_seconds.clone()))?;

        let confirmation_delay_seconds = Histogram::new(
            "stellar_confirmation_delay_seconds",
            "Time from submission to on-chain confirmation (seconds)",
        )?;
        registry.register(Box::new(confirmation_delay_seconds.clone()))?;

        let retry_attempts = Histogram::new(
            "stellar_retry_attempts",
            "Number of retry attempts per transaction",
        )?;
        registry.register(Box::new(retry_attempts.clone()))?;

        Ok(Self {
            tx_submitted_total,
            tx_confirmed_total,
            tx_failed_total,
            channel_rotations_total,
            sequence_errors_total,
            fee_errors_total,
            transient_errors_total,
            tx_throughput_tps,
            channel_pool_utilization_percent,
            channels_active,
            channels_circuit_broken,
            in_flight_transactions,
            current_surge_fee_stroops,
            submission_duration_seconds,
            confirmation_delay_seconds,
            retry_attempts,
            registry,
        })
    }
}

/// Metrics scope guard for timing operations
pub struct MetricsTimer {
    start: std::time::Instant,
    histogram: Histogram,
}

impl MetricsTimer {
    pub fn new(histogram: Histogram) -> Self {
        Self {
            start: std::time::Instant::now(),
            histogram,
        }
    }
}

impl Drop for MetricsTimer {
    fn drop(&mut self) {
        let elapsed = self.start.elapsed().as_secs_f64();
        self.histogram.observe(elapsed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_creation() {
        let registry = Arc::new(Registry::new());
        let metrics = StellarMetrics::new(registry).unwrap();
        
        metrics.tx_submitted_total.inc();
        assert_eq!(metrics.tx_submitted_total.get_value() as i32, 1);
    }

    #[test]
    fn test_metrics_timer() {
        let registry = Arc::new(Registry::new());
        let metrics = StellarMetrics::new(registry).unwrap();
        
        {
            let _timer = MetricsTimer::new(metrics.submission_duration_seconds.clone());
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        
        // Timer should have recorded the observation
        let samples = metrics.submission_duration_seconds.collect();
        assert!(!samples.is_empty());
    }
}
