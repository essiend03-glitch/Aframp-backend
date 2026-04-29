/// SLA Monitoring Engine
///
/// Polls Prometheus metrics every 60 seconds and evaluates each enabled SLO.
/// When a breach is detected it:
///   1. Opens a `sla_breach_incidents` row with a forensic context snapshot.
///   2. Fires a Slack/webhook notification to the on-call channel.
///   3. Marks the incident for partner communication.
///
/// Idempotency: a new incident is only opened when no `open` or `investigating`
/// incident already exists for the same SLO.
use crate::sla::repository::SlaRepository;
use chrono::Utc;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::watch;
use tokio::time::{interval, Duration};
use tracing::{error, info, warn};

pub const MONITOR_INTERVAL_SECS: u64 = 60;

pub struct SlaMonitorWorker {
    repo: Arc<SlaRepository>,
    http: reqwest::Client,
    prometheus_url: String,
    slack_webhook: Option<String>,
}

impl SlaMonitorWorker {
    pub fn new(pool: PgPool, http: reqwest::Client) -> Self {
        Self {
            repo: Arc::new(SlaRepository::new(pool)),
            http,
            prometheus_url: std::env::var("PROMETHEUS_URL")
                .unwrap_or_else(|_| "http://localhost:9090".into()),
            slack_webhook: std::env::var("SLA_SLACK_WEBHOOK").ok(),
        }
    }

    pub async fn run(self, mut shutdown_rx: watch::Receiver<bool>) {
        info!("SlaMonitorWorker started (interval={}s)", MONITOR_INTERVAL_SECS);
        let mut ticker = interval(Duration::from_secs(MONITOR_INTERVAL_SECS));
        ticker.tick().await; // fire immediately on startup

        loop {
            tokio::select! {
                _ = shutdown_rx.changed() => {
                    info!("SlaMonitorWorker shutting down");
                    break;
                }
                _ = ticker.tick() => {
                    if let Err(e) = self.run_cycle().await {
                        error!(error = %e, "SlaMonitorWorker cycle failed");
                    }
                }
            }
        }
    }

    async fn run_cycle(&self) -> anyhow::Result<()> {
        let slos = self.repo.list_slos().await?;

        for slo in &slos {
            let observed = match self.query_metric(&slo.metric_name).await {
                Ok(v) => v,
                Err(e) => {
                    warn!(metric = %slo.metric_name, error = %e, "Failed to query metric");
                    continue;
                }
            };

            let threshold: f64 = slo.threshold.to_string().parse().unwrap_or(0.0);
            let breached = match slo.operator.as_str() {
                "lt"  => observed >= threshold,
                "lte" => observed > threshold,
                "gt"  => observed <= threshold,
                "gte" => observed < threshold,
                _     => false,
            };

            if !breached {
                continue;
            }

            // Check for an already-open incident for this SLO
            let open = self.repo.list_open_incidents().await?;
            let already_open = open.iter().any(|i| {
                i.slo_id == slo.id
                    && matches!(i.status.as_str(), "open" | "investigating")
            });
            if already_open {
                continue;
            }

            warn!(
                slo = %slo.name,
                observed,
                threshold,
                "SLA breach detected — opening incident"
            );

            let context = self.build_context_snapshot(slo, observed).await;
            let incident = self
                .repo
                .open_incident(slo.id, observed, threshold, "aframp-backend", context)
                .await?;

            info!(incident_id = %incident.id, slo = %slo.name, "Incident opened");

            // Fire notification (fire-and-forget)
            if let Some(ref webhook) = self.slack_webhook {
                let msg = serde_json::json!({
                    "text": format!(
                        "🚨 *SLA Breach Detected* — `{}`\n\
                         Observed: `{:.4}` | Threshold: `{:.4}` | Severity: `{}`\n\
                         Incident ID: `{}`",
                        slo.name, observed, threshold, slo.severity, incident.id
                    )
                });
                let _ = self.http.post(webhook).json(&msg).send().await;
            }

            // Mark partners notified (status page update hook)
            let _ = self.repo.mark_partners_notified(incident.id).await;
        }

        Ok(())
    }

    /// Query a Prometheus instant vector and return the scalar value.
    async fn query_metric(&self, metric: &str) -> anyhow::Result<f64> {
        let url = format!("{}/api/v1/query", self.prometheus_url);
        let resp: serde_json::Value = self
            .http
            .get(&url)
            .query(&[("query", metric)])
            .send()
            .await?
            .json()
            .await?;

        let value = resp["data"]["result"]
            .as_array()
            .and_then(|r| r.first())
            .and_then(|r| r["value"].as_array())
            .and_then(|v| v.get(1))
            .and_then(|v| v.as_str())
            .and_then(|s| s.parse::<f64>().ok())
            .unwrap_or(0.0);

        Ok(value)
    }

    async fn build_context_snapshot(
        &self,
        slo: &crate::sla::models::SloDefinition,
        observed: f64,
    ) -> serde_json::Value {
        // Collect recent latency trace data from Prometheus
        let latency_trace = self.query_range_metric(&slo.metric_name, 300).await;

        // Collect recent error-rate spike for forensic context
        let error_rate = self
            .query_metric("rate(aframp_http_requests_total{status=~\"5..\"}[5m])")
            .await
            .unwrap_or(0.0);

        serde_json::json!({
            "slo_name": slo.name,
            "metric_name": slo.metric_name,
            "observed_value": observed,
            "threshold": slo.threshold,
            "operator": slo.operator,
            "window_seconds": slo.window_seconds,
            "snapshot_at": Utc::now().to_rfc3339(),
            "prometheus_url": self.prometheus_url,
            // Forensic attachments (Issue #405 — automated root-cause context)
            "latency_trace_5m": latency_trace,
            "error_rate_5m": error_rate,
            "recent_logs_hint": format!(
                "Query logs: service=aframp-backend metric={} window=5m",
                slo.metric_name
            ),
        })
    }

    /// Query a Prometheus range vector and return the last N data points as a JSON array.
    async fn query_range_metric(&self, metric: &str, range_secs: u64) -> serde_json::Value {
        let url = format!("{}/api/v1/query_range", self.prometheus_url);
        let end = Utc::now().timestamp();
        let start = end - range_secs as i64;
        let resp = self
            .http
            .get(&url)
            .query(&[
                ("query", metric),
                ("start", &start.to_string()),
                ("end", &end.to_string()),
                ("step", "30"),
            ])
            .send()
            .await;

        match resp {
            Ok(r) => r.json::<serde_json::Value>().await.unwrap_or(serde_json::Value::Null),
            Err(_) => serde_json::Value::Null,
        }
    }
}
