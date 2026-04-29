//! Request/response types for wallet analytics endpoints (Issue #369).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Shared enums
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SnapshotPeriod {
    Daily,
    Weekly,
    Monthly,
}

impl SnapshotPeriod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Daily => "daily",
            Self::Weekly => "weekly",
            Self::Monthly => "monthly",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpendingCategory {
    BillPayments,
    Transfers,
    Onramp,
    Offramp,
}

impl SpendingCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::BillPayments => "bill_payments",
            Self::Transfers => "transfers",
            Self::Onramp => "onramp",
            Self::Offramp => "offramp",
        }
    }

    pub fn from_tx_type(tx_type: &str) -> Self {
        match tx_type {
            "onramp" => Self::Onramp,
            "offramp" => Self::Offramp,
            "bill_payment" => Self::BillPayments,
            _ => Self::Transfers,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TimeRange {
    Last7Days,
    Last30Days,
    Last90Days,
    CurrentMonth,
    CurrentYear,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Granularity {
    Daily,
    Weekly,
    Monthly,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnomalyType {
    VolumeSpike,
    SizeShift,
    NewCounterpartyRate,
    TimePatternShift,
}

impl AnomalyType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::VolumeSpike => "volume_spike",
            Self::SizeShift => "size_shift",
            Self::NewCounterpartyRate => "new_counterparty_rate",
            Self::TimePatternShift => "time_pattern_shift",
        }
    }
}

// ---------------------------------------------------------------------------
// Query params
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct AnalyticsQuery {
    pub range: Option<TimeRange>,
    pub granularity: Option<Granularity>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct ExportQuery {
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
    pub metrics: Option<Vec<String>>,
}

// ---------------------------------------------------------------------------
// Consumer-facing responses
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct AnalyticsSummaryResponse {
    pub wallet_address: String,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub total_tx_count: i64,
    pub total_cngn_sent: String,
    pub total_cngn_received: String,
    pub total_fees_paid: String,
    pub active_days: i32,
    pub delta_tx_count_pct: Option<f64>,
    pub delta_cngn_sent_pct: Option<f64>,
    pub delta_cngn_received_pct: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct SpendingBreakdownItem {
    pub category: String,
    pub tx_count: i32,
    pub total_amount: String,
    pub percentage: f64,
}

#[derive(Debug, Serialize)]
pub struct SpendingBreakdownResponse {
    pub wallet_address: String,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub categories: Vec<SpendingBreakdownItem>,
}

#[derive(Debug, Serialize)]
pub struct TrendDataPoint {
    pub timestamp: DateTime<Utc>,
    pub tx_count: i64,
    pub cngn_volume: String,
}

#[derive(Debug, Serialize)]
pub struct TrendsResponse {
    pub wallet_address: String,
    pub granularity: String,
    pub data_points: Vec<TrendDataPoint>,
}

#[derive(Debug, Serialize)]
pub struct CounterpartyItem {
    pub counterparty_id: String,
    pub counterparty_type: String,
    pub tx_count: i32,
    pub total_amount_sent: String,
    pub first_tx_at: DateTime<Utc>,
    pub last_tx_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct CounterpartiesResponse {
    pub wallet_address: String,
    pub counterparties: Vec<CounterpartyItem>,
}

#[derive(Debug, Serialize)]
pub struct ProviderUsageItem {
    pub provider: String,
    pub tx_count: i64,
    pub total_amount: String,
    pub success_rate: f64,
}

#[derive(Debug, Serialize)]
pub struct ProvidersResponse {
    pub wallet_address: String,
    pub providers: Vec<ProviderUsageItem>,
}

#[derive(Debug, Serialize)]
pub struct InsightResponse {
    pub id: Uuid,
    pub wallet_address: String,
    pub period: String,
    pub period_start: DateTime<Utc>,
    pub top_category: Option<String>,
    pub top_category_amount: Option<String>,
    pub prev_period_delta_pct: Option<f64>,
    pub largest_tx_amount: Option<String>,
    pub most_frequent_counterparty: Option<String>,
    pub estimated_monthly_fees: Option<String>,
    pub cngn_balance_trend: Option<String>,
    pub generated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct InsightPreferencesRequest {
    pub weekly_insights: bool,
    pub monthly_insights: bool,
}

#[derive(Debug, Serialize)]
pub struct InsightPreferencesResponse {
    pub wallet_address: String,
    pub weekly_insights: bool,
    pub monthly_insights: bool,
}

// ---------------------------------------------------------------------------
// Admin responses
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct AdminOverviewResponse {
    pub total_wallets: i64,
    pub active_wallets_period: i64,
    pub new_wallets_period: i64,
    pub activation_rate: f64,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct AdminActivityResponse {
    pub total_cngn_transferred: String,
    pub total_fiat_onramped: String,
    pub total_fiat_offramped: String,
    pub avg_tx_size: String,
    pub total_tx_count: i64,
    pub most_used_tx_types: Vec<(String, i64)>,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct AdminRetentionResponse {
    pub retained_wallets: i64,
    pub churned_wallets: i64,
    pub churn_rate: f64,
    pub avg_wallet_lifetime_days: f64,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct CohortDataPoint {
    pub cohort_month: String,
    pub cohort_size: i64,
    pub active_in_period: i64,
    pub retention_rate: f64,
}

#[derive(Debug, Serialize)]
pub struct AdminCohortsResponse {
    pub cohorts: Vec<CohortDataPoint>,
}

#[derive(Debug, Serialize)]
pub struct RiskBand {
    pub band: String,
    pub min_score: f64,
    pub max_score: f64,
    pub wallet_count: i64,
}

#[derive(Debug, Serialize)]
pub struct AdminRiskDistributionResponse {
    pub bands: Vec<RiskBand>,
    pub avg_risk_score: f64,
    pub high_risk_count: i64,
}

#[derive(Debug, Serialize)]
pub struct AnomalyFlagItem {
    pub id: Uuid,
    pub wallet_address: String,
    pub anomaly_type: String,
    pub deviation_magnitude: f64,
    pub flagged_at: DateTime<Utc>,
    pub routed_to_compliance: bool,
}

#[derive(Debug, Serialize)]
pub struct AdminAnomaliesResponse {
    pub anomalies: Vec<AnomalyFlagItem>,
    pub total: i64,
}

#[derive(Debug, Serialize)]
pub struct BehaviourProfileResponse {
    pub wallet_address: String,
    pub avg_tx_size: String,
    pub tx_frequency_per_week: f64,
    pub preferred_hour_utc: Option<i16>,
    pub preferred_provider: Option<String>,
    pub preferred_currency_pair: Option<String>,
    pub risk_score: f64,
    pub profile_updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ExportResponse {
    pub export_id: Uuid,
    pub status: String,
    pub message: String,
}
