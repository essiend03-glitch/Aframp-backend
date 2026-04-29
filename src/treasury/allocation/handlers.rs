/// HTTP Handlers — Smart Treasury Allocation Engine
///
/// Route map:
///
///   Internal (treasury-operator RBAC required):
///     POST   /treasury/allocation/record                  — record balance snapshot + run pipeline
///     GET    /treasury/allocation/monitor                 — internal allocation dashboard
///     GET    /treasury/allocation/alerts                  — list unresolved concentration alerts
///     GET    /treasury/allocation/rwa/latest              — latest RWA snapshot
///     POST   /treasury/allocation/rwa/calculate           — trigger daily RWA calculation
///     GET    /treasury/allocation/orders                  — list transfer orders
///     GET    /treasury/allocation/orders/:id              — get single transfer order
///     POST   /treasury/allocation/orders/:id/decision     — approve / reject transfer order
///     POST   /treasury/allocation/orders/:id/complete     — mark transfer order completed
///     POST   /treasury/allocation/custodians/:id/rating   — update custodian risk rating
///
///   Public (no auth):
///     GET    /treasury/allocation/public                  — sanitised holdings dashboard
use super::engine::AllocationEngine;
use super::repository::AllocationRepository;
use super::types::*;
use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::Utc;
use std::sync::Arc;
use uuid::Uuid;

/// Shared state injected via `axum::extract::State`.
pub type AllocationState = Arc<AllocationEngine>;
/// Read-only repo state for the public endpoint (no engine overhead).
pub type AllocationRepoState = Arc<AllocationRepository>;

// ── Internal handlers ─────────────────────────────────────────────────────────

/// POST /treasury/allocation/record
///
/// Records a new balance snapshot for a custodian and runs the full
/// concentration-check → alert → rebalance pipeline.
pub async fn record_allocation(
    State(engine): State<AllocationState>,
    Extension(operator_id): Extension<String>,
    Json(req): Json<RecordAllocationRequest>,
) -> impl IntoResponse {
    match engine.record_and_evaluate(req, &operator_id).await {
        Ok(snapshot) => (StatusCode::CREATED, Json(snapshot)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

/// GET /treasury/allocation/monitor
///
/// Returns the real-time internal allocation dashboard:
/// per-custodian balance, concentration %, breach flags, peg coverage.
pub async fn get_allocation_monitor(
    State(engine): State<AllocationState>,
) -> impl IntoResponse {
    match engine.allocation_monitor().await {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

/// GET /treasury/allocation/alerts
///
/// Lists all unresolved concentration alerts.
pub async fn list_alerts(
    State(engine): State<AllocationState>,
) -> impl IntoResponse {
    // Reach into the repo via the engine's public accessor.
    match engine.unresolved_alerts().await {
        Ok(alerts) => (StatusCode::OK, Json(alerts)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

/// GET /treasury/allocation/rwa/latest
///
/// Returns the most recent daily RWA snapshot.
pub async fn get_latest_rwa(
    State(engine): State<AllocationState>,
) -> impl IntoResponse {
    match engine.latest_rwa().await {
        Ok(Some(snap)) => (StatusCode::OK, Json(snap)).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "No RWA snapshot found" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

/// POST /treasury/allocation/rwa/calculate
///
/// Triggers the daily RWA calculation for today (or a supplied date).
/// Body: `{ "onchain_supply_kobo": 1234567890, "date": "2026-04-23" }`
pub async fn calculate_rwa(
    State(engine): State<AllocationState>,
    Extension(operator_id): Extension<String>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let onchain_supply_kobo = match body
        .get("onchain_supply_kobo")
        .and_then(|v| v.as_i64())
    {
        Some(v) => v,
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "onchain_supply_kobo is required (i64)" })),
            )
                .into_response()
        }
    };

    let date = body
        .get("date")
        .and_then(|v| v.as_str())
        .and_then(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").ok())
        .unwrap_or_else(|| Utc::now().date_naive());

    let _ = operator_id; // available for audit if needed

    match engine.calculate_daily_rwa(onchain_supply_kobo, date).await {
        Ok(snap) => (StatusCode::OK, Json(snap)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

/// GET /treasury/allocation/orders?status=pending_approval&page=1&page_size=20
pub async fn list_transfer_orders(
    State(engine): State<AllocationState>,
    Query(q): Query<ListTransferOrdersQuery>,
) -> impl IntoResponse {
    match engine
        .list_orders(q.status, q.page_size(), q.offset())
        .await
    {
        Ok(orders) => (StatusCode::OK, Json(orders)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

/// GET /treasury/allocation/orders/:id
pub async fn get_transfer_order(
    State(engine): State<AllocationState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match engine.get_order(id).await {
        Ok(order) => (StatusCode::OK, Json(order)).into_response(),
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

/// POST /treasury/allocation/orders/:id/decision
///
/// Approve or reject a pending transfer order.
/// Body: `{ "action": "approve" | "reject", "rejection_reason": "..." }`
pub async fn decide_transfer_order(
    State(engine): State<AllocationState>,
    Extension(operator_id): Extension<String>,
    Path(id): Path<Uuid>,
    Json(req): Json<TransferOrderDecisionRequest>,
) -> impl IntoResponse {
    match engine.decide_order(id, req, &operator_id).await {
        Ok(order) => (StatusCode::OK, Json(order)).into_response(),
        Err(e) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

/// POST /treasury/allocation/orders/:id/complete
///
/// Mark an executing transfer order as completed.
/// Body: `{ "bank_reference": "NXP20260423001" }`
pub async fn complete_transfer_order(
    State(engine): State<AllocationState>,
    Extension(operator_id): Extension<String>,
    Path(id): Path<Uuid>,
    Json(req): Json<CompleteTransferRequest>,
) -> impl IntoResponse {
    match engine.complete_order(id, req, &operator_id).await {
        Ok(order) => (StatusCode::OK, Json(order)).into_response(),
        Err(e) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

/// POST /treasury/allocation/custodians/:id/rating
///
/// Update a custodian's risk rating. If the new rating requires rebalancing,
/// transfer orders are auto-generated and returned.
pub async fn update_custodian_rating(
    State(engine): State<AllocationState>,
    Extension(operator_id): Extension<String>,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateRiskRatingRequest>,
) -> impl IntoResponse {
    match engine
        .handle_rating_downgrade(id, req.risk_rating, &operator_id)
        .await
    {
        Ok(orders) => {
            let rebalance_triggered = !orders.is_empty();
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "rebalance_triggered": rebalance_triggered,
                    "transfer_orders_created": orders.len(),
                    "orders": orders,
                })),
            )
                .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}

// ── Public handler ────────────────────────────────────────────────────────────

/// GET /treasury/allocation/public
///
/// Sanitised public transparency dashboard.
/// Returns diversified holdings data without sensitive account details.
pub async fn public_reserve_dashboard(
    State(engine): State<AllocationState>,
) -> impl IntoResponse {
    match engine.public_dashboard().await {
        Ok(resp) => (StatusCode::OK, Json(resp)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
}
