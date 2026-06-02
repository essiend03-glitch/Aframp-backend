//! Prometheus metrics for the commission management engine (Issue #471).

use prometheus::{
    register_counter_vec, register_gauge_vec, register_histogram_vec, CounterVec, GaugeVec,
    HistogramVec,
};
use std::sync::OnceLock;

// ── Counters ─────────────────────────────────────────────────────────────────

static COMMISSION_ACCRUED: OnceLock<CounterVec> = OnceLock::new();
static PAYOUT_TOTAL: OnceLock<CounterVec> = OnceLock::new();
static SPLIT_ERRORS: OnceLock<CounterVec> = OnceLock::new();
static INVARIANT_VIOLATIONS: OnceLock<CounterVec> = OnceLock::new();

// ── Histograms ────────────────────────────────────────────────────────────────

static PAYOUT_DURATION: OnceLock<HistogramVec> = OnceLock::new();

// ── Gauges ────────────────────────────────────────────────────────────────────

static ACCRUED_LIABILITY: OnceLock<GaugeVec> = OnceLock::new();

// ── Accessors ────────────────────────────────────────────────────────────────

fn commission_accrued() -> &'static CounterVec {
    COMMISSION_ACCRUED.get_or_init(|| {
        register_counter_vec!(
            "partner_commission_accrued_total",
            "Total partner commissions accrued in stroops",
            &["partner_id", "corridor"]
        )
        .expect("register partner_commission_accrued_total")
    })
}

fn payout_total() -> &'static CounterVec {
    PAYOUT_TOTAL.get_or_init(|| {
        register_counter_vec!(
            "partner_payout_total",
            "Total commission payouts dispatched",
            &["partner_id", "status"]
        )
        .expect("register partner_payout_total")
    })
}

fn split_errors() -> &'static CounterVec {
    SPLIT_ERRORS.get_or_init(|| {
        register_counter_vec!(
            "revenue_split_calculation_errors_total",
            "Errors during revenue split calculation",
            &["error_type"]
        )
        .expect("register revenue_split_calculation_errors_total")
    })
}

fn invariant_violations_counter() -> &'static CounterVec {
    INVARIANT_VIOLATIONS.get_or_init(|| {
        register_counter_vec!(
            "commission_invariant_violations_total",
            "Fee-split invariant violations (gross != platform + partner)",
            &[]
        )
        .expect("register commission_invariant_violations_total")
    })
}

fn payout_duration() -> &'static HistogramVec {
    PAYOUT_DURATION.get_or_init(|| {
        register_histogram_vec!(
            "partner_payout_duration_seconds",
            "Duration of batch payout processing in seconds",
            &["partner_id"],
            vec![0.1, 0.5, 1.0, 2.0, 5.0, 10.0, 30.0]
        )
        .expect("register partner_payout_duration_seconds")
    })
}

fn accrued_liability() -> &'static GaugeVec {
    ACCRUED_LIABILITY.get_or_init(|| {
        register_gauge_vec!(
            "partner_commission_accrued_liability_stroops",
            "Current unpaid commission liability per partner in stroops",
            &["partner_id"]
        )
        .expect("register partner_commission_accrued_liability_stroops")
    })
}

// ── Public API ────────────────────────────────────────────────────────────────

pub fn commission_evaluated(gross_stroops: i64, partner_stroops: i64) {
    // Fired for every successful split evaluation — not per-partner here
    // (per-partner increments happen in the service layer).
    let _ = (gross_stroops, partner_stroops); // available for future label expansion
}

pub fn commission_accrued(partner_id: &str, corridor: &str, stroops: i64) {
    commission_accrued()
        .with_label_values(&[partner_id, corridor])
        .inc_by(stroops as f64);
}

pub fn payout_dispatched(partner_id: &str, status: &str, stroops: i64) {
    payout_total()
        .with_label_values(&[partner_id, status])
        .inc_by(stroops as f64);
}

pub fn split_error(error_type: &str) {
    split_errors().with_label_values(&[error_type]).inc();
}

pub fn invariant_violation() {
    invariant_violations_counter().with_label_values(&[]).inc();
}

pub fn observe_payout_duration(partner_id: &str, seconds: f64) {
    payout_duration()
        .with_label_values(&[partner_id])
        .observe(seconds);
}

pub fn set_accrued_liability(partner_id: &str, stroops: i64) {
    accrued_liability()
        .with_label_values(&[partner_id])
        .set(stroops as f64);
}
