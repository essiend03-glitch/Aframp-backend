use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use crate::auth::middleware::AuthMiddleware;
use crate::error::AppError;
use super::{
    DeFiService, CreateSavingsAccountRequest, DepositRequest, WithdrawalRequest as SavingsWithdrawalRequest,
    SavingsAccountResponse, CngnSavingsProduct, YieldRateHistory,
    CreateAmmPositionRequest, AmmSwapRequest, AmmPoolResponse,
};

/// DeFi handlers for savings products and AMM operations
pub struct DeFiHandlers;

impl DeFiHandlers {
    // ── Savings Product Endpoints ─────────────────────────────────────────────

    /// List available savings products
    pub async fn list_savings_products(
        State(defi_service): State<Arc<DeFiService>>,
        Query(params): Query<ListSavingsProductsParams>,
    ) -> Result<Json<Vec<CngnSavingsProduct>>, AppError> {
        let products = defi_service.list_savings_products(params.active_only).await?;
        Ok(Json(products))
    }

    /// Get savings product details
    pub async fn get_savings_product(
        State(defi_service): State<Arc<DeFiService>>,
        Path(product_id): Path<Uuid>,
    ) -> Result<Json<CngnSavingsProduct>, AppError> {
        let product = defi_service.get_savings_product(product_id).await?;
        Ok(Json(product))
    }

    /// Create a new savings account
    pub async fn create_savings_account(
        State(defi_service): State<Arc<DeFiService>>,
        Json(request): Json<CreateSavingsAccountRequest>,
        auth_user: AuthMiddleware,
    ) -> Result<Json<SavingsAccountResponse>, AppError> {
        let account = defi_service.create_savings_account(
            request,
            &auth_user.user_id,
            auth_user.ip_address.as_deref(),
        ).await?;
        
        let response = defi_service.build_savings_account_response(account).await?;
        Ok(Json(response))
    }

    /// Get user's savings accounts
    pub async fn get_user_savings_accounts(
        State(defi_service): State<Arc<DeFiService>>,
        auth_user: AuthMiddleware,
    ) -> Result<Json<Vec<SavingsAccountResponse>>, AppError> {
        let accounts = defi_service.get_user_savings_accounts(&auth_user.user_id).await?;
        Ok(Json(accounts))
    }

    /// Get savings account details
    pub async fn get_savings_account(
        State(defi_service): State<Arc<DeFiService>>,
        Path(account_id): Path<Uuid>,
        auth_user: AuthMiddleware,
    ) -> Result<Json<SavingsAccountResponse>, AppError> {
        let account = defi_service.get_savings_account(account_id, &auth_user.user_id).await?;
        let response = defi_service.build_savings_account_response(account).await?;
        Ok(Json(response))
    }

    /// Deposit to savings account
    pub async fn deposit_to_savings_account(
        State(defi_service): State<Arc<DeFiService>>,
        Json(request): Json<DepositRequest>,
        auth_user: AuthMiddleware,
    ) -> Result<Json<SavingsAccountResponse>, AppError> {
        let account = defi_service.deposit_to_savings_account(request, &auth_user.user_id).await?;
        let response = defi_service.build_savings_account_response(account).await?;
        Ok(Json(response))
    }

    /// Withdraw from savings account
    pub async fn withdraw_from_savings_account(
        State(defi_service): State<Arc<DeFiService>>,
        Json(request): Json<SavingsWithdrawalRequest>,
        auth_user: AuthMiddleware,
    ) -> Result<Json<super::WithdrawalRequest>, AppError> {
        let withdrawal = defi_service.withdraw_from_savings_account(request, &auth_user.user_id).await?;
        Ok(Json(withdrawal))
    }

    /// Get projected yield for savings account
    pub async fn get_projected_yield(
        State(defi_service): State<Arc<DeFiService>>,
        Path(account_id): Path<Uuid>,
        Query(params): Query<ProjectedYieldParams>,
        auth_user: AuthMiddleware,
    ) -> Result<Json<super::ProjectedYield>, AppError> {
        let projected = defi_service.calculate_projected_yield(
            account_id,
            params.period_days.unwrap_or(30),
            &auth_user.user_id,
        ).await?;
        
        Ok(Json(projected))
    }

    /// Reinvest accrued yield
    pub async fn reinvest_yield(
        State(defi_service): State<Arc<DeFiService>>,
        Path(account_id): Path<Uuid>,
        auth_user: AuthMiddleware,
    ) -> Result<Json<SavingsAccountResponse>, AppError> {
        let account = defi_service.reinvest_yield(account_id, &auth_user.user_id).await?;
        let response = defi_service.build_savings_account_response(account).await?;
        Ok(Json(response))
    }

    /// Get yield rate history for a product
    pub async fn get_yield_rate_history(
        State(defi_service): State<Arc<DeFiService>>,
        Path(product_id): Path<Uuid>,
        Query(params): Query<YieldHistoryParams>,
    ) -> Result<Json<Vec<YieldRateHistory>>, AppError> {
        let history = defi_service.get_yield_rate_history(
            product_id,
            params.days,
        ).await?;
        
        Ok(Json(history))
    }

    /// Accept risk disclosure
    pub async fn accept_risk_disclosure(
        State(defi_service): State<Arc<DeFiService>>,
        Json(request): Json<RiskDisclosureAcceptanceRequest>,
        auth_user: AuthMiddleware,
    ) -> Result<StatusCode, AppError> {
        defi_service.accept_risk_disclosure(
            &auth_user.user_id,
            request.product_id,
            auth_user.ip_address.as_deref(),
        ).await?;
        
        Ok(StatusCode::OK)
    }

    // ── AMM Integration Endpoints ─────────────────────────────────────────────

    /// Get AMM pools
    pub async fn get_amm_pools(
        State(defi_service): State<Arc<DeFiService>>,
        Query(params): Query<AmmPoolsParams>,
    ) -> Result<Json<Vec<AmmPoolResponse>>, AppError> {
        let pools = defi_service.get_amm_pools(params.include_positions).await?;
        Ok(Json(pools))
    }

    /// Get AMM pool details
    pub async fn get_amm_pool(
        State(defi_service): State<Arc<DeFiService>>,
        Path(pool_id): Path<String>,
    ) -> Result<Json<AmmPoolResponse>, AppError> {
        let pool = defi_service.get_amm_pool(&pool_id).await?;
        Ok(Json(pool))
    }

    /// Create AMM liquidity position
    pub async fn create_amm_position(
        State(defi_service): State<Arc<DeFiService>>,
        Json(request): Json<CreateAmmPositionRequest>,
        auth_user: AuthMiddleware,
    ) -> Result<Json<super::AmmLiquidityPosition>, AppError> {
        let position = defi_service.create_amm_position(request, &auth_user.user_id).await?;
        Ok(Json(position))
    }

    /// Execute AMM swap
    pub async fn execute_amm_swap(
        State(defi_service): State<Arc<DeFiService>>,
        Json(request): Json<AmmSwapRequest>,
        auth_user: AuthMiddleware,
    ) -> Result<Json<super::DeFiSwapResult>, AppError> {
        let swap_result = defi_service.execute_amm_swap(request, &auth_user.user_id).await?;
        Ok(Json(swap_result))
    }

    /// Get AMM position
    pub async fn get_amm_position(
        State(defi_service): State<Arc<DeFiService>>,
        Path(position_id): Path<Uuid>,
        auth_user: AuthMiddleware,
    ) -> Result<Json<super::AmmLiquidityPosition>, AppError> {
        let position = defi_service.get_amm_position(position_id, &auth_user.user_id).await?;
        Ok(Json(position))
    }

    /// Close AMM position
    pub async fn close_amm_position(
        State(defi_service): State<Arc<DeFiService>>,
        Path(position_id): Path<Uuid>,
        Json(request): Json<CloseAmmPositionRequest>,
        auth_user: AuthMiddleware,
    ) -> Result<Json<super::DeFiWithdrawalResult>, AppError> {
        let result = defi_service.close_amm_position(
            position_id,
            request.percentage,
            &auth_user.user_id,
        ).await?;
        
        Ok(Json(result))
    }

    /// Get AMM position history
    pub async fn get_amm_position_history(
        State(defi_service): State<Arc<DeFiService>>,
        Path(position_id): Path<Uuid>,
        Query(params): Query<PositionHistoryParams>,
    ) -> Result<Json<Vec<super::AmmPositionSnapshot>>, AppError> {
        let history = defi_service.get_amm_position_history(
            position_id,
            params.limit,
        ).await?;
        
        Ok(Json(history))
    }

    // ── Admin Endpoints ─────────────────────────────────────────────────────

    /// Get savings product overview (admin)
    pub async fn get_savings_overview(
        State(defi_service): State<Arc<DeFiService>>,
    ) -> Result<Json<super::SavingsOverviewMetrics>, AppError> {
        let overview = defi_service.get_savings_overview().await?;
        Ok(Json(overview))
    }

    /// Get all savings accounts (admin)
    pub async fn get_all_savings_accounts(
        State(defi_service): State<Arc<DeFiService>>,
        Query(params): Query<AdminAccountsParams>,
    ) -> Result<Json<Vec<SavingsAccountResponse>>, AppError> {
        let accounts = defi_service.get_all_savings_accounts(
            params.product_id,
            params.status,
            params.date_from,
            params.date_to,
            params.limit,
            params.offset,
        ).await?;
        
        Ok(Json(accounts))
    }

    /// Suspend savings product (admin)
    pub async fn suspend_savings_product(
        State(defi_service): State<Arc<DeFiService>>,
        Path(product_id): Path<Uuid>,
        Json(request): Json<SuspendProductRequest>,
        auth_user: AuthMiddleware,
    ) -> Result<StatusCode, AppError> {
        defi_service.suspend_savings_product(
            product_id,
            &request.reason,
            &auth_user.user_id,
        ).await?;
        
        Ok(StatusCode::OK)
    }

    /// Close savings product (admin)
    pub async fn close_savings_product(
        State(defi_service): State<Arc<DeFiService>>,
        Path(product_id): Path<Uuid>,
        Json(request): Json<CloseProductRequest>,
        auth_user: AuthMiddleware,
    ) -> Result<StatusCode, AppError> {
        defi_service.close_savings_product(
            product_id,
            &request.reason,
            request.transition_period_days,
            &auth_user.user_id,
        ).await?;
        
        Ok(StatusCode::OK)
    }

    /// Get AMM income metrics (admin)
    pub async fn get_amm_income_metrics(
        State(defi_service): State<Arc<DeFiService>>,
        Query(params): Query<IncomeMetricsParams>,
    ) -> Result<Json<super::AmmIncomeMetrics>, AppError> {
        let metrics = defi_service.get_amm_income_metrics(
            params.period_start,
            params.period_end,
        ).await?;
        
        Ok(Json(metrics))
    }

    /// Get AMM positions (admin)
    pub async fn get_amm_positions(
        State(defi_service): State<Arc<DeFiService>>,
        Query(params): Query<AdminPositionsParams>,
    ) -> Result<Json<Vec<super::AmmLiquidityPosition>>, AppError> {
        let positions = defi_service.get_amm_positions(
            params.pool_id,
            params.status,
            params.limit,
            params.offset,
        ).await?;
        
        Ok(Json(positions))
    }
}

// ── Request/Response DTOs ─────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ListSavingsProductsParams {
    pub active_only: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct ProjectedYieldParams {
    pub period_days: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct YieldHistoryParams {
    pub days: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct RiskDisclosureAcceptanceRequest {
    pub product_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct AmmPoolsParams {
    pub include_positions: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct CloseAmmPositionRequest {
    pub percentage: f64, // 0.0 to 1.0
}

#[derive(Debug, Deserialize)]
pub struct PositionHistoryParams {
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct AdminAccountsParams {
    pub product_id: Option<Uuid>,
    pub status: Option<String>,
    pub date_from: Option<chrono::DateTime<chrono::Utc>>,
    pub date_to: Option<chrono::DateTime<chrono::Utc>>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct SuspendProductRequest {
    pub reason: String,
}

#[derive(Debug, Deserialize)]
pub struct CloseProductRequest {
    pub reason: String,
    pub transition_period_days: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct IncomeMetricsParams {
    pub period_start: Option<chrono::DateTime<chrono::Utc>>,
    pub period_end: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct AdminPositionsParams {
    pub pool_id: Option<String>,
    pub status: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct SavingsOverviewMetrics {
    pub total_cngn_deposited: sqlx::types::BigDecimal,
    pub total_yield_accrued: sqlx::types::BigDecimal,
    pub average_yield_rate: f64,
    pub active_account_count: u64,
    pub total_withdrawal_volume: sqlx::types::BigDecimal,
    pub product_breakdown: Vec<ProductMetrics>,
}

#[derive(Debug, Serialize)]
pub struct ProductMetrics {
    pub product_id: Uuid,
    pub product_name: String,
    pub total_deposits: sqlx::types::BigDecimal,
    pub total_yield: sqlx::types::BigDecimal,
    pub active_accounts: u64,
    pub average_yield_rate: f64,
}

#[derive(Debug, Serialize)]
pub struct AmmIncomeMetrics {
    pub period_start: chrono::DateTime<chrono::Utc>,
    pub period_end: chrono::DateTime<chrono::Utc>,
    pub total_fee_income: sqlx::types::BigDecimal,
    pub pool_breakdown: Vec<PoolIncomeMetrics>,
}

#[derive(Debug, Serialize)]
pub struct PoolIncomeMetrics {
    pub pool_id: String,
    pub asset_pair: String,
    pub fee_income: sqlx::types::BigDecimal,
    pub volume: sqlx::types::BigDecimal,
    pub effective_apr: f64,
}
