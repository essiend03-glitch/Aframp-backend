/// Domain types for the Capacity Planning & Forecasting Engine.
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ── DB enums ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "forecast_horizon", rename_all = "snake_case")]
pub enum ForecastHorizon {
    Rolling90d,
    Annual12m,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "forecast_metric", rename_all = "snake_case")]
pub enum ForecastMetric {
    Tps,
    StorageGb,
    DbConnections,
    MemoryGb,
    CpuCores,
    ActiveMerchants,
    ActiveAgents,
}

impl ForecastMetric {
    pub fn all() -> &'static [ForecastMetric] {
        &[
            Self::Tps,
            Self::StorageGb,
            Self::DbConnections,
            Self::MemoryGb,
            Self::CpuCores,
            Self::ActiveMerchants,
            Self::ActiveAgents,
        ]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Tps => "Peak TPS",
            Self::StorageGb => "Storage (GB)",
            Self::DbConnections => "DB Connections",
            Self::MemoryGb => "Memory (GB)",
            Self::CpuCores => "CPU Cores",
            Self::ActiveMerchants => "Active Merchants",
            Self::ActiveAgents => "Active Agents",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "capacity_alert_severity", rename_all = "lowercase")]
pub enum CapacityAlertSeverity {
    Warning,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "capacity_alert_resource", rename_all = "snake_case")]
pub enum CapacityAlertResource {
    Storage,
    Tps,
    Memory,
    Cpu,
    DbConnections,
    Cost,
}

// ── DB row types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BusinessMetricRow {
    pub id: Uuid,
    pub metric_date: NaiveDate,
    pub active_merchants: i32,
    pub active_agents: i32,
    pub daily_transactions: i64,
    pub peak_tps: f64,
    pub avg_transaction_size_kb: f64,
    pub api_call_volume: i64,
    pub db_connections_peak: i32,
    pub storage_used_gb: f64,
    pub storage_growth_gb: f64,
    pub avg_cpu_pct: f64,
    pub avg_memory_gb: f64,
    pub corridor_breakdown: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct ResourceConsumptionUnit {
    pub id: Uuid,
    pub model_month: NaiveDate,
    pub cpu_cores_per_1k_tps: f64,
    pub memory_gb_per_1k_tps: f64,
    pub disk_iops_per_1k_tps: f64,
    pub storage_gb_per_1k_tx: f64,
    pub db_connections_per_agent: f64,
    pub db_connections_per_merchant: f64,
    pub memory_mb_per_api_call: f64,
    pub overhead_multiplier: f64,
    pub forecast_accuracy_pct: Option<f64>,
    pub computed_by: String,
    pub notes: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl ResourceConsumptionUnit {
    /// Compute projected resources from business drivers.
    pub fn project(&self, drivers: &BusinessDrivers) -> ProjectedResources {
        let tps = drivers.peak_tps;
        let mult = self.overhead_multiplier;

        let cpu_cores = (tps / 1000.0) * self.cpu_cores_per_1k_tps * mult;
        let memory_gb = (tps / 1000.0) * self.memory_gb_per_1k_tps * mult;
        let storage_gb = (drivers.daily_transactions as f64 / 1000.0)
            * self.storage_gb_per_1k_tx
            * mult;
        let db_connections = ((drivers.active_agents as f64 * self.db_connections_per_agent)
            + (drivers.active_merchants as f64 * self.db_connections_per_merchant))
            * mult;
        let api_memory_gb =
            (drivers.api_call_volume as f64 * self.memory_mb_per_api_call / 1024.0) * mult;

        ProjectedResources {
            peak_tps: tps,
            cpu_cores,
            memory_gb: memory_gb + api_memory_gb,
            storage_gb,
            db_connections: db_connections.ceil() as i32,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CapacityForecast {
    pub id: Uuid,
    pub forecast_date: NaiveDate,
    pub target_date: NaiveDate,
    pub horizon: ForecastHorizon,
    pub metric: ForecastMetric,
    pub predicted_value: f64,
    pub lower_bound: f64,
    pub upper_bound: f64,
    pub actual_value: Option<f64>,
    pub ape_pct: Option<f64>,
    pub model_version: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CapacityScenario {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub transaction_volume_multiplier: f64,
    pub timeframe_months: i32,
    pub new_merchant_chains: i32,
    pub new_agent_count: i32,
    pub projected_peak_tps: Option<f64>,
    pub projected_storage_gb: Option<f64>,
    pub projected_memory_gb: Option<f64>,
    pub projected_cpu_cores: Option<f64>,
    pub projected_db_connections: Option<i32>,
    pub projected_monthly_cost_usd: Option<f64>,
    pub cost_delta_vs_baseline_usd: Option<f64>,
    pub cloud_provider: String,
    pub resource_breakdown: serde_json::Value,
    pub cost_breakdown: serde_json::Value,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CostProjection {
    pub id: Uuid,
    pub projection_month: NaiveDate,
    pub cloud_provider: String,
    pub cpu_cores: f64,
    pub memory_gb: f64,
    pub storage_gb: f64,
    pub db_connections: i32,
    pub cpu_cost_usd: f64,
    pub memory_cost_usd: f64,
    pub storage_cost_usd: f64,
    pub db_cost_usd: f64,
    pub total_cost_usd: f64,
    pub prev_month_cost_usd: Option<f64>,
    pub cost_delta_pct: Option<f64>,
    pub source: String,
    pub scenario_id: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CapacityAlert {
    pub id: Uuid,
    pub resource: CapacityAlertResource,
    pub severity: CapacityAlertSeverity,
    pub projected_breach_date: NaiveDate,
    pub days_until_breach: i32,
    pub current_value: f64,
    pub threshold_value: f64,
    pub projected_value: f64,
    pub message: String,
    pub notified_at: Option<DateTime<Utc>>,
    pub acknowledged_by: Option<String>,
    pub acknowledged_at: Option<DateTime<Utc>>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub review_task_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct QuarterlyReport {
    pub id: Uuid,
    pub quarter: String,
    pub report_date: NaiveDate,
    pub growth_summary: serde_json::Value,
    pub capacity_requirements: serde_json::Value,
    pub recommendations: serde_json::Value,
    pub prev_quarter_accuracy_pct: Option<f64>,
    pub executive_summary: Option<String>,
    pub full_report: serde_json::Value,
    pub generated_by: String,
    pub created_at: DateTime<Utc>,
}

// ── Value objects ─────────────────────────────────────────────────────────────

/// Business drivers used as input to the RCU model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BusinessDrivers {
    pub active_merchants: i32,
    pub active_agents: i32,
    pub daily_transactions: i64,
    pub peak_tps: f64,
    pub api_call_volume: i64,
}

/// Projected technical resource requirements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectedResources {
    pub peak_tps: f64,
    pub cpu_cores: f64,
    pub memory_gb: f64,
    pub storage_gb: f64,
    pub db_connections: i32,
}

/// Cloud provider pricing config (USD/unit/month).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudPricingConfig {
    pub provider: String,
    /// USD per vCPU per month
    pub cpu_usd_per_core: f64,
    /// USD per GB RAM per month
    pub memory_usd_per_gb: f64,
    /// USD per GB storage per month
    pub storage_usd_per_gb: f64,
    /// USD per DB connection per month
    pub db_usd_per_connection: f64,
}

impl CloudPricingConfig {
    pub fn aws() -> Self {
        Self {
            provider: "aws".into(),
            cpu_usd_per_core: 48.0,      // ~c6i.xlarge / 4 cores = $192/mo
            memory_usd_per_gb: 6.0,      // ~$6/GB/mo (r6i family)
            storage_usd_per_gb: 0.10,    // gp3 EBS
            db_usd_per_connection: 0.50, // RDS proxy overhead
        }
    }

    pub fn gcp() -> Self {
        Self {
            provider: "gcp".into(),
            cpu_usd_per_core: 44.0,
            memory_usd_per_gb: 5.5,
            storage_usd_per_gb: 0.08,
            db_usd_per_connection: 0.45,
        }
    }

    pub fn azure() -> Self {
        Self {
            provider: "azure".into(),
            cpu_usd_per_core: 50.0,
            memory_usd_per_gb: 6.5,
            storage_usd_per_gb: 0.12,
            db_usd_per_connection: 0.55,
        }
    }

    pub fn from_name(name: &str) -> Self {
        match name.to_lowercase().as_str() {
            "gcp" => Self::gcp(),
            "azure" => Self::azure(),
            _ => Self::aws(),
        }
    }

    pub fn compute_cost(&self, resources: &ProjectedResources) -> CostBreakdown {
        let cpu = resources.cpu_cores * self.cpu_usd_per_core;
        let memory = resources.memory_gb * self.memory_usd_per_gb;
        let storage = resources.storage_gb * self.storage_usd_per_gb;
        let db = resources.db_connections as f64 * self.db_usd_per_connection;
        CostBreakdown {
            cpu_cost_usd: cpu,
            memory_cost_usd: memory,
            storage_cost_usd: storage,
            db_cost_usd: db,
            total_cost_usd: cpu + memory + storage + db,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostBreakdown {
    pub cpu_cost_usd: f64,
    pub memory_cost_usd: f64,
    pub storage_cost_usd: f64,
    pub db_cost_usd: f64,
    pub total_cost_usd: f64,
}

// ── API request / response types ──────────────────────────────────────────────

/// POST /capacity/scenarios
#[derive(Debug, Deserialize)]
pub struct RunScenarioRequest {
    pub name: String,
    pub description: Option<String>,
    /// e.g. 2.0 = double current transaction volume
    pub transaction_volume_multiplier: f64,
    /// Months into the future
    pub timeframe_months: i32,
    /// Number of new merchant chains being onboarded
    pub new_merchant_chains: i32,
    /// Number of new agents being onboarded
    pub new_agent_count: i32,
    /// "aws" | "gcp" | "azure"
    pub cloud_provider: Option<String>,
}

/// POST /capacity/metrics
#[derive(Debug, Deserialize)]
pub struct IngestMetricsRequest {
    pub metric_date: NaiveDate,
    pub active_merchants: i32,
    pub active_agents: i32,
    pub daily_transactions: i64,
    pub peak_tps: f64,
    pub avg_transaction_size_kb: f64,
    pub api_call_volume: i64,
    pub db_connections_peak: i32,
    pub storage_used_gb: f64,
    pub storage_growth_gb: f64,
    pub avg_cpu_pct: f64,
    pub avg_memory_gb: f64,
    pub corridor_breakdown: Option<serde_json::Value>,
}

/// GET /capacity/dashboard — management view (no raw technical metrics)
#[derive(Debug, Serialize)]
pub struct CapacityDashboard {
    pub generated_at: DateTime<Utc>,
    pub peg_status: String,
    /// Plain-language capacity health
    pub capacity_health: String,
    /// 90-day outlook per resource (plain language)
    pub outlook_90d: Vec<ResourceOutlook>,
    /// Monthly burn rate
    pub monthly_burn_rate_usd: f64,
    /// 12-month projected burn rate
    pub projected_annual_cost_usd: f64,
    /// Active alerts count
    pub active_alerts: usize,
    /// Alerts requiring immediate action
    pub critical_alerts: Vec<AlertSummary>,
}

#[derive(Debug, Serialize)]
pub struct ResourceOutlook {
    pub resource: String,
    pub status: String,       // "healthy" | "watch" | "action_required"
    pub plain_language: String,
    pub days_to_threshold: Option<i32>,
}

#[derive(Debug, Serialize)]
pub struct AlertSummary {
    pub resource: String,
    pub message: String,
    pub days_until_breach: i32,
    pub severity: String,
}

/// GET /capacity/forecast
#[derive(Debug, Deserialize)]
pub struct ForecastQuery {
    pub horizon: Option<String>, // "90d" | "12m"
    pub metric: Option<String>,
}

/// GET /capacity/alerts
#[derive(Debug, Deserialize)]
pub struct AlertQuery {
    pub resolved: Option<bool>,
    pub resource: Option<String>,
}

/// POST /capacity/alerts/:id/acknowledge
#[derive(Debug, Deserialize)]
pub struct AcknowledgeAlertRequest {
    pub acknowledged_by: String,
    pub review_task_id: Option<String>,
}
