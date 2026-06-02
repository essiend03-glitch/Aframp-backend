//! PEP Metrics Service
//! Tracks and exposes PEP screening and monitoring metrics

use crate::pep::extended_models::{PepMetricsResponse, PepScreeningMetrics};
use chrono::{NaiveDate, Utc};
use prometheus::{
    Counter, CounterVec, Gauge, GaugeVec, Histogram, Opts, Registry,
};
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

/// PEP Metrics Service - tracks all PEP-related metrics
pub struct PepMetricsService {
    registry: Registry,

    // Counters
    pub screens_total: CounterVec,
    pub detections_by_confidence: CounterVec,
    pub edd_initiated_total: CounterVec,
    pub edd_completed_total: CounterVec,
    pub transactions_flagged_total: CounterVec,
    pub transactions_reviewed_total: CounterVec,
    pub false_positives_cleared: CounterVec,

    // Gauges
    pub active_pep_accounts: Gauge,
    pub pep_accounts_pending_edd: Gauge,
    pub transaction_review_queue: Gauge,
    pub days_since_last_db_update: Gauge,

    // Histograms
    pub screening_latency: Histogram,
    pub edd_completion_time: Histogram,
}

impl PepMetricsService {
    pub fn new() -> Result<Self, prometheus::Error> {
        let registry = Registry::new();

        // Initialize counters
        let screens_total = CounterVec::new(
            Opts::new("pep_screens_total", "Total number of PEP screens performed"),
            &["screen_type"], // "initial" or "rescreening"
        )?;

        let detections_by_confidence = CounterVec::new(
            Opts::new("pep_detections_by_confidence", "PEP detections by confidence level"),
            &["confidence"], // "high", "medium", "low"
        )?;

        let edd_initiated_total = CounterVec::new(
            Opts::new("pep_edd_initiated_total", "Total EDD processes initiated"),
            &["edd_type"], // "standard", "simplified", "ongoing"
        )?;

        let edd_completed_total = CounterVec::new(
            Opts::new("pep_edd_completed_total", "Total EDD processes completed"),
            &["outcome"], // "approved", "rejected"
        )?;

        let transactions_flagged_total = CounterVec::new(
            Opts::new("pep_transactions_flagged_total", "Total PEP transactions flagged"),
            &["flag_type"], // "threshold_breach", "unusual_pattern", etc.
        )?;

        let transactions_reviewed_total = CounterVec::new(
            Opts::new("pep_transactions_reviewed_total", "Total PEP transactions reviewed"),
            &["outcome"], // "approved", "cleared", "escalated"
        )?;

        let false_positives_cleared = CounterVec::new(
            Opts::new("pep_false_positives_cleared_total", "Total false positives cleared"),
            &["reason"],
        )?;

        // Initialize gauges
        let active_pep_accounts = Gauge::new(
            "pep_active_accounts",
            "Number of active PEP accounts",
        )?;

        let pep_accounts_pending_edd = Gauge::new(
            "pep_accounts_pending_edd",
            "Number of PEP accounts pending EDD",
        )?;

        let transaction_review_queue = Gauge::new(
            "pep_transaction_review_queue_depth",
            "Number of PEP transactions pending review",
        )?;

        let days_since_last_db_update = Gauge::new(
            "pep_days_since_database_update",
            "Days since last PEP database update",
        )?;

        // Initialize histograms
        let screening_latency = Histogram::new(vec![
            0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 25.0, 50.0,
        ])?;

        let edd_completion_time = Histogram::new(vec![
            1.0, 2.0, 5.0, 10.0, 20.0, 40.0, 80.0, 160.0,
        ])?;

        // Register all metrics
        registry.register(Box::new(screens_total.clone()))?;
        registry.register(Box::new(detections_by_confidence.clone()))?;
        registry.register(Box::new(edd_initiated_total.clone()))?;
        registry.register(Box::new(edd_completed_total.clone()))?;
        registry.register(Box::new(transactions_flagged_total.clone()))?;
        registry.register(Box::new(transactions_reviewed_total.clone()))?;
        registry.register(Box::new(false_positives_cleared.clone()))?;
        registry.register(Box::new(active_pep_accounts.clone()))?;
        registry.register(Box::new(pep_accounts_pending_edd.clone()))?;
        registry.register(Box::new(transaction_review_queue.clone()))?;
        registry.register(Box::new(days_since_last_db_update.clone()))?;
        registry.register(Box::new(screening_latency.clone()))?;
        registry.register(Box::new(edd_completion_time.clone()))?;

        Ok(Self {
            registry,
            screens_total,
            detections_by_confidence,
            edd_initiated_total,
            edd_completed_total,
            transactions_flagged_total,
            transactions_reviewed_total,
            false_positives_cleared,
            active_pep_accounts,
            pep_accounts_pending_edd,
            transaction_review_queue,
            days_since_last_db_update,
            screening_latency,
            edd_completion_time,
        })
    }

    /// Record a PEP screen
    pub fn record_screen(&self, screen_type: &str) {
        self.screens_total
            .with_label_values(&[screen_type])
            .inc();
    }

    /// Record a PEP detection
    pub fn record_detection(&self, confidence: &str) {
        self.detections_by_confidence
            .with_label_values(&[confidence])
            .inc();
    }

    /// Record EDD initiation
    pub fn record_edd_initiated(&self, edd_type: &str) {
        self.edd_initiated_total
            .with_label_values(&[edd_type])
            .inc();
    }

    /// Record EDD completion
    pub fn record_edd_completed(&self, outcome: &str, duration_hours: f64) {
        self.edd_completed_total
            .with_label_values(&[outcome])
            .inc();
        self.edd_completion_time.observe(duration_hours);
    }

    /// Record a flagged transaction
    pub fn record_transaction_flagged(&self, flag_type: &str) {
        self.transactions_flagged_total
            .with_label_values(&[flag_type])
            .inc();
    }

    /// Record a reviewed transaction
    pub fn record_transaction_reviewed(&self, outcome: &str) {
        self.transactions_reviewed_total
            .with_label_values(&[outcome])
            .inc();
    }

    /// Record false positive clearance
    pub fn record_false_positive_cleared(&self, reason: &str) {
        self.false_positives_cleared
            .with_label_values(&[reason])
            .inc();
    }

    /// Update gauge values
    pub fn update_gauges(
        &self,
        active_peps: i64,
        pending_edd: i64,
        queue_depth: i64,
        days_since_update: i64,
    ) {
        self.active_pep_accounts.set(active_peps as f64);
        self.pep_accounts_pending_edd.set(pending_edd as f64);
        self.transaction_review_queue.set(queue_depth as f64);
        self.days_since_last_db_update.set(days_since_update as f64);
    }

    /// Get the registry for Prometheus exposition
    pub fn registry(&self) -> &Registry {
        &self.registry
    }
}

impl Default for PepMetricsService {
    fn default() -> Self {
        Self::new().expect("Failed to create metrics service")
    }
}

// ============================================================================
// Alerting
// ============================================================================

/// Alert conditions for PEP monitoring
#[derive(Debug, Clone)]
pub struct PepAlertConfig {
    /// Alert if queue exceeds this depth
    pub transaction_queue_alert_threshold: i64,
    /// Alert if database not updated within this hours
    pub database_staleness_alert_hours: i64,
    /// Alert on every high-confidence detection
    pub alert_on_high_confidence: bool,
}

impl Default for PepAlertConfig {
    fn default() -> Self {
        Self {
            transaction_queue_alert_threshold: 50,
            database_staleness_alert_hours: 48,
            alert_on_high_confidence: true,
        }
    }
}

/// Check if alerts should be triggered
pub fn check_alerts(
    config: &PepAlertConfig,
    metrics: &PepMetricsResponse,
    last_detection_alert: Option<chrono::DateTime<Utc>>,
) -> Vec<PepAlert> {
    let mut alerts = Vec::new();
    let now = Utc::now();

    // Check transaction queue depth
    if metrics.transaction_review_queue_depth > config.transaction_queue_alert_threshold {
        alerts.push(PepAlert {
            alert_type: AlertType::TransactionQueueDepth,
            severity: AlertSeverity::Warning,
            message: format!(
                "PEP transaction review queue ({} exceeds threshold ({})",
                metrics.transaction_review_queue_depth,
                config.transaction_queue_alert_threshold
            ),
            timestamp: now,
        });
    }

    // Check database staleness
    if let Some(days) = metrics.days_since_last_db_update {
        if days > config.database_staleness_alert_hours / 24 {
            alerts.push(PepAlert {
                alert_type: AlertType::DatabaseStaleness,
                severity: AlertSeverity::Critical,
                message: format!(
                    "PEP database not updated in {} days (max: {})",
                    days,
                    config.database_staleness_alert_hours
                ),
                timestamp: now,
            });
        }
    }

    // Check for overdue EDD renewals
    if metrics.pending_edd_count > 0 {
        alerts.push(PepAlert {
            alert_type: AlertType::EddRenewalOverdue,
            severity: AlertSeverity::Warning,
            message: format!(
                "{} PEP accounts have overdue EDD renewal",
                metrics.pending_edd_count
            ),
            timestamp: now,
        });
    }

    alerts
}

#[derive(Debug, Clone, Serialize)]
pub struct PepAlert {
    pub alert_type: AlertType,
    pub severity: AlertSeverity,
    pub message: String,
    pub timestamp: chrono::DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub enum AlertType {
    HighConfidenceDetection,
    TransactionQueueDepth,
    DatabaseStaleness,
    EddRenewalOverdue,
}

#[derive(Debug, Clone, Serialize)]
pub enum AlertSeverity {
    Info,
    Warning,
    Critical,
}