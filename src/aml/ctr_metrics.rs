//! CTR Prometheus Metrics
//!
//! Provides Prometheus counters and gauges for CTR lifecycle metrics and deadlines.

use lazy_static::lazy_static;
use prometheus::{
    register_counter_vec, register_gauge_vec, register_histogram_vec, CounterVec, GaugeVec,
    HistogramVec,
};

lazy_static! {
    /// Counter for CTRs generated
    pub static ref CTR_GENERATED_TOTAL: CounterVec = register_counter_vec!(
        "ctr_generated_total",
        "Total number of CTRs generated",
        &["ctr_type", "detection_method"]
    )
    .unwrap();

    /// Counter for CTRs filed
    pub static ref CTR_FILED_TOTAL: CounterVec = register_counter_vec!(
        "ctr_filed_total",
        "Total number of CTRs filed",
        &["ctr_type", "filing_method"]
    )
    .unwrap();

    /// Counter for CTR status changes
    pub static ref CTR_STATUS_CHANGE_TOTAL: CounterVec = register_counter_vec!(
        "ctr_status_change_total",
        "Total number of CTR status changes",
        &["from_status", "to_status"]
    )
    .unwrap();

    /// Counter for threshold breaches
    pub static ref CTR_THRESHOLD_BREACH_TOTAL: CounterVec = register_counter_vec!(
        "ctr_threshold_breach_total",
        "Total number of threshold breaches",
        &["subject_type"]
    )
    .unwrap();

    /// Counter for exemptions applied
    pub static ref CTR_EXEMPTION_APPLIED_TOTAL: CounterVec = register_counter_vec!(
        "ctr_exemption_applied_total",
        "Total number of exemptions applied",
        &["exemption_category"]
    )
    .unwrap();

    /// Counter for batch filing operations
    pub static ref CTR_BATCH_FILING_TOTAL: CounterVec = register_counter_vec!(
        "ctr_batch_filing_total",
        "Total number of batch filing operations",
        &["status"]
    )
    .unwrap();

    /// Counter for deadline reminders sent
    pub static ref CTR_DEADLINE_REMINDER_TOTAL: CounterVec = register_counter_vec!(
        "ctr_deadline_reminder_total",
        "Total number of deadline reminders sent",
        &["reminder_type"]
    )
    .unwrap();

    /// Counter for overdue alerts
    pub static ref CTR_OVERDUE_ALERT_TOTAL: CounterVec = register_counter_vec!(
        "ctr_overdue_alert_total",
        "Total number of overdue alerts sent",
        &["alert_type"]
    )
    .unwrap();

    /// Gauge for CTRs by status
    pub static ref CTR_BY_STATUS: GaugeVec = register_gauge_vec!(
        "ctr_by_status",
        "Number of CTRs by status",
        &["status"]
    )
    .unwrap();

    /// Gauge for CTRs by type
    pub static ref CTR_BY_TYPE: GaugeVec = register_gauge_vec!(
        "ctr_by_type",
        "Number of CTRs by type",
        &["ctr_type"]
    )
    .unwrap();

    /// Gauge for overdue CTRs
    pub static ref CTR_OVERDUE: GaugeVec = register_gauge_vec!(
        "ctr_overdue",
        "Number of overdue CTRs",
        &["days_overdue_range"]
    )
    .unwrap();

    /// Gauge for CTRs approaching deadline
    pub static ref CTR_APPROACHING_DEADLINE: GaugeVec = register_gauge_vec!(
        "ctr_approaching_deadline",
        "Number of CTRs approaching deadline",
        &["days_until_deadline"]
    )
    .unwrap();

    /// Gauge for total amount in pending CTRs
    pub static ref CTR_PENDING_AMOUNT_NGN: GaugeVec = register_gauge_vec!(
        "ctr_pending_amount_ngn",
        "Total amount in NGN for pending CTRs",
        &["status"]
    )
    .unwrap();

    /// Histogram for CTR processing time
    pub static ref CTR_PROCESSING_DURATION_SECONDS: HistogramVec = register_histogram_vec!(
        "ctr_processing_duration_seconds",
        "Time taken to process CTR through lifecycle stages",
        &["stage"],
        vec![1.0, 5.0, 10.0, 30.0, 60.0, 300.0, 600.0, 1800.0, 3600.0]
    )
    .unwrap();

    /// Histogram for batch filing duration
    pub static ref CTR_BATCH_FILING_DURATION_SECONDS: HistogramVec = register_histogram_vec!(
        "ctr_batch_filing_duration_seconds",
        "Time taken for batch filing operations",
        &["batch_size_range"],
        vec![1.0, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0]
    )
    .unwrap();

    /// Histogram for filing retry count
    pub static ref CTR_FILING_RETRY_COUNT: HistogramVec = register_histogram_vec!(
        "ctr_filing_retry_count",
        "Number of retries for CTR filing",
        &["final_status"],
        vec![0.0, 1.0, 2.0, 3.0, 4.0, 5.0]
    )
    .unwrap();
}

/// Record CTR generation
pub fn record_ctr_generated(ctr_type: &str, detection_method: &str) {
    CTR_GENERATED_TOTAL
        .with_label_values(&[ctr_type, detection_method])
        .inc();
}

/// Record CTR filed
pub fn record_ctr_filed(ctr_type: &str, filing_method: &str) {
    CTR_FILED_TOTAL
        .with_label_values(&[ctr_type, filing_method])
        .inc();
}

/// Record CTR status change
pub fn record_status_change(from_status: &str, to_status: &str) {
    CTR_STATUS_CHANGE_TOTAL
        .with_label_values(&[from_status, to_status])
        .inc();
}

/// Record threshold breach
pub fn record_threshold_breach(subject_type: &str) {
    CTR_THRESHOLD_BREACH_TOTAL
        .with_label_values(&[subject_type])
        .inc();
}

/// Record exemption applied
pub fn record_exemption_applied(exemption_category: &str) {
    CTR_EXEMPTION_APPLIED_TOTAL
        .with_label_values(&[exemption_category])
        .inc();
}

/// Record batch filing
pub fn record_batch_filing(status: &str, count: usize) {
    CTR_BATCH_FILING_TOTAL
        .with_label_values(&[status])
        .inc_by(count as f64);
}

/// Record deadline reminder
pub fn record_deadline_reminder(reminder_type: &str) {
    CTR_DEADLINE_REMINDER_TOTAL
        .with_label_values(&[reminder_type])
        .inc();
}

/// Record overdue alert
pub fn record_overdue_alert(alert_type: &str) {
    CTR_OVERDUE_ALERT_TOTAL
        .with_label_values(&[alert_type])
        .inc();
}

/// Update CTR status gauge
pub fn update_ctr_by_status(status: &str, count: f64) {
    CTR_BY_STATUS.with_label_values(&[status]).set(count);
}

/// Update CTR type gauge
pub fn update_ctr_by_type(ctr_type: &str, count: f64) {
    CTR_BY_TYPE.with_label_values(&[ctr_type]).set(count);
}

/// Update overdue CTRs gauge
pub fn update_overdue_ctrs(days_range: &str, count: f64) {
    CTR_OVERDUE.with_label_values(&[days_range]).set(count);
}

/// Update approaching deadline gauge
pub fn update_approaching_deadline(days_until: &str, count: f64) {
    CTR_APPROACHING_DEADLINE
        .with_label_values(&[days_until])
        .set(count);
}

/// Update pending amount gauge
pub fn update_pending_amount(status: &str, amount: f64) {
    CTR_PENDING_AMOUNT_NGN
        .with_label_values(&[status])
        .set(amount);
}

/// Record processing duration
pub fn record_processing_duration(stage: &str, duration_seconds: f64) {
    CTR_PROCESSING_DURATION_SECONDS
        .with_label_values(&[stage])
        .observe(duration_seconds);
}

/// Record batch filing duration
pub fn record_batch_filing_duration(batch_size_range: &str, duration_seconds: f64) {
    CTR_BATCH_FILING_DURATION_SECONDS
        .with_label_values(&[batch_size_range])
        .observe(duration_seconds);
}

/// Record filing retry count
pub fn record_filing_retry_count(final_status: &str, retry_count: f64) {
    CTR_FILING_RETRY_COUNT
        .with_label_values(&[final_status])
        .observe(retry_count);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_metrics() {
        record_ctr_generated("individual", "automatic");
        record_threshold_breach("individual");
        record_status_change("draft", "under_review");
    }
}
