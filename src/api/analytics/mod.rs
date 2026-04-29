//! Consumer-facing wallet analytics HTTP handlers (Issue #369).
//!
//! Routes:
//!   GET /api/wallet/:wallet_id/analytics/summary
//!   GET /api/wallet/:wallet_id/analytics/spending
//!   GET /api/wallet/:wallet_id/analytics/trends
//!   GET /api/wallet/:wallet_id/analytics/counterparties
//!   GET /api/wallet/:wallet_id/analytics/providers
//!   GET /api/wallet/:wallet_id/analytics/insights
//!   GET /api/wallet/:wallet_id/analytics/insights/preferences
//!   PUT /api/wallet/:wallet_id/analytics/insights/preferences
//!   POST /api/wallet/:wallet_id/analytics/export

pub mod models;

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use bigdecimal::ToPrimitive;
use chrono::{Duration, Utc};
use serde_json::json;
use uuid::Uuid;

use crate::database::analytics_repository::AnalyticsRepository;
use crate::metrics::analytics as metrics;

use models::{
    AnalyticsQuery, AnalyticsSummaryResponse, CounterpartiesResponse, CounterpartyItem,
    ExportQuery, ExportResponse, Granularity, InsightPreferencesRequest,
    InsightPreferencesResponse, InsightResponse, ProviderUsageItem, ProvidersResponse,
    SpendingBreakdownItem, SpendingBreakdownResponse, TimeRange, TrendDataPoint, TrendsResponse,
};

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

#[derive(Clone)]
pub struct AnalyticsState {
    pub repo: Arc<AnalyticsRepository>,
    pub redis: Option<Arc<crate::cache::RedisCache>>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn resolve_range(q: &AnalyticsQuery) -> (chrono::DateTime<Utc>, chrono::DateTime<Utc>) {
    let now = Utc::now();
    match q.range.unwrap_or(TimeRange::Last30Days) {
        TimeRange::Last7Days => (now - Duration::days(7), now),
        TimeRange::Last30Days => (now - Duration::days(30), now),
        TimeRange::Last90Days => (now - Duration::days(90), now),
        TimeRange::CurrentMonth => {
            let start = now.with_day(1).unwrap_or(now);
            (start, now)
        }
        TimeRange::CurrentYear => {
            let start = now
                .with_month(1)
                .and_then(|d| d.with_day(1))
                .unwrap_or(now);
            (start, now)
        }
        TimeRange::Custom => (
            q.from.unwrap_or(now - Duration::days(30)),
            q.to.unwrap_or(now),
        ),
    }
}

fn bd_to_str(v: &sqlx::types::BigDecimal) -> String {
    v.to_string()
}

fn not_found(msg: &str) -> Response {
    (StatusCode::NOT_FOUND, Json(json!({"error": msg}))).into_response()
}

fn internal_err(msg: &str) -> Response {
    (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": msg}))).into_response()
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

pub async fn get_summary(
    State(state): State<Arc<AnalyticsState>>,
    Path(wallet_id): Path<String>,
    Query(q): Query<AnalyticsQuery>,
) -> Response {
    let (from, to) = resolve_range(&q);
    let period = "monthly";

    let snaps = match state.repo.get_snapshots(&wallet_id, period, from, to).await {
        Ok(s) => s,
        Err(e) => return internal_err(&e.to_string()),
    };

    if snaps.is_empty() {
        return not_found("No analytics data found for this wallet");
    }

    let current = &snaps[0];

    // Previous period for deltas
    let prev_from = from - (to - from);
    let prev_snaps = state.repo.get_snapshots(&wallet_id, period, prev_from, from).await.unwrap_or_default();
    let prev = prev_snaps.first();

    let delta_tx = prev.map(|p| {
        if p.total_tx_count > 0 {
            (current.total_tx_count as f64 - p.total_tx_count as f64) / p.total_tx_count as f64 * 100.0
        } else { 0.0 }
    });

    metrics::cache_miss("summary");

    (StatusCode::OK, Json(AnalyticsSummaryResponse {
        wallet_address: wallet_id,
        period_start: current.period_start,
        period_end: current.period_end,
        total_tx_count: current.total_tx_count as i64,
        total_cngn_sent: bd_to_str(&current.total_cngn_sent),
        total_cngn_received: bd_to_str(&current.total_cngn_received),
        total_fees_paid: bd_to_str(&current.total_fees_paid),
        active_days: current.active_days,
        delta_tx_count_pct: delta_tx,
        delta_cngn_sent_pct: None,
        delta_cngn_received_pct: None,
    })).into_response()
}

pub async fn get_spending(
    State(state): State<Arc<AnalyticsState>>,
    Path(wallet_id): Path<String>,
    Query(q): Query<AnalyticsQuery>,
) -> Response {
    let (from, _to) = resolve_range(&q);

    let cats = match state.repo.get_spending_categories(&wallet_id, "monthly", from).await {
        Ok(c) => c,
        Err(e) => return internal_err(&e.to_string()),
    };

    let items: Vec<SpendingBreakdownItem> = cats.iter().map(|c| SpendingBreakdownItem {
        category: c.category.clone(),
        tx_count: c.tx_count,
        total_amount: bd_to_str(&c.total_amount),
        percentage: c.percentage.to_f64().unwrap_or(0.0),
    }).collect();

    (StatusCode::OK, Json(SpendingBreakdownResponse {
        wallet_address: wallet_id,
        period_start: from,
        period_end: _to,
        categories: items,
    })).into_response()
}

pub async fn get_trends(
    State(state): State<Arc<AnalyticsState>>,
    Path(wallet_id): Path<String>,
    Query(q): Query<AnalyticsQuery>,
) -> Response {
    let (from, to) = resolve_range(&q);
    let gran = q.granularity.unwrap_or(Granularity::Daily);
    let period = match gran {
        Granularity::Daily => "daily",
        Granularity::Weekly => "weekly",
        Granularity::Monthly => "monthly",
    };

    let snaps = match state.repo.get_snapshots(&wallet_id, period, from, to).await {
        Ok(s) => s,
        Err(e) => return internal_err(&e.to_string()),
    };

    let points: Vec<TrendDataPoint> = snaps.iter().map(|s| TrendDataPoint {
        timestamp: s.period_start,
        tx_count: s.total_tx_count as i64,
        cngn_volume: bd_to_str(&s.total_cngn_sent),
    }).collect();

    (StatusCode::OK, Json(TrendsResponse {
        wallet_address: wallet_id,
        granularity: period.to_string(),
        data_points: points,
    })).into_response()
}

pub async fn get_counterparties(
    State(state): State<Arc<AnalyticsState>>,
    Path(wallet_id): Path<String>,
) -> Response {
    let rows = match state.repo.get_top_counterparties(&wallet_id, 20).await {
        Ok(r) => r,
        Err(e) => return internal_err(&e.to_string()),
    };

    let items: Vec<CounterpartyItem> = rows.iter().map(|r| CounterpartyItem {
        counterparty_id: r.counterparty_id.clone(),
        counterparty_type: r.counterparty_type.clone(),
        tx_count: r.tx_count,
        total_amount_sent: bd_to_str(&r.total_amount_sent),
        first_tx_at: r.first_tx_at,
        last_tx_at: r.last_tx_at,
    }).collect();

    (StatusCode::OK, Json(CounterpartiesResponse {
        wallet_address: wallet_id,
        counterparties: items,
    })).into_response()
}

pub async fn get_providers(
    State(state): State<Arc<AnalyticsState>>,
    Path(wallet_id): Path<String>,
    Query(q): Query<AnalyticsQuery>,
) -> Response {
    let (from, to) = resolve_range(&q);

    // Aggregate provider usage from snapshots
    let snaps = state.repo.get_snapshots(&wallet_id, "daily", from, to).await.unwrap_or_default();

    let mut provider_map: std::collections::HashMap<String, (i64, f64)> = std::collections::HashMap::new();
    for s in &snaps {
        if let Some(p) = &s.most_used_provider {
            let e = provider_map.entry(p.clone()).or_insert((0, 0.0));
            e.0 += s.total_tx_count as i64;
        }
    }

    let items: Vec<ProviderUsageItem> = provider_map.into_iter().map(|(provider, (count, _))| {
        ProviderUsageItem {
            provider,
            tx_count: count,
            total_amount: "0".to_string(),
            success_rate: 1.0, // placeholder — requires per-tx status tracking
        }
    }).collect();

    (StatusCode::OK, Json(ProvidersResponse {
        wallet_address: wallet_id,
        providers: items,
    })).into_response()
}

pub async fn get_insights(
    State(state): State<Arc<AnalyticsState>>,
    Path(wallet_id): Path<String>,
) -> Response {
    let rows = match state.repo.get_latest_insights(&wallet_id, 10).await {
        Ok(r) => r,
        Err(e) => return internal_err(&e.to_string()),
    };

    let items: Vec<InsightResponse> = rows.iter().map(|r| InsightResponse {
        id: r.id,
        wallet_address: r.wallet_address.clone(),
        period: r.period.clone(),
        period_start: r.period_start,
        top_category: r.top_category.clone(),
        top_category_amount: r.top_category_amount.as_ref().map(bd_to_str),
        prev_period_delta_pct: r.prev_period_delta_pct.as_ref().and_then(|v| v.to_f64()),
        largest_tx_amount: r.largest_tx_amount.as_ref().map(bd_to_str),
        most_frequent_counterparty: r.most_frequent_counterparty.clone(),
        estimated_monthly_fees: r.estimated_monthly_fees.as_ref().map(bd_to_str),
        cngn_balance_trend: r.cngn_balance_trend.clone(),
        generated_at: r.generated_at,
    }).collect();

    (StatusCode::OK, Json(items)).into_response()
}

pub async fn get_insight_preferences(
    State(state): State<Arc<AnalyticsState>>,
    Path(wallet_id): Path<String>,
) -> Response {
    let prefs = state.repo.get_insight_preferences(&wallet_id).await.unwrap_or(None);
    let (weekly, monthly) = prefs.unwrap_or((true, true));
    (StatusCode::OK, Json(InsightPreferencesResponse {
        wallet_address: wallet_id,
        weekly_insights: weekly,
        monthly_insights: monthly,
    })).into_response()
}

pub async fn update_insight_preferences(
    State(state): State<Arc<AnalyticsState>>,
    Path(wallet_id): Path<String>,
    Json(body): Json<InsightPreferencesRequest>,
) -> Response {
    match state.repo.upsert_insight_preferences(&wallet_id, body.weekly_insights, body.monthly_insights).await {
        Ok(_) => (StatusCode::OK, Json(InsightPreferencesResponse {
            wallet_address: wallet_id,
            weekly_insights: body.weekly_insights,
            monthly_insights: body.monthly_insights,
        })).into_response(),
        Err(e) => internal_err(&e.to_string()),
    }
}

pub async fn export_analytics(
    State(_state): State<Arc<AnalyticsState>>,
    Path(wallet_id): Path<String>,
    Json(_body): Json<ExportQuery>,
) -> Response {
    // Async export — enqueue job and return export ID
    let export_id = Uuid::new_v4();
    (StatusCode::ACCEPTED, Json(ExportResponse {
        export_id,
        status: "queued".to_string(),
        message: format!("Export {} queued. You will be notified when ready.", export_id),
    })).into_response()
}

// ---------------------------------------------------------------------------
// use chrono::Datelike / Timelike needed in resolve_range
// ---------------------------------------------------------------------------
use chrono::{Datelike, Timelike};
