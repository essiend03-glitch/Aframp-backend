use prometheus::{register_counter_vec, register_gauge_vec, CounterVec, GaugeVec};
use std::sync::OnceLock;

static PLATFORM_TVL: OnceLock<GaugeVec> = OnceLock::new();
static WEIGHTED_AVG_YIELD: OnceLock<GaugeVec> = OnceLock::new();
static OUTSTANDING_LOANS: OnceLock<GaugeVec> = OnceLock::new();
static AVG_HEALTH_FACTOR: OnceLock<GaugeVec> = OnceLock::new();
static DEFI_REVENUE: OnceLock<GaugeVec> = OnceLock::new();
static SNAPSHOTS_GENERATED: OnceLock<CounterVec> = OnceLock::new();
static CACHE_HITS: OnceLock<CounterVec> = OnceLock::new();
static CACHE_MISSES: OnceLock<CounterVec> = OnceLock::new();
static REPORTS_GENERATED: OnceLock<CounterVec> = OnceLock::new();
static EXPORT_REQUESTS: OnceLock<CounterVec> = OnceLock::new();

fn platform_tvl() -> &'static GaugeVec {
    PLATFORM_TVL.get_or_init(|| {
        register_gauge_vec!("aframp_defi_platform_tvl", "Total DeFi TVL", &[]).unwrap()
    })
}

fn weighted_avg_yield() -> &'static GaugeVec {
    WEIGHTED_AVG_YIELD.get_or_init(|| {
        register_gauge_vec!("aframp_defi_weighted_avg_yield_rate", "Weighted average yield rate", &[]).unwrap()
    })
}

fn outstanding_loans() -> &'static GaugeVec {
    OUTSTANDING_LOANS.get_or_init(|| {
        register_gauge_vec!("aframp_defi_outstanding_loans", "Total outstanding loans", &[]).unwrap()
    })
}

fn avg_health_factor() -> &'static GaugeVec {
    AVG_HEALTH_FACTOR.get_or_init(|| {
        register_gauge_vec!("aframp_defi_avg_lending_health_factor", "Average lending health factor", &[]).unwrap()
    })
}

fn defi_revenue() -> &'static GaugeVec {
    DEFI_REVENUE.get_or_init(|| {
        register_gauge_vec!("aframp_defi_revenue", "DeFi revenue in current period", &[]).unwrap()
    })
}

fn snapshots_generated() -> &'static CounterVec {
    SNAPSHOTS_GENERATED.get_or_init(|| {
        register_counter_vec!("aframp_defi_analytics_snapshots_total", "Analytics snapshots generated", &[]).unwrap()
    })
}

fn cache_hits() -> &'static CounterVec {
    CACHE_HITS.get_or_init(|| {
        register_counter_vec!("aframp_defi_analytics_cache_hits_total", "Analytics cache hits", &["endpoint"]).unwrap()
    })
}

fn cache_misses() -> &'static CounterVec {
    CACHE_MISSES.get_or_init(|| {
        register_counter_vec!("aframp_defi_analytics_cache_misses_total", "Analytics cache misses", &["endpoint"]).unwrap()
    })
}

fn reports_generated() -> &'static CounterVec {
    REPORTS_GENERATED.get_or_init(|| {
        register_counter_vec!("aframp_defi_analytics_reports_total", "Analytics reports generated", &["report_type"]).unwrap()
    })
}

fn export_requests() -> &'static CounterVec {
    EXPORT_REQUESTS.get_or_init(|| {
        register_counter_vec!("aframp_defi_analytics_export_requests_total", "Analytics export requests", &["scope"]).unwrap()
    })
}

pub fn set_platform_tvl(v: f64) { platform_tvl().with_label_values(&[]).set(v); }
pub fn set_weighted_avg_yield_rate(v: f64) { weighted_avg_yield().with_label_values(&[]).set(v); }
pub fn set_total_outstanding_loans(v: f64) { outstanding_loans().with_label_values(&[]).set(v); }
pub fn set_avg_lending_health_factor(v: f64) { avg_health_factor().with_label_values(&[]).set(v); }
pub fn set_defi_revenue(v: f64) { defi_revenue().with_label_values(&[]).set(v); }
pub fn inc_snapshot_generated() { snapshots_generated().with_label_values(&[]).inc(); }
pub fn inc_cache_hit(endpoint: &str) { cache_hits().with_label_values(&[endpoint]).inc(); }
pub fn inc_cache_miss(endpoint: &str) { cache_misses().with_label_values(&[endpoint]).inc(); }
pub fn inc_report_generated(report_type: &str) { reports_generated().with_label_values(&[report_type]).inc(); }
pub fn inc_export_requested(scope: &str) { export_requests().with_label_values(&[scope]).inc(); }
