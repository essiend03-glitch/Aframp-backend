/// Task 3 — Yield Distribution & Interest Calculation Engine
///
/// Computes, attributes, and distributes yield earned from DeFi protocol positions
/// to savings account holders, liquidity providers, and the platform treasury.
/// Handles pro-rata distribution, tiered rates, compound reinvestment, and full
/// reconciliation with a tamper-evident audit trail.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::BigDecimal;
use std::collections::HashMap;
use std::sync::Arc;
use uuid::Uuid;

use crate::database::DbPool;
use crate::error::AppError;

// ── Yield Source ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "yield_source_type", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum YieldSourceType {
    AmmTradingFees,
    LendingInterest,
    LiquidityMiningIncentives,
    PlatformTreasuryContribution,
}

/// Yield source record for a single calculation cycle
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct YieldSourceRecord {
    pub source_id: Uuid,
    pub protocol_id: String,
    pub source_type: YieldSourceType,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub gross_yield: BigDecimal,
    pub gas_fees_deducted: BigDecimal,
    pub platform_management_fee: BigDecimal,
    pub protocol_fees_deducted: BigDecimal,
    pub net_distributable_yield: BigDecimal,
    pub is_realized: bool,
    pub recorded_at: DateTime<Utc>,
}

// ── Distribution Model ────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DistributionModel {
    ProRata,
    Tiered,
    Waterfall,
}

// ── Yield Tier Configuration ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct YieldTierConfig {
    pub tier_id: Uuid,
    pub product_id: Uuid,
    pub tier_name: String,
    /// Minimum balance to qualify for this tier
    pub min_balance: BigDecimal,
    /// Maximum balance for this tier (None = unlimited)
    pub max_balance: Option<BigDecimal>,
    /// Annual yield rate for this tier (e.g. 0.05 = 5%)
    pub annual_rate: f64,
    pub is_active: bool,
    pub updated_at: DateTime<Utc>,
}

// ── Yield Accrual ─────────────────────────────────────────────────────────────

/// Per-account yield accrual record for a single distribution cycle
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct YieldAccrualEntry {
    pub accrual_id: Uuid,
    pub account_id: Uuid,
    pub product_id: Uuid,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub opening_balance: BigDecimal,
    /// Pro-rata share of the pool (0.0–1.0)
    pub pro_rata_share: f64,
    pub yield_source_type: YieldSourceType,
    pub rate_applied: f64,
    pub yield_amount: BigDecimal,
    /// Fiat equivalent at time of crediting (for tax reporting)
    pub fiat_equivalent: BigDecimal,
    pub is_compound_reinvested: bool,
    pub credited_at: DateTime<Utc>,
}

// ── Treasury Yield ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TreasuryYieldRecord {
    pub record_id: Uuid,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub gross_yield: BigDecimal,
    pub management_fee_obligation: BigDecimal,
    pub net_treasury_yield: BigDecimal,
    pub source_breakdown: serde_json::Value,
    pub credited_at: DateTime<Utc>,
}

// ── Reconciliation ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct YieldReconciliationRecord {
    pub reconciliation_id: Uuid,
    pub cycle_id: Uuid,
    pub period_start: DateTime<Utc>,
    pub period_end: DateTime<Utc>,
    pub total_net_distributable: BigDecimal,
    pub total_distributed_to_accounts: BigDecimal,
    pub total_distributed_to_treasury: BigDecimal,
    pub rounding_discrepancy: BigDecimal,
    pub is_balanced: bool,
    pub discrepancy_exceeds_tolerance: bool,
    pub reconciled_at: DateTime<Utc>,
}

// ── Yield Rate Publication ────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct EffectiveYieldRate {
    pub rate_id: Uuid,
    pub product_id: Uuid,
    /// Raw effective annualised rate from this cycle
    pub raw_rate: f64,
    /// Smoothed rate (moving average)
    pub smoothed_rate: f64,
    pub computed_at: DateTime<Utc>,
}

// ── Tax Summary ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YieldTaxSummary {
    pub account_id: Uuid,
    pub tax_year: i32,
    pub total_yield_earned: BigDecimal,
    pub total_fiat_equivalent: BigDecimal,
    pub monthly_breakdown: Vec<MonthlyYieldSummary>,
    pub source_breakdown: Vec<SourceYieldSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MonthlyYieldSummary {
    pub year: i32,
    pub month: u32,
    pub yield_amount: BigDecimal,
    pub fiat_equivalent: BigDecimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceYieldSummary {
    pub source_type: YieldSourceType,
    pub yield_amount: BigDecimal,
    pub fiat_equivalent: BigDecimal,
}

// ── Yield Distribution Engine ─────────────────────────────────────────────────

pub struct YieldDistributionEngine {
    db: Arc<DbPool>,
    /// Reconciliation tolerance — discrepancy above this is a critical error
    reconciliation_tolerance: BigDecimal,
    /// Number of cycles for smoothed rate moving average
    smoothing_window: usize,
    /// Minimum acceptable yield rate — alert if below for consecutive_cycles_threshold cycles
    min_acceptable_rate: f64,
    consecutive_cycles_threshold: u32,
}

impl YieldDistributionEngine {
    pub fn new(db: Arc<DbPool>) -> Self {
        Self {
            db,
            reconciliation_tolerance: BigDecimal::from(1), // 1 unit tolerance
            smoothing_window: 7,
            min_acceptable_rate: 0.01, // 1% annualised
            consecutive_cycles_threshold: 3,
        }
    }

    // ── Pro-Rata Share Calculation ────────────────────────────────────────────

    /// Compute each account's pro-rata share of the pool at period start.
    /// Handles mid-period deposits and withdrawals by time-weighting.
    ///
    /// `account_balances`: (account_id, balance, deposit_timestamp, withdrawal_timestamp)
    /// Returns a map of account_id → pro-rata share (0.0–1.0)
    pub fn compute_pro_rata_shares(
        &self,
        period_start: DateTime<Utc>,
        period_end: DateTime<Utc>,
        account_balances: &[(Uuid, BigDecimal, DateTime<Utc>, Option<DateTime<Utc>>)],
    ) -> HashMap<Uuid, f64> {
        let period_secs = (period_end - period_start).num_seconds() as f64;
        if period_secs <= 0.0 {
            return HashMap::new();
        }

        // Compute time-weighted balance for each account
        let weighted: Vec<(Uuid, f64)> = account_balances
            .iter()
            .map(|(id, balance, deposit_ts, withdrawal_ts)| {
                // Clamp deposit/withdrawal to period boundaries
                let effective_start = (*deposit_ts).max(period_start);
                let effective_end = withdrawal_ts
                    .map(|w| w.min(period_end))
                    .unwrap_or(period_end);

                let active_secs = (effective_end - effective_start)
                    .num_seconds()
                    .max(0) as f64;

                let balance_f64: f64 = balance.to_string().parse().unwrap_or(0.0);
                let time_weighted = balance_f64 * (active_secs / period_secs);
                (*id, time_weighted)
            })
            .collect();

        let total_weighted: f64 = weighted.iter().map(|(_, w)| w).sum();
        if total_weighted <= 0.0 {
            return HashMap::new();
        }

        weighted
            .into_iter()
            .map(|(id, w)| (id, w / total_weighted))
            .collect()
    }

    /// Distribute net yield pro-rata across accounts
    pub fn distribute_pro_rata(
        &self,
        net_yield: &BigDecimal,
        shares: &HashMap<Uuid, f64>,
    ) -> HashMap<Uuid, BigDecimal> {
        let net_f64: f64 = net_yield.to_string().parse().unwrap_or(0.0);
        let mut distributions: HashMap<Uuid, BigDecimal> = shares
            .iter()
            .map(|(id, share)| {
                let amount = net_f64 * share;
                let bd = BigDecimal::from((amount * 1e8) as i64) / BigDecimal::from(100_000_000_i64);
                (*id, bd)
            })
            .collect();

        // Apply rounding correction to largest account to ensure sum == net_yield
        let distributed_sum: BigDecimal = distributions.values().cloned().sum();
        let discrepancy = net_yield.clone() - distributed_sum;
        if discrepancy != BigDecimal::from(0) {
            if let Some(max_id) = shares
                .iter()
                .max_by(|a, b| a.1.partial_cmp(b.1).unwrap_or(std::cmp::Ordering::Equal))
                .map(|(id, _)| *id)
            {
                let entry = distributions.entry(max_id).or_insert(BigDecimal::from(0));
                *entry = entry.clone() + discrepancy;
            }
        }

        distributions
    }

    // ── Tiered Rate Application ───────────────────────────────────────────────

    /// Compute yield entitlement for an account under tiered rates.
    /// Returns (rate_applied, yield_amount).
    pub fn compute_tiered_yield(
        &self,
        balance: &BigDecimal,
        period_secs: i64,
        tiers: &[YieldTierConfig],
    ) -> (f64, BigDecimal) {
        // Find the applicable tier (highest tier whose min_balance <= balance)
        let applicable_tier = tiers
            .iter()
            .filter(|t| t.is_active && *balance >= t.min_balance)
            .filter(|t| t.max_balance.as_ref().map_or(true, |max| balance <= max))
            .max_by(|a, b| a.annual_rate.partial_cmp(&b.annual_rate).unwrap_or(std::cmp::Ordering::Equal));

        let rate = applicable_tier.map(|t| t.annual_rate).unwrap_or(0.0);
        let balance_f64: f64 = balance.to_string().parse().unwrap_or(0.0);
        let period_fraction = period_secs as f64 / (365.0 * 24.0 * 3600.0);
        let yield_amount = balance_f64 * rate * period_fraction;
        let yield_bd = BigDecimal::from((yield_amount * 1e8) as i64) / BigDecimal::from(100_000_000_i64);

        (rate, yield_bd)
    }

    /// Apply proportional haircut if total tier-adjusted yield exceeds available net yield
    pub fn apply_tier_rate_cap(
        &self,
        entitlements: &HashMap<Uuid, BigDecimal>,
        net_distributable: &BigDecimal,
    ) -> HashMap<Uuid, BigDecimal> {
        let total_entitlement: BigDecimal = entitlements.values().cloned().sum();
        if total_entitlement <= *net_distributable || total_entitlement == BigDecimal::from(0) {
            return entitlements.clone();
        }

        let net_f64: f64 = net_distributable.to_string().parse().unwrap_or(0.0);
        let total_f64: f64 = total_entitlement.to_string().parse().unwrap_or(1.0);
        let haircut_ratio = net_f64 / total_f64;

        entitlements
            .iter()
            .map(|(id, amount)| {
                let amount_f64: f64 = amount.to_string().parse().unwrap_or(0.0);
                let adjusted = amount_f64 * haircut_ratio;
                let bd = BigDecimal::from((adjusted * 1e8) as i64) / BigDecimal::from(100_000_000_i64);
                (*id, bd)
            })
            .collect()
    }

    // ── Compound Yield ────────────────────────────────────────────────────────

    /// Compute compound yield: add accrued yield to principal before next cycle
    pub fn apply_compound_reinvestment(
        &self,
        principal: &BigDecimal,
        accrued_yield: &BigDecimal,
    ) -> BigDecimal {
        principal.clone() + accrued_yield.clone()
    }

    // ── Smoothed Rate ─────────────────────────────────────────────────────────

    /// Compute a simple moving average of the last N raw rates
    pub fn compute_smoothed_rate(&self, historical_rates: &[f64]) -> f64 {
        if historical_rates.is_empty() {
            return 0.0;
        }
        let window = self.smoothing_window.min(historical_rates.len());
        let recent = &historical_rates[historical_rates.len() - window..];
        recent.iter().sum::<f64>() / window as f64
    }

    // ── Reconciliation ────────────────────────────────────────────────────────

    /// Verify that sum of all distributions equals net distributable yield.
    /// Returns (is_balanced, discrepancy)
    pub fn reconcile(
        &self,
        net_distributable: &BigDecimal,
        account_distributions: &HashMap<Uuid, BigDecimal>,
        treasury_yield: &BigDecimal,
    ) -> (bool, BigDecimal) {
        let total_distributed: BigDecimal =
            account_distributions.values().cloned().sum::<BigDecimal>() + treasury_yield.clone();
        let discrepancy = (net_distributable.clone() - total_distributed).abs();
        let is_balanced = discrepancy <= self.reconciliation_tolerance;
        (is_balanced, discrepancy)
    }

    // ── Treasury Attribution ──────────────────────────────────────────────────

    /// Compute treasury yield: platform-owned position yield minus management fee obligations
    pub fn compute_treasury_yield(
        &self,
        platform_position_yield: &BigDecimal,
        management_fee_rate: f64,
    ) -> (BigDecimal, BigDecimal) {
        let yield_f64: f64 = platform_position_yield.to_string().parse().unwrap_or(0.0);
        let fee = yield_f64 * management_fee_rate;
        let fee_bd = BigDecimal::from((fee * 1e8) as i64) / BigDecimal::from(100_000_000_i64);
        let net = platform_position_yield.clone() - fee_bd.clone();
        (net, fee_bd)
    }

    // ── Split-Period Calculation ──────────────────────────────────────────────

    /// Handle yield rate change mid-period: split at change point and apply each rate
    pub fn compute_split_period_yield(
        &self,
        balance: &BigDecimal,
        period_start: DateTime<Utc>,
        period_end: DateTime<Utc>,
        rate_change_at: DateTime<Utc>,
        rate_before: f64,
        rate_after: f64,
    ) -> BigDecimal {
        let total_secs = (period_end - period_start).num_seconds() as f64;
        if total_secs <= 0.0 {
            return BigDecimal::from(0);
        }

        let secs_before = ((rate_change_at.min(period_end) - period_start).num_seconds().max(0)) as f64;
        let secs_after = total_secs - secs_before;

        let balance_f64: f64 = balance.to_string().parse().unwrap_or(0.0);
        let year_secs = 365.0 * 24.0 * 3600.0;

        let yield_before = balance_f64 * rate_before * (secs_before / year_secs);
        let yield_after = balance_f64 * rate_after * (secs_after / year_secs);
        let total = yield_before + yield_after;

        BigDecimal::from((total * 1e8) as i64) / BigDecimal::from(100_000_000_i64)
    }
}

// ── HTTP Handlers ─────────────────────────────────────────────────────────────

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::Json,
    routing::{get, patch, post},
    Router,
};

pub struct YieldDistributionHandlers;

impl YieldDistributionHandlers {
    /// GET /api/savings/accounts/:account_id/yield-accruals
    pub async fn get_yield_accruals(
        State(engine): State<Arc<YieldDistributionEngine>>,
        Path(account_id): Path<Uuid>,
        Query(params): Query<YieldAccrualParams>,
    ) -> Result<Json<Vec<YieldAccrualEntry>>, AppError> {
        let limit = params.limit.unwrap_or(100);
        let rows = sqlx::query_as::<_, YieldAccrualEntry>(
            "SELECT * FROM defi_yield_accruals WHERE account_id = $1 ORDER BY credited_at DESC LIMIT $2",
        )
        .bind(account_id)
        .bind(limit)
        .fetch_all(engine.db.as_ref())
        .await
        .map_err(AppError::from)?;
        Ok(Json(rows))
    }

    /// GET /api/savings/products/:product_id/yield-rate
    pub async fn get_yield_rate(
        State(engine): State<Arc<YieldDistributionEngine>>,
        Path(product_id): Path<Uuid>,
        Query(params): Query<YieldRateParams>,
    ) -> Result<Json<YieldRateResponse>, AppError> {
        let limit = params.history_limit.unwrap_or(90);
        let current = sqlx::query_as::<_, EffectiveYieldRate>(
            "SELECT * FROM defi_effective_yield_rates WHERE product_id = $1 ORDER BY computed_at DESC LIMIT 1",
        )
        .bind(product_id)
        .fetch_optional(engine.db.as_ref())
        .await
        .map_err(AppError::from)?;

        let history = sqlx::query_as::<_, EffectiveYieldRate>(
            "SELECT * FROM defi_effective_yield_rates WHERE product_id = $1 ORDER BY computed_at DESC LIMIT $2",
        )
        .bind(product_id)
        .bind(limit)
        .fetch_all(engine.db.as_ref())
        .await
        .map_err(AppError::from)?;

        Ok(Json(YieldRateResponse { current, history }))
    }

    /// GET /api/admin/defi/yield/tier-configuration
    pub async fn get_tier_configuration(
        State(engine): State<Arc<YieldDistributionEngine>>,
        Query(params): Query<TierConfigParams>,
    ) -> Result<Json<Vec<YieldTierConfig>>, AppError> {
        let rows = if let Some(product_id) = params.product_id {
            sqlx::query_as::<_, YieldTierConfig>(
                "SELECT * FROM defi_yield_tier_configs WHERE product_id = $1 AND is_active = true ORDER BY min_balance",
            )
            .bind(product_id)
            .fetch_all(engine.db.as_ref())
            .await
            .map_err(AppError::from)?
        } else {
            sqlx::query_as::<_, YieldTierConfig>(
                "SELECT * FROM defi_yield_tier_configs WHERE is_active = true ORDER BY product_id, min_balance",
            )
            .fetch_all(engine.db.as_ref())
            .await
            .map_err(AppError::from)?
        };
        Ok(Json(rows))
    }

    /// PATCH /api/admin/defi/yield/tier-configuration
    pub async fn update_tier_configuration(
        State(engine): State<Arc<YieldDistributionEngine>>,
        Json(req): Json<UpdateTierConfigRequest>,
    ) -> Result<StatusCode, AppError> {
        sqlx::query!(
            "UPDATE defi_yield_tier_configs SET annual_rate = $1, updated_at = NOW() WHERE tier_id = $2",
            req.annual_rate,
            req.tier_id
        )
        .execute(engine.db.as_ref())
        .await
        .map_err(AppError::from)?;
        Ok(StatusCode::OK)
    }

    /// GET /api/admin/defi/yield/treasury
    pub async fn get_treasury_yield(
        State(engine): State<Arc<YieldDistributionEngine>>,
        Query(params): Query<TreasuryYieldParams>,
    ) -> Result<Json<Vec<TreasuryYieldRecord>>, AppError> {
        let limit = params.limit.unwrap_or(50);
        let rows = sqlx::query_as::<_, TreasuryYieldRecord>(
            "SELECT * FROM defi_treasury_yield_records ORDER BY credited_at DESC LIMIT $1",
        )
        .bind(limit)
        .fetch_all(engine.db.as_ref())
        .await
        .map_err(AppError::from)?;
        Ok(Json(rows))
    }

    /// GET /api/admin/defi/yield/reconciliation
    pub async fn get_reconciliation(
        State(engine): State<Arc<YieldDistributionEngine>>,
    ) -> Result<Json<Option<YieldReconciliationRecord>>, AppError> {
        let record = sqlx::query_as::<_, YieldReconciliationRecord>(
            "SELECT * FROM defi_yield_reconciliation ORDER BY reconciled_at DESC LIMIT 1",
        )
        .fetch_optional(engine.db.as_ref())
        .await
        .map_err(AppError::from)?;
        Ok(Json(record))
    }

    /// GET /api/savings/accounts/:account_id/tax-summary
    pub async fn get_tax_summary(
        State(engine): State<Arc<YieldDistributionEngine>>,
        Path(account_id): Path<Uuid>,
        Query(params): Query<TaxSummaryParams>,
    ) -> Result<Json<YieldTaxSummary>, AppError> {
        let tax_year = params.year.unwrap_or_else(|| Utc::now().format("%Y").to_string().parse().unwrap_or(2026));

        let rows = sqlx::query!(
            r#"
            SELECT
                EXTRACT(MONTH FROM credited_at)::int AS month,
                yield_source_type AS "yield_source_type: String",
                SUM(yield_amount) AS yield_amount,
                SUM(fiat_equivalent) AS fiat_equivalent
            FROM defi_yield_accruals
            WHERE account_id = $1
              AND EXTRACT(YEAR FROM credited_at) = $2
            GROUP BY month, yield_source_type
            ORDER BY month
            "#,
            account_id,
            tax_year as f64
        )
        .fetch_all(engine.db.as_ref())
        .await
        .map_err(AppError::from)?;

        let mut monthly_map: HashMap<u32, (BigDecimal, BigDecimal)> = HashMap::new();
        let mut source_map: HashMap<String, (BigDecimal, BigDecimal)> = HashMap::new();

        for row in &rows {
            let month = row.month.unwrap_or(0) as u32;
            let yield_amt = row.yield_amount.clone().unwrap_or_default();
            let fiat_amt = row.fiat_equivalent.clone().unwrap_or_default();
            let entry = monthly_map.entry(month).or_insert((BigDecimal::from(0), BigDecimal::from(0)));
            entry.0 = entry.0.clone() + yield_amt.clone();
            entry.1 = entry.1.clone() + fiat_amt.clone();
            let src_entry = source_map.entry(row.yield_source_type.clone().unwrap_or_default()).or_insert((BigDecimal::from(0), BigDecimal::from(0)));
            src_entry.0 = src_entry.0.clone() + yield_amt;
            src_entry.1 = src_entry.1.clone() + fiat_amt;
        }

        let total_yield: BigDecimal = monthly_map.values().map(|(y, _)| y.clone()).sum();
        let total_fiat: BigDecimal = monthly_map.values().map(|(_, f)| f.clone()).sum();

        let monthly_breakdown = monthly_map
            .into_iter()
            .map(|(month, (yield_amount, fiat_equivalent))| MonthlyYieldSummary {
                year: tax_year,
                month,
                yield_amount,
                fiat_equivalent,
            })
            .collect();

        let source_breakdown = source_map
            .into_iter()
            .map(|(src, (yield_amount, fiat_equivalent))| SourceYieldSummary {
                source_type: match src.as_str() {
                    "amm_trading_fees" => YieldSourceType::AmmTradingFees,
                    "lending_interest" => YieldSourceType::LendingInterest,
                    "liquidity_mining_incentives" => YieldSourceType::LiquidityMiningIncentives,
                    _ => YieldSourceType::PlatformTreasuryContribution,
                },
                yield_amount,
                fiat_equivalent,
            })
            .collect();

        Ok(Json(YieldTaxSummary {
            account_id,
            tax_year,
            total_yield_earned: total_yield,
            total_fiat_equivalent: total_fiat,
            monthly_breakdown,
            source_breakdown,
        }))
    }
}

// ── DTOs ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct YieldAccrualParams {
    pub limit: Option<i64>,
    pub source_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct YieldRateParams {
    pub history_limit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct YieldRateResponse {
    pub current: Option<EffectiveYieldRate>,
    pub history: Vec<EffectiveYieldRate>,
}

#[derive(Debug, Deserialize)]
pub struct TierConfigParams {
    pub product_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTierConfigRequest {
    pub tier_id: Uuid,
    pub annual_rate: f64,
}

#[derive(Debug, Deserialize)]
pub struct TreasuryYieldParams {
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct TaxSummaryParams {
    pub year: Option<i32>,
}

// ── Routes ────────────────────────────────────────────────────────────────────

pub fn yield_distribution_routes(engine: Arc<YieldDistributionEngine>) -> Router {
    Router::new()
        // Consumer endpoints
        .route("/savings/accounts/:account_id/yield-accruals", get(YieldDistributionHandlers::get_yield_accruals))
        .route("/savings/accounts/:account_id/tax-summary", get(YieldDistributionHandlers::get_tax_summary))
        .route("/savings/products/:product_id/yield-rate", get(YieldDistributionHandlers::get_yield_rate))
        // Admin endpoints
        .route("/admin/defi/yield/tier-configuration", get(YieldDistributionHandlers::get_tier_configuration))
        .route("/admin/defi/yield/tier-configuration", patch(YieldDistributionHandlers::update_tier_configuration))
        .route("/admin/defi/yield/treasury", get(YieldDistributionHandlers::get_treasury_yield))
        .route("/admin/defi/yield/reconciliation", get(YieldDistributionHandlers::get_reconciliation))
        .with_state(engine)
}

// ── Observability ─────────────────────────────────────────────────────────────

pub fn record_distribution_cycle(
    cycle_id: Uuid,
    accounts_credited: usize,
    total_distributed: &BigDecimal,
    is_balanced: bool,
) {
    if is_balanced {
        tracing::info!(
            cycle_id = %cycle_id,
            accounts_credited = accounts_credited,
            total_distributed = %total_distributed,
            "Yield distribution cycle completed — reconciliation OK"
        );
    } else {
        tracing::error!(
            cycle_id = %cycle_id,
            accounts_credited = accounts_credited,
            total_distributed = %total_distributed,
            "Yield distribution cycle completed — RECONCILIATION DISCREPANCY"
        );
    }
}

pub fn record_reconciliation_failure(cycle_id: Uuid, discrepancy: &BigDecimal) {
    tracing::error!(
        cycle_id = %cycle_id,
        discrepancy = %discrepancy,
        "Yield reconciliation discrepancy exceeds tolerance — immediate investigation required"
    );
}

pub fn record_yield_rate_below_minimum(product_id: Uuid, rate: f64, consecutive_cycles: u32) {
    tracing::warn!(
        product_id = %product_id,
        rate = rate,
        consecutive_cycles = consecutive_cycles,
        "Effective yield rate below minimum acceptable threshold"
    );
}

// ── Unit Tests ────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_engine() -> YieldDistributionEngine {
        YieldDistributionEngine {
            db: unsafe { Arc::from_raw(std::ptr::NonNull::dangling().as_ptr()) },
            reconciliation_tolerance: BigDecimal::from(1),
            smoothing_window: 3,
            min_acceptable_rate: 0.01,
            consecutive_cycles_threshold: 3,
        }
    }

    #[test]
    fn test_pro_rata_equal_balances() {
        let engine = make_engine();
        let period_start = Utc::now() - chrono::Duration::hours(24);
        let period_end = Utc::now();
        let deposit_ts = period_start - chrono::Duration::hours(1);

        let id_a = Uuid::new_v4();
        let id_b = Uuid::new_v4();
        let accounts = vec![
            (id_a, BigDecimal::from(1000), deposit_ts, None),
            (id_b, BigDecimal::from(1000), deposit_ts, None),
        ];

        let shares = engine.compute_pro_rata_shares(period_start, period_end, &accounts);
        assert!((shares[&id_a] - 0.5).abs() < 0.001);
        assert!((shares[&id_b] - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_pro_rata_mid_period_deposit() {
        let engine = make_engine();
        let period_start = Utc::now() - chrono::Duration::hours(24);
        let period_end = Utc::now();
        let early_deposit = period_start - chrono::Duration::hours(1);
        let mid_deposit = period_start + chrono::Duration::hours(12); // halfway through

        let id_a = Uuid::new_v4();
        let id_b = Uuid::new_v4();
        let accounts = vec![
            (id_a, BigDecimal::from(1000), early_deposit, None),
            (id_b, BigDecimal::from(1000), mid_deposit, None),
        ];

        let shares = engine.compute_pro_rata_shares(period_start, period_end, &accounts);
        // id_b was active for only half the period, so its share should be ~1/3
        assert!(shares[&id_a] > shares[&id_b]);
        assert!((shares[&id_a] + shares[&id_b] - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_pro_rata_mid_period_withdrawal() {
        let engine = make_engine();
        let period_start = Utc::now() - chrono::Duration::hours(24);
        let period_end = Utc::now();
        let deposit_ts = period_start - chrono::Duration::hours(1);
        let withdrawal_ts = period_start + chrono::Duration::hours(12);

        let id_a = Uuid::new_v4();
        let id_b = Uuid::new_v4();
        let accounts = vec![
            (id_a, BigDecimal::from(1000), deposit_ts, None),
            (id_b, BigDecimal::from(1000), deposit_ts, Some(withdrawal_ts)),
        ];

        let shares = engine.compute_pro_rata_shares(period_start, period_end, &accounts);
        assert!(shares[&id_a] > shares[&id_b]);
    }

    #[test]
    fn test_rounding_correction_sums_to_net_yield() {
        let engine = make_engine();
        let net_yield = BigDecimal::from(100);
        let mut shares = HashMap::new();
        let id_a = Uuid::new_v4();
        let id_b = Uuid::new_v4();
        let id_c = Uuid::new_v4();
        shares.insert(id_a, 1.0 / 3.0);
        shares.insert(id_b, 1.0 / 3.0);
        shares.insert(id_c, 1.0 / 3.0);

        let distributions = engine.distribute_pro_rata(&net_yield, &shares);
        let total: BigDecimal = distributions.values().cloned().sum();
        // Should equal net_yield after rounding correction
        assert_eq!(total, net_yield);
    }

    #[test]
    fn test_tiered_rate_application() {
        let engine = make_engine();
        let tiers = vec![
            YieldTierConfig {
                tier_id: Uuid::new_v4(),
                product_id: Uuid::new_v4(),
                tier_name: "Base".into(),
                min_balance: BigDecimal::from(0),
                max_balance: Some(BigDecimal::from(10_000)),
                annual_rate: 0.05,
                is_active: true,
                updated_at: Utc::now(),
            },
            YieldTierConfig {
                tier_id: Uuid::new_v4(),
                product_id: Uuid::new_v4(),
                tier_name: "Enhanced".into(),
                min_balance: BigDecimal::from(10_001),
                max_balance: None,
                annual_rate: 0.08,
                is_active: true,
                updated_at: Utc::now(),
            },
        ];

        let (rate_small, _) = engine.compute_tiered_yield(&BigDecimal::from(5_000), 86400, &tiers);
        let (rate_large, _) = engine.compute_tiered_yield(&BigDecimal::from(50_000), 86400, &tiers);
        assert!((rate_small - 0.05).abs() < 0.001);
        assert!((rate_large - 0.08).abs() < 0.001);
    }

    #[test]
    fn test_tier_rate_cap_haircut() {
        let engine = make_engine();
        let mut entitlements = HashMap::new();
        let id_a = Uuid::new_v4();
        let id_b = Uuid::new_v4();
        entitlements.insert(id_a, BigDecimal::from(60));
        entitlements.insert(id_b, BigDecimal::from(60));

        let net = BigDecimal::from(100);
        let capped = engine.apply_tier_rate_cap(&entitlements, &net);
        let total: BigDecimal = capped.values().cloned().sum();
        // Total should not exceed net_distributable
        assert!(total <= net + BigDecimal::from(1)); // allow 1 unit rounding
    }

    #[test]
    fn test_compound_yield() {
        let engine = make_engine();
        let principal = BigDecimal::from(1000);
        let accrued = BigDecimal::from(50);
        let compounded = engine.apply_compound_reinvestment(&principal, &accrued);
        assert_eq!(compounded, BigDecimal::from(1050));
    }

    #[test]
    fn test_smoothed_rate_moving_average() {
        let engine = make_engine();
        let rates = vec![0.05, 0.06, 0.04, 0.07, 0.05];
        let smoothed = engine.compute_smoothed_rate(&rates);
        // Last 3: 0.04, 0.07, 0.05 → avg = 0.0533...
        assert!((smoothed - (0.04 + 0.07 + 0.05) / 3.0).abs() < 0.001);
    }

    #[test]
    fn test_reconciliation_balanced() {
        let engine = make_engine();
        let net = BigDecimal::from(1000);
        let mut distributions = HashMap::new();
        distributions.insert(Uuid::new_v4(), BigDecimal::from(800));
        let treasury = BigDecimal::from(200);
        let (is_balanced, discrepancy) = engine.reconcile(&net, &distributions, &treasury);
        assert!(is_balanced);
        assert_eq!(discrepancy, BigDecimal::from(0));
    }

    #[test]
    fn test_reconciliation_discrepancy_detected() {
        let engine = make_engine();
        let net = BigDecimal::from(1000);
        let mut distributions = HashMap::new();
        distributions.insert(Uuid::new_v4(), BigDecimal::from(900));
        let treasury = BigDecimal::from(200); // total = 1100, discrepancy = 100
        let (is_balanced, discrepancy) = engine.reconcile(&net, &distributions, &treasury);
        assert!(!is_balanced);
        assert!(discrepancy > engine.reconciliation_tolerance);
    }

    #[test]
    fn test_split_period_yield_rate_change() {
        let engine = make_engine();
        let period_start = Utc::now() - chrono::Duration::hours(24);
        let period_end = Utc::now();
        let rate_change_at = period_start + chrono::Duration::hours(12);
        let balance = BigDecimal::from(10_000);

        let yield_amount = engine.compute_split_period_yield(
            &balance,
            period_start,
            period_end,
            rate_change_at,
            0.05,
            0.08,
        );
        // Should be positive and non-zero
        assert!(yield_amount > BigDecimal::from(0));
    }

    #[test]
    fn test_treasury_attribution() {
        let engine = make_engine();
        let gross = BigDecimal::from(1000);
        let (net, fee) = engine.compute_treasury_yield(&gross, 0.10);
        assert_eq!(fee, BigDecimal::from(100));
        assert_eq!(net, BigDecimal::from(900));
    }
}
