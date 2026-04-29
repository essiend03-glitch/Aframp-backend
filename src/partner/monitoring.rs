use std::time::Duration;

/// Holds performance metrics for a specific partner
#[derive(Debug, Clone)]
pub struct PartnerMetrics {
    pub partner_id: String,
    pub average_latency: Duration,
    pub error_rate_percent: f32,
    pub request_count: u64,
}

/// SLA Monitor handles detecting breaches of defined thresholds
pub trait SlaMonitor {
    /// Checks if the partner metrics violate the expected SLA
    fn check_sla_breach(&self, metrics: &PartnerMetrics, threshold_ms: u64) -> bool;

    /// Alerts the partner contact of a degraded service state
    fn trigger_alert(&self, partner_id: &str, message: &str) -> Result<(), String>;
}

/// Health checking via Synthetic Probes
pub trait SyntheticProbe {
    /// Dispatches a test transaction/request to the partner's endpoint
    fn run_probe(&self, endpoint: &str) -> Result<Duration, String>;
}

/// Basic implementation for SLA monitoring
pub struct DefaultSlaMonitor;

impl SlaMonitor for DefaultSlaMonitor {
    fn check_sla_breach(&self, metrics: &PartnerMetrics, threshold_ms: u64) -> bool {
        metrics.average_latency.as_millis() as u64 > threshold_ms || metrics.error_rate_percent > 5.0
    }

    fn trigger_alert(&self, partner_id: &str, message: &str) -> Result<(), String> {
        // Mock sending alert (webhook/email)
        println!("ALERT to Partner {}: {}", partner_id, message);
        Ok(())
    }
}
