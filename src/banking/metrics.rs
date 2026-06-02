//! Banking Integration Metrics & Observability
//! Tracks API latency, webhook ingestion, and settlement mismatches

use prometheus::{
    Counter, CounterVec, Gauge, GaugeVec, Histogram, HistogramVec, Opts, Registry,
};
use std::sync::Arc;

/// Banking Metrics Service
pub struct BankingMetricsService {
    registry: Registry,

    // Counters
    pub bank_api_requests_total: CounterVec,
    pub bank_webhook_ingest_total: CounterVec,
    pub fiat_settlement_total: CounterVec,
    pub fiat_settlement_mismatch_total: Counter,
    pub signature_validation_failures: Counter,

    // Gauges
    pub active_bank_connections: GaugeVec,
    pub webhook_backlog: GaugeVec,
    pub pending_settlements: Gauge,

    // Histograms
    pub bank_api_latency_seconds: HistogramVec,
    pub settlement_processing_seconds: HistogramVec,
}

impl BankingMetricsService {
    pub fn new() -> Result<Self, prometheus::Error> {
        let registry = Registry::new();

        // Initialize counters
        let bank_api_requests_total = CounterVec::new(
            Opts::new(
                "bank_api_requests_total",
                "Total bank API requests made",
            ),
            &["bank_code", "endpoint", "status"],
        )?;

        let bank_webhook_ingest_total = CounterVec::new(
            Opts::new(
                "bank_webhook_ingest_total",
                "Total webhooks ingested from banks",
            ),
            &["bank_code", "event_type", "status"],
        )?;

        let fiat_settlement_total = CounterVec::new(
            Opts::new(
                "fiat_settlement_total",
                "Total fiat settlements processed",
            ),
            &["status"],
        )?;

        let fiat_settlement_mismatch_total = Counter::new(
            "fiat_settlement_mismatch_total",
            "Total settlements with amount mismatches",
        )?;

        let signature_validation_failures = Counter::new(
            "bank_webhook_signature_validation_failures_total",
            "Total webhook signature validation failures",
        )?;

        // Initialize gauges
        let active_bank_connections = GaugeVec::new(
            Opts::new(
                "bank_active_connections",
                "Number of active bank connections",
            ),
            &["bank_code"],
        )?;

        let webhook_backlog = GaugeVec::new(
            Opts::new(
                "bank_webhook_backlog",
                "Number of webhooks pending processing",
            ),
            &["bank_code"],
        )?;

        let pending_settlements = Gauge::new(
            "fiat_settlement_pending_count",
            "Number of settlements pending processing",
        )?;

        // Initialize histograms
        let bank_api_latency_seconds = HistogramVec::new(
            Opts::new(
                "bank_api_latency_seconds",
                "Bank API request latency in seconds",
            ),
            &["bank_code", "endpoint"],
            vec![0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0],
        )?;

        let settlement_processing_seconds = HistogramVec::new(
            Opts::new(
                "fiat_settlement_processing_seconds",
                "Time to process fiat settlements",
            ),
            &["stage"],
            vec![0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0],
        )?;

        // Register all metrics
        registry.register(Box::new(bank_api_requests_total.clone()))?;
        registry.register(Box::new(bank_webhook_ingest_total.clone()))?;
        registry.register(Box::new(fiat_settlement_total.clone()))?;
        registry.register(Box::new(fiat_settlement_mismatch_total.clone()))?;
        registry.register(Box::new(signature_validation_failures.clone()))?;
        registry.register(Box::new(active_bank_connections.clone()))?;
        registry.register(Box::new(webhook_backlog.clone()))?;
        registry.register(Box::new(pending_settlements.clone()))?;
        registry.register(Box::new(bank_api_latency_seconds.clone()))?;
        registry.register(Box::new(settlement_processing_seconds.clone()))?;

        Ok(Self {
            registry,
            bank_api_requests_total,
            bank_webhook_ingest_total,
            fiat_settlement_total,
            fiat_settlement_mismatch_total,
            signature_validation_failures,
            active_bank_connections,
            webhook_backlog,
            pending_settlements,
            bank_api_latency_seconds,
            settlement_processing_seconds,
        })
    }

    /// Record bank API request
    pub fn record_api_request(
        &self,
        bank_code: &str,
        endpoint: &str,
        status: &str,
    ) {
        self.bank_api_requests_total
            .with_label_values(&[bank_code, endpoint, status])
            .inc();
    }

    /// Record API latency
    pub fn record_api_latency(&self, bank_code: &str, endpoint: &str, latency_secs: f64) {
        self.bank_api_latency_seconds
            .with_label_values(&[bank_code, endpoint])
            .observe(latency_secs);
    }

    /// Record webhook ingestion
    pub fn record_webhook(
        &self,
        bank_code: &str,
        event_type: &str,
        status: &str,
    ) {
        self.bank_webhook_ingest_total
            .with_label_values(&[bank_code, event_type, status])
            .inc();
    }

    /// Record signature validation failure
    pub fn record_signature_failure(&self) {
        self.signature_validation_failures.inc();
    }

    /// Record settlement
    pub fn record_settlement(&self, status: &str) {
        self.fiat_settlement_total
            .with_label_values(&[status])
            .inc();
    }

    /// Record settlement mismatch
    pub fn record_settlement_mismatch(&self) {
        self.fiat_settlement_mismatch_total.inc();
    }

    /// Update active connections
    pub fn set_active_connections(&self, bank_code: &str, count: f64) {
        self.active_bank_connections
            .with_label_values(&[bank_code])
            .set(count);
    }

    /// Update webhook backlog
    pub fn set_webhook_backlog(&self, bank_code: &str, count: f64) {
        self.webhook_backlog
            .with_label_values(&[bank_code])
            .set(count);
    }

    /// Update pending settlements
    pub fn set_pending_settlements(&self, count: f64) {
        self.pending_settlements.set(count);
    }

    /// Record settlement processing time
    pub fn record_settlement_time(&self, stage: &str, secs: f64) {
        self.settlement_processing_seconds
            .with_label_values(&[stage])
            .observe(secs);
    }

    /// Get registry for Prometheus exposition
    pub fn registry(&self) -> &Registry {
        &self.registry
    }
}

impl Default for BankingMetricsService {
    fn default() -> Self {
        Self::new().expect("Failed to create banking metrics service")
    }
}

// ============================================================================
// Structured Logging Helpers
// ============================================================================

/// Structured log context for banking operations
#[derive(Debug, Clone)]
pub struct BankingLogContext {
    pub bank_code: String,
    pub request_id: Option<String>,
    pub account_mask: Option<String>,
    pub transaction_id: Option<String>,
}

impl BankingLogContext {
    pub fn new(bank_code: impl Into<String>) -> Self {
        Self {
            bank_code: bank_code.into(),
            request_id: None,
            account_mask: None,
            transaction_id: None,
        }
    }

    pub fn with_request_id(mut self, id: impl Into<String>) -> Self {
        self.request_id = Some(id.into());
        self
    }

    pub fn with_account_mask(mut self, mask: impl Into<String>) -> Self {
        self.account_mask = Some(mask.into());
        self
    }

    pub fn with_transaction(mut self, tx_id: impl Into<String>) -> Self {
        self.transaction_id = Some(tx_id.into());
        self
    }
}

/// Mask sensitive account number for logging
pub fn mask_account_number(account: &str) -> String {
    if account.len() < 4 {
        "****".to_string()
    } else {
        let len = account.len();
        format!("****{}", &account[len - 4..])
    }
}

/// Alert conditions for banking operations
#[derive(Debug, Clone)]
pub struct BankingAlertConfig {
    /// Alert if API latency exceeds this threshold (seconds)
    pub latency_alert_threshold: f64,
    /// Alert if consecutive signature failures exceed this count
    pub signature_failure_threshold: i32,
    /// Alert if webhook backlog exceeds this count
    pub webhook_backlog_threshold: i32,
    /// Alert if settlement queue exceeds this count
    pub settlement_queue_threshold: i32,
}

impl Default for BankingAlertConfig {
    fn default() -> Self {
        Self {
            latency_alert_threshold: 5.0,
            signature_failure_threshold: 5,
            webhook_backlog_threshold: 100,
            settlement_queue_threshold: 50,
        }
    }
}

/// Check if alert should be triggered
pub fn check_banking_alerts(
    config: &BankingAlertConfig,
    metrics: &BankingMetricsService,
) -> Vec<BankingAlert> {
    let mut alerts = Vec::new();

    // Check for high webhook backlog
    for bank_code in ["044", "058", "011"] {
        // Example bank codes
        let backlog = metrics
            .webhook_backlog
            .with_label_values(&[bank_code])
            .get();
        if backlog as i32 > config.webhook_backlog_threshold {
            alerts.push(BankingAlert {
                alert_type: AlertType::WebhookBacklog,
                severity: AlertSeverity::Warning,
                bank_code: Some(bank_code.to_string()),
                message: format!(
                    "Webhook backlog for {} exceeds threshold ({} > {})",
                    bank_code, backlog, config.webhook_backlog_threshold
                ),
            });
        }
    }

    alerts
}

#[derive(Debug, Clone)]
pub struct BankingAlert {
    pub alert_type: AlertType,
    pub severity: AlertSeverity,
    pub bank_code: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Copy)]
pub enum AlertType {
    ApiServerError,
    WebhookBacklog,
    SignatureValidationFailure,
    SettlementMismatch,
}

#[derive(Debug, Clone, Copy)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}