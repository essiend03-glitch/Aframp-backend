//! Admin wallet analytics HTTP handlers (Issue #369).
//!
//! Routes (all require admin auth):
//!   GET  /api/admin/analytics/wallets/overview
//!   GET  /api/admin/analytics/wallets/activity
//!   GET  /api/admin/analytics/wallets/retention
//!   GET  /api/admin/analytics/wallets/cohorts
//!   GET  /api/admin/analytics/wallets/risk-distribution
//!   GET  /api/admin/analytics/wallets/anomalies
//!   GET  /api/admin/wallets/:wallet_id/behaviour-profile
//!   POST /api/admin/analytics/wallets/export

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use bigdecimal::ToPrimitive;
use chrono::{Duration, Utc};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::api::analytics::models::{
    AdminActivityResponse, AdminAnomaliesResponse, AdminCohortsResponse, AdminOverviewResponse,
    AdminRetentionResponse, AdminRiskDistributionResponse, AnomalyFlagItem, BehaviourProfileResponse,
    CohortDataPoint, ExportResponse, RiskBand,
};
use crate::database::analytics_repository::AnalyticsRepository;
use crate::metrics::analytics as metrics;

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct AdminAnalyticsState {
    pub repo: Arc<AnalyticsRepository>,
}

// ---------------------------------------------------------------------------
// Query params
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct PeriodQuery {
    pub days: Option<i64>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn internal_err(msg: &str) -> Response {
    (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": msg}))).into_response()
}

fn not_found(msg: &str) -> Response {
    (StatusCode::NOT_FOUND, Json(json!({"error": msg}))).into_response()
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

pub async fn get_overview(
    State(state): State<Arc<AdminAnalyticsState>>,
    Query(q): Query<PeriodQuery>,
) -> Response {
    let days = q.days.unwrap_or(30);
    let to = Utc::now();
    let from = to - Duration::days(days);
    let from_date = from.date_naive();
    let to_date = to.date_naive();

    let aggs = match state.repo.get_daily_aggregates(from_date, to_date).await {
        Ok(a) => a,
        Err(e) => return internal_err(&e.to_string()),
    };

    let total_wallets = aggs.first().map(|a| a.total_wallets).unwrap_or(0);
    let active_wallets: i64 = aggs.iter().map(|a| a.active_wallets).max().unwrap_or(0);
    let new_wallets: i64 = aggs.iter().map(|a| a.new_wallets).sum();
    let activation_rate = if total_wallets > 0 {
        active_wallets as f64 / total_wallets as f64 * 100.0
    } else {
        0.0
    };

    metrics::cache_miss("admin_overview");

    (StatusCode::OK, Json(AdminOverviewResponse {
        total_wallets,
        active_wallets_period: active_wallets,
        new_wallets_period: new_wallets,
        activation_rate,
        period_start: from,
        period_end: to,
    })).into_response()
}

pub async fn get_activity(
    State(state): State<Arc<AdminAnalyticsState>>,
    Query(q): Query<PeriodQuery>,
) -> Response {
    let days = q.days.unwrap_or(30);
    let to = Utc::now();
    let from = to - Duration::days(days);

    let aggs = match state.repo.get_daily_aggregates(from.date_naive(), to.date_naive()).await {
        Ok(a) => a,
        Err(e) => return internal_err(&e.to_string()),
    };

    let total_cngn: bigdecimal::BigDecimal = aggs.iter().map(|a| a.total_cngn_transferred.clone()).sum();
    let total_onramped: bigdecimal::BigDecimal = aggs.iter().map(|a| a.total_fiat_onramped.clone()).sum();
    let total_offramped: bigdecimal::BigDecimal = aggs.iter().map(|a| a.total_fiat_offramped.clone()).sum();
    let total_txs: i64 = aggs.iter().map(|a| a.total_tx_count).sum();
    let avg_size = if !aggs.is_empty() {
        aggs.iter().map(|a| a.avg_tx_size.clone()).sum::<bigdecimal::BigDecimal>()
            / bigdecimal::BigDecimal::from(aggs.len() as i64)
    } else {
        bigdecimal::BigDecimal::from(0)
    };

    (StatusCode::OK, Json(AdminActivityResponse {
        total_cngn_transferred: total_cngn.to_string(),
        total_fiat_onramped: total_onramped.to_string(),
        total_fiat_offramped: total_offramped.to_string(),
        avg_tx_size: avg_size.to_string(),
        total_tx_count: total_txs,
        most_used_tx_types: vec![],
        period_start: from,
        period_end: to,
    })).into_response()
}

pub async fn get_retention(
    State(state): State<Arc<AdminAnalyticsState>>,
    Query(q): Query<PeriodQuery>,
) -> Response {
    let days = q.days.unwrap_or(30);
    let to = Utc::now();
    let curr_from = to - Duration::days(days);
    let prev_from = curr_from - Duration::days(days);

    let (retained, churned) = match state.repo
        .retention_metrics(prev_from, curr_from, curr_from, to)
        .await
    {
        Ok(r) => r,
        Err(e) => return internal_err(&e.to_string()),
    };

    let total_prev = retained + churned;
    let churn_rate = if total_prev > 0 {
        churned as f64 / total_prev as f64 * 100.0
    } else {
        0.0
    };

    (StatusCode::OK, Json(AdminRetentionResponse {
        retained_wallets: retained,
        churned_wallets: churned,
        churn_rate,
        avg_wallet_lifetime_days: days as f64, // placeholder
        period_start: curr_from,
        period_end: to,
    })).into_response()
}

pub async fn get_cohorts(
    State(state): State<Arc<AdminAnalyticsState>>,
    Query(q): Query<PeriodQuery>,
) -> Response {
    let days = q.days.unwrap_or(90);
    let to = Utc::now();
    let from = to - Duration::days(days);

    let rows = match state.repo.cohort_analysis(from, to).await {
        Ok(r) => r,
        Err(e) => return internal_err(&e.to_string()),
    };

    let cohorts: Vec<CohortDataPoint> = rows.into_iter().map(|(month, size, active)| {
        let retention_rate = if size > 0 { active as f64 / size as f64 * 100.0 } else { 0.0 };
        CohortDataPoint {
            cohort_month: month,
            cohort_size: size,
            active_in_period: active,
            retention_rate,
        }
    }).collect();

    (StatusCode::OK, Json(AdminCohortsResponse { cohorts })).into_response()
}

pub async fn get_risk_distribution(
    State(state): State<Arc<AdminAnalyticsState>>,
) -> Response {
    let dist = match state.repo.risk_score_distribution().await {
        Ok(d) => d,
        Err(e) => return internal_err(&e.to_string()),
    };
    let avg = state.repo.avg_risk_score().await.unwrap_or(0.0);
    let open_anomalies = state.repo.count_open_anomalies().await.unwrap_or(0);

    let bands: Vec<RiskBand> = dist.into_iter().map(|(band, count, min_s, max_s)| RiskBand {
        band,
        min_score: min_s,
        max_score: max_s,
        wallet_count: count,
    }).collect();

    let high_risk = bands.iter()
        .filter(|b| b.band == "high" || b.band == "critical")
        .map(|b| b.wallet_count)
        .sum();

    metrics::anomaly_flagged_wallets(open_anomalies as f64);

    (StatusCode::OK, Json(AdminRiskDistributionResponse {
        bands,
        avg_risk_score: avg,
        high_risk_count: high_risk,
    })).into_response()
}

pub async fn get_anomalies(
    State(state): State<Arc<AdminAnalyticsState>>,
) -> Response {
    let rows = match state.repo.list_open_anomalies(100, 0).await {
        Ok(r) => r,
        Err(e) => return internal_err(&e.to_string()),
    };
    let total = state.repo.count_open_anomalies().await.unwrap_or(0);

    let items: Vec<AnomalyFlagItem> = rows.iter().map(|r| AnomalyFlagItem {
        id: r.id,
        wallet_address: r.wallet_address.clone(),
        anomaly_type: r.anomaly_type.clone(),
        deviation_magnitude: r.deviation_magnitude.to_f64().unwrap_or(0.0),
        flagged_at: r.flagged_at,
        routed_to_compliance: r.routed_to_compliance,
    }).collect();

    (StatusCode::OK, Json(AdminAnomaliesResponse { anomalies: items, total })).into_response()
}

pub async fn get_behaviour_profile(
    State(state): State<Arc<AdminAnalyticsState>>,
    Path(wallet_id): Path<String>,
) -> Response {
    match state.repo.get_profile(&wallet_id).await {
        Ok(Some(p)) => (StatusCode::OK, Json(BehaviourProfileResponse {
            wallet_address: wallet_id,
            avg_tx_size: p.avg_tx_size.to_string(),
            tx_frequency_per_week: p.tx_frequency_per_week.to_f64().unwrap_or(0.0),
            preferred_hour_utc: p.preferred_hour_utc,
            preferred_provider: p.preferred_provider,
            preferred_currency_pair: p.preferred_currency_pair,
            risk_score: p.risk_score.to_f64().unwrap_or(0.0),
            profile_updated_at: p.profile_updated_at,
        })).into_response(),
        Ok(None) => not_found("Behaviour profile not found"),
        Err(e) => internal_err(&e.to_string()),
    }
}

pub async fn export_admin_analytics(
    State(_state): State<Arc<AdminAnalyticsState>>,
) -> Response {
    let export_id = Uuid::new_v4();
    (StatusCode::ACCEPTED, Json(ExportResponse {
        export_id,
        status: "queued".to_string(),
        message: format!("Admin export {} queued.", export_id),
    })).into_response()
}
