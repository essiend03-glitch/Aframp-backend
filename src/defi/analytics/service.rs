use std::collections::HashMap;
use std::sync::Arc;
use chrono::{Duration, Utc};
use sqlx::types::BigDecimal;
use uuid::Uuid;

use crate::error::AppError;
use super::models::*;
use super::repository::DefiAnalyticsRepository;
use super::metrics as defi_metrics;

/// Configurable benchmark yield rate (e.g. Nigerian savings account rate ~4%)
const BENCHMARK_YIELD_RATE: f64 = 0.04;

pub struct DefiAnalyticsService {
    repo: Arc<DefiAnalyticsRepository>,
}

impl DefiAnalyticsService {
    pub fn new(repo: Arc<DefiAnalyticsRepository>) -> Self {
        Self { repo }
    }

    // ── Platform Analytics ────────────────────────────────────────────────────

    /// Compute and persist a platform-wide DeFi summary snapshot
    pub async fn compute_platform_snapshot(&self) -> Result<DefiPlatformSnapshot, AppError> {
        let now = Utc::now();
        let period_start = now - Duration::hours(1);

        let savings_tvl = self.repo.sum_savings_deposits().await?;
        let amm_tvl = self.repo.sum_amm_liquidity().await?;
        let collateral = self.repo.sum_lending_collateral().await?;
        let loans = self.repo.sum_outstanding_loans().await?;
        let yield_distributed = self.repo.sum_yield_in_period(period_start, now).await?;
        let wavg_yield = self.repo.weighted_avg_yield_rate().await?;
        let (savings_count, amm_count, lending_count) = self.repo.count_active_positions().await?;

        let total_tvl: BigDecimal = savings_tvl.clone() + amm_tvl.clone() + collateral.clone();

        let snapshot = DefiPlatformSnapshot {
            snapshot_id: Uuid::new_v4(),
            snapshot_at: now,
            period_start,
            period_end: now,
            total_value_locked: total_tvl.clone(),
            total_yield_distributed: yield_distributed,
            weighted_avg_yield_rate: wavg_yield,
            total_amm_liquidity: amm_tvl,
            total_collateral_locked: collateral,
            total_outstanding_loans: loans,
            active_savings_positions: savings_count,
            active_amm_positions: amm_count,
            active_lending_positions: lending_count,
            platform_defi_revenue: BigDecimal::from(0), // computed separately via fee tracking
            created_at: now,
        };

        self.repo.insert_platform_snapshot(&snapshot).await?;

        // Update Prometheus gauges
        if let Ok(tvl_f64) = total_tvl.to_string().parse::<f64>() {
            defi_metrics::set_platform_tvl(tvl_f64);
        }
        defi_metrics::set_weighted_avg_yield_rate(wavg_yield);
        defi_metrics::inc_snapshot_generated();

        tracing::info!(
            tvl = %snapshot.total_value_locked,
            wavg_yield = snapshot.weighted_avg_yield_rate,
            "DeFi platform snapshot computed"
        );

        Ok(snapshot)
    }

    pub async fn get_platform_summary(&self) -> Result<PlatformSummaryResponse, AppError> {
        let history = self.repo.get_platform_snapshot_history(2).await?;
        let current = history.into_iter().next().ok_or_else(|| {
            AppError::NotFound("No platform snapshot available".into())
        })?;

        // Compute deltas vs previous snapshot if available
        let prev = self.repo.get_platform_snapshot_history(2).await?;
        let (tvl_delta, yield_delta, rev_delta) = if prev.len() >= 2 {
            let p = &prev[1];
            let tvl_d = pct_delta(&current.total_value_locked, &p.total_value_locked);
            let y_d = pct_delta(&current.total_yield_distributed, &p.total_yield_distributed);
            let r_d = pct_delta(&current.platform_defi_revenue, &p.platform_defi_revenue);
            (tvl_d, y_d, r_d)
        } else {
            (0.0, 0.0, 0.0)
        };

        Ok(PlatformSummaryResponse {
            current,
            tvl_delta_pct: tvl_delta,
            yield_delta_pct: yield_delta,
            revenue_delta_pct: rev_delta,
        })
    }

    pub async fn get_platform_history(&self, limit: i64) -> Result<Vec<DefiPlatformSnapshot>, AppError> {
        self.repo.get_platform_snapshot_history(limit).await
    }

    // ── Strategy Analytics ────────────────────────────────────────────────────

    pub async fn compute_strategy_snapshot(
        &self,
        strategy_id: Uuid,
        strategy_name: &str,
        total_allocated: BigDecimal,
        yield_earned: BigDecimal,
        effective_yield_rate: f64,
        max_drawdown: f64,
        rebalancing_count: i64,
        protocol_contributions: HashMap<String, f64>,
    ) -> Result<DefiStrategySnapshot, AppError> {
        let now = Utc::now();
        let period_start = now - Duration::hours(24);

        let risk_adjusted_return = compute_risk_adjusted_return(effective_yield_rate, max_drawdown);
        let benchmark_delta = effective_yield_rate - BENCHMARK_YIELD_RATE;

        let snapshot = DefiStrategySnapshot {
            snapshot_id: Uuid::new_v4(),
            strategy_id,
            period_start,
            period_end: now,
            total_allocated,
            yield_earned,
            effective_yield_rate,
            max_drawdown,
            risk_adjusted_return,
            rebalancing_event_count: rebalancing_count as i32,
            protocol_contributions: serde_json::to_value(&protocol_contributions)?,
            benchmark_yield_rate: BENCHMARK_YIELD_RATE,
            benchmark_delta,
            created_at: now,
        };

        self.repo.insert_strategy_snapshot(&snapshot).await?;
        defi_metrics::inc_snapshot_generated();

        tracing::info!(
            strategy_id = %strategy_id,
            strategy_name = %strategy_name,
            effective_yield_rate = effective_yield_rate,
            risk_adjusted_return = risk_adjusted_return,
            "Strategy snapshot computed"
        );

        Ok(snapshot)
    }

    pub async fn get_all_strategies_analytics(&self) -> Result<Vec<DefiStrategySnapshot>, AppError> {
        let mut snapshots = self.repo.get_all_strategy_latest_snapshots().await?;
        // Rank by risk-adjusted return descending
        snapshots.sort_by(|a, b| b.risk_adjusted_return.partial_cmp(&a.risk_adjusted_return).unwrap_or(std::cmp::Ordering::Equal));
        Ok(snapshots)
    }

    pub async fn get_strategy_analytics(
        &self,
        strategy_id: Uuid,
        limit: i64,
    ) -> Result<Vec<DefiStrategySnapshot>, AppError> {
        self.repo.get_strategy_snapshots(strategy_id, limit).await
    }

    pub async fn get_yield_attribution(&self, strategy_id: Uuid) -> Result<YieldAttributionResponse, AppError> {
        let snapshots = self.repo.get_strategy_snapshots(strategy_id, 90).await?;

        let mut protocol_totals: HashMap<String, f64> = HashMap::new();
        let mut period_contributions = Vec::new();
        let mut grand_total = 0.0f64;

        for s in &snapshots {
            let yield_f64: f64 = s.yield_earned.to_string().parse().unwrap_or(0.0);
            grand_total += yield_f64;

            if let Some(obj) = s.protocol_contributions.as_object() {
                for (k, v) in obj {
                    let pct = v.as_f64().unwrap_or(0.0);
                    *protocol_totals.entry(k.clone()).or_insert(0.0) += pct * yield_f64;
                }
            }

            period_contributions.push(PeriodContribution {
                period_start: s.period_start,
                period_end: s.period_end,
                yield_earned: s.yield_earned.clone(),
                pct_of_total: 0.0, // filled below
            });
        }

        // Normalise protocol totals to percentages
        let protocol_pcts: HashMap<String, f64> = protocol_totals
            .into_iter()
            .map(|(k, v)| (k, if grand_total > 0.0 { v / grand_total } else { 0.0 }))
            .collect();

        // Fill period pct_of_total
        for pc in &mut period_contributions {
            let y: f64 = pc.yield_earned.to_string().parse().unwrap_or(0.0);
            pc.pct_of_total = if grand_total > 0.0 { y / grand_total } else { 0.0 };
        }

        Ok(YieldAttributionResponse {
            strategy_id,
            protocol_contributions: protocol_pcts,
            period_contributions,
        })
    }

    // ── Protocol Analytics ────────────────────────────────────────────────────

    pub async fn get_all_protocols_analytics(&self) -> Result<Vec<DefiProtocolSnapshot>, AppError> {
        let mut snapshots = self.repo.get_all_protocol_latest_snapshots().await?;
        // Rank by yield_earned descending
        snapshots.sort_by(|a, b| b.yield_earned.partial_cmp(&a.yield_earned).unwrap_or(std::cmp::Ordering::Equal));
        Ok(snapshots)
    }

    pub async fn get_protocol_analytics(
        &self,
        protocol_id: &str,
        limit: i64,
    ) -> Result<Vec<DefiProtocolSnapshot>, AppError> {
        self.repo.get_protocol_snapshots(protocol_id, limit).await
    }

    // ── AMM Analytics ─────────────────────────────────────────────────────────

    pub async fn get_all_amm_pools_analytics(&self) -> Result<Vec<DefiAmmPoolSnapshot>, AppError> {
        self.repo.get_all_amm_pool_latest_snapshots().await
    }

    pub async fn get_amm_pool_analytics(
        &self,
        pool_id: &str,
        limit: i64,
    ) -> Result<Vec<DefiAmmPoolSnapshot>, AppError> {
        self.repo.get_amm_pool_snapshots(pool_id, limit).await
    }

    // ── Lending Analytics ─────────────────────────────────────────────────────

    pub async fn compute_lending_snapshot(&self) -> Result<DefiLendingSnapshot, AppError> {
        let now = Utc::now();
        let period_start = now - Duration::hours(24);

        let total_collateral = self.repo.sum_lending_collateral().await?;
        let total_loans = self.repo.sum_outstanding_loans().await?;
        let avg_hf = self.repo.avg_lending_health_factor().await?;
        let liq_count = self.repo.count_liquidations_in_period(period_start, now).await?;
        let unique_borrowers = self.repo.count_unique_borrowers().await?;

        let total_loans_f64: f64 = total_loans.to_string().parse().unwrap_or(0.0);
        let total_collateral_f64: f64 = total_collateral.to_string().parse().unwrap_or(0.0);
        let ltv = if total_collateral_f64 > 0.0 { total_loans_f64 / total_collateral_f64 } else { 0.0 };
        let liq_rate = if unique_borrowers > 0 { liq_count as f64 / unique_borrowers as f64 } else { 0.0 };
        let avg_loan_size = if unique_borrowers > 0 {
            BigDecimal::from(total_loans_f64 as i64) / BigDecimal::from(unique_borrowers)
        } else {
            BigDecimal::from(0)
        };

        let snapshot = DefiLendingSnapshot {
            snapshot_id: Uuid::new_v4(),
            period_start,
            period_end: now,
            total_collateral,
            total_outstanding_loans: total_loans,
            avg_loan_to_value_ratio: ltv,
            avg_health_factor: avg_hf,
            liquidation_count: liq_count as i32,
            liquidation_rate: liq_rate,
            interest_income: BigDecimal::from(0),
            unique_borrowers: unique_borrowers as i32,
            avg_loan_size,
            created_at: now,
        };

        self.repo.insert_lending_snapshot(&snapshot).await?;

        defi_metrics::set_total_outstanding_loans(total_loans_f64);
        defi_metrics::set_avg_lending_health_factor(avg_hf);
        defi_metrics::inc_snapshot_generated();

        Ok(snapshot)
    }

    pub async fn get_lending_analytics(&self) -> Result<Option<DefiLendingSnapshot>, AppError> {
        self.repo.get_latest_lending_snapshot().await
    }

    pub async fn get_lending_liquidation_analytics(&self) -> Result<Vec<DefiLendingSnapshot>, AppError> {
        self.repo.get_lending_snapshot_history(30).await
    }

    // ── User Analytics ────────────────────────────────────────────────────────

    pub async fn get_user_summary(&self, wallet_id: Uuid) -> Result<DefiUserSnapshot, AppError> {
        let now = Utc::now();
        let period_start = now - Duration::hours(24);

        let (total_deposited, total_yield, net_yield_rate) =
            self.repo.get_user_savings_data(wallet_id).await?;
        let (total_collateral, outstanding_loans) =
            self.repo.get_user_lending_data(wallet_id).await?;

        let deposited_f64: f64 = total_deposited.to_string().parse().unwrap_or(0.0);
        let collateral_f64: f64 = total_collateral.to_string().parse().unwrap_or(0.0);
        let loans_f64: f64 = outstanding_loans.to_string().parse().unwrap_or(0.0);
        let net_position = deposited_f64 + collateral_f64 - loans_f64;

        let snapshot = DefiUserSnapshot {
            snapshot_id: Uuid::new_v4(),
            wallet_id,
            period_start,
            period_end: now,
            total_deposited_savings: total_deposited,
            total_yield_earned: total_yield,
            net_yield_rate,
            total_collateral_locked: total_collateral,
            outstanding_loan_balance: outstanding_loans,
            net_defi_position_value: BigDecimal::from(net_position as i64),
            product_usage: serde_json::json!({}),
            created_at: now,
        };

        self.repo.upsert_user_snapshot(&snapshot).await?;
        Ok(snapshot)
    }

    pub async fn get_user_yield_history(
        &self,
        wallet_id: Uuid,
        limit: i64,
    ) -> Result<Vec<DefiUserSnapshot>, AppError> {
        self.repo.get_user_yield_history(wallet_id, limit).await
    }

    // ── Reports ───────────────────────────────────────────────────────────────

    pub async fn generate_report(
        &self,
        report_type: &str,
    ) -> Result<DefiAnalyticsReport, AppError> {
        let now = Utc::now();
        let (period_start, period_end) = match report_type {
            "weekly" => (now - Duration::weeks(1), now),
            "monthly" => (now - Duration::days(30), now),
            "quarterly" => (now - Duration::days(90), now),
            _ => return Err(AppError::BadRequest("Invalid report type".into())),
        };

        let report = DefiAnalyticsReport {
            report_id: Uuid::new_v4(),
            report_type: report_type.to_string(),
            period_start,
            period_end,
            status: "pending".to_string(),
            report_data: None,
            download_url: None,
            generated_at: None,
            created_at: now,
        };

        self.repo.insert_report(&report).await?;

        // Build report data from latest snapshots
        let platform = self.repo.get_platform_snapshot_history(1).await?;
        let strategies = self.repo.get_all_strategy_latest_snapshots().await?;
        let protocols = self.repo.get_all_protocol_latest_snapshots().await?;
        let lending = self.repo.get_latest_lending_snapshot().await?;

        let data = serde_json::json!({
            "period_start": period_start,
            "period_end": period_end,
            "platform": platform.first(),
            "strategy_count": strategies.len(),
            "top_strategies": strategies.iter().take(5).collect::<Vec<_>>(),
            "protocol_count": protocols.len(),
            "lending": lending,
        });

        self.repo.update_report_ready(report.report_id, data).await?;
        defi_metrics::inc_report_generated(report_type);

        tracing::info!(report_type = %report_type, report_id = %report.report_id, "DeFi analytics report generated");

        Ok(report)
    }

    pub async fn list_reports(&self) -> Result<Vec<DefiAnalyticsReport>, AppError> {
        self.repo.list_reports().await
    }

    // ── Export ────────────────────────────────────────────────────────────────

    pub async fn request_platform_export(
        &self,
        requester_id: &str,
        req: ExportRequest,
    ) -> Result<ExportResponse, AppError> {
        let export_id = Uuid::new_v4();
        let metrics = serde_json::to_value(&req.metric_set)?;
        self.repo.insert_export_request(
            export_id, requester_id, "platform",
            req.date_range_start, req.date_range_end, &metrics,
        ).await?;
        defi_metrics::inc_export_requested("platform");
        Ok(ExportResponse {
            export_id,
            status: "pending".to_string(),
            message: "Export queued. You will be notified when ready.".to_string(),
        })
    }

    pub async fn request_user_export(
        &self,
        wallet_id: Uuid,
        req: ExportRequest,
    ) -> Result<ExportResponse, AppError> {
        let export_id = Uuid::new_v4();
        let metrics = serde_json::to_value(&req.metric_set)?;
        self.repo.insert_export_request(
            export_id, &wallet_id.to_string(), "user",
            req.date_range_start, req.date_range_end, &metrics,
        ).await?;
        defi_metrics::inc_export_requested("user");
        Ok(ExportResponse {
            export_id,
            status: "pending".to_string(),
            message: "Export queued. You will be notified when ready.".to_string(),
        })
    }
}

// ── Pure computation helpers ──────────────────────────────────────────────────

/// Compute risk-adjusted return (simplified Sharpe-like: yield / (1 + drawdown))
pub fn compute_risk_adjusted_return(effective_yield_rate: f64, max_drawdown: f64) -> f64 {
    if max_drawdown >= 1.0 {
        return 0.0;
    }
    effective_yield_rate / (1.0 + max_drawdown)
}

/// Compute weighted average yield rate
pub fn compute_weighted_avg_yield_rate(products: &[(f64, f64)]) -> f64 {
    // products: Vec<(yield_rate, deposited_amount)>
    let total_weight: f64 = products.iter().map(|(_, w)| w).sum();
    if total_weight == 0.0 {
        return 0.0;
    }
    products.iter().map(|(r, w)| r * w).sum::<f64>() / total_weight
}

/// Compute AMM capital efficiency: volume / liquidity
pub fn compute_amm_capital_efficiency(trading_volume: f64, liquidity: f64) -> f64 {
    if liquidity == 0.0 { 0.0 } else { trading_volume / liquidity }
}

/// Compute impermanent loss vs hold strategy
/// Returns the difference: actual_yield - hold_return (negative = IL cost)
pub fn compute_il_vs_hold(fee_income: f64, impermanent_loss: f64, hold_return: f64) -> f64 {
    (fee_income - impermanent_loss) - hold_return
}

/// Compute liquidation rate: liquidations / total_borrowers
pub fn compute_liquidation_rate(liquidation_count: u64, total_borrowers: u64) -> f64 {
    if total_borrowers == 0 { 0.0 } else { liquidation_count as f64 / total_borrowers as f64 }
}

/// Compute protocol efficiency: yield per unit of capital
pub fn compute_protocol_efficiency(yield_earned: f64, capital_deployed: f64) -> f64 {
    if capital_deployed == 0.0 { 0.0 } else { yield_earned / capital_deployed }
}

/// Compute benchmark comparison delta
pub fn compute_benchmark_delta(effective_yield_rate: f64, benchmark_rate: f64) -> f64 {
    effective_yield_rate - benchmark_rate
}

fn pct_delta(current: &BigDecimal, previous: &BigDecimal) -> f64 {
    let c: f64 = current.to_string().parse().unwrap_or(0.0);
    let p: f64 = previous.to_string().parse().unwrap_or(0.0);
    if p == 0.0 { 0.0 } else { (c - p) / p * 100.0 }
}
