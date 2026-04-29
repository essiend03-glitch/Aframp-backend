use prometheus::{Counter, Gauge, Histogram, IntCounter, IntGauge, Registry};
use std::collections::HashMap;
use std::sync::Arc;
use chrono::Utc;
use uuid::Uuid;

use crate::error::AppError;
use super::*;

/// DeFi observability and monitoring system
pub struct DeFiObservability {
    registry: Registry,
    metrics: DeFiMetrics,
}

impl DeFiObservability {
    pub fn new() -> Self {
        let registry = Registry::new();
        let metrics = DeFiMetrics::new(&registry);
        
        Self { registry, metrics }
    }

    /// Get Prometheus metrics registry
    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// Record strategy creation
    pub fn record_strategy_created(&self, strategy_type: StrategyType) {
        self.metrics.strategies_created_total
            .with_label_values(&[&format!("{:?}", strategy_type)])
            .inc();
        
        self.metrics.active_strategies_total.inc();
        tracing::info!(strategy_type = ?strategy_type, "DeFi strategy created");
    }

    /// Record strategy activation
    pub fn record_strategy_activated(&self, strategy_id: Uuid, total_allocation: f64) {
        self.metrics.strategies_activated_total.inc();
        self.metrics.strategy_total_allocation
            .with_label_values(&[&strategy_id.to_string()])
            .set(total_allocation);
        
        tracing::info!(
            strategy_id = %strategy_id,
            total_allocation = total_allocation,
            "DeFi strategy activated"
        );
    }

    /// Record strategy performance
    pub fn record_strategy_performance(&self, strategy_id: Uuid, yield_rate: f64, drawdown: f64) {
        self.metrics.strategy_yield_rate
            .with_label_values(&[&strategy_id.to_string()])
            .set(yield_rate);
        
        self.metrics.strategy_max_drawdown
            .with_label_values(&[&strategy_id.to_string()])
            .set(drawdown);
    }

    /// Record rebalancing event
    pub fn record_rebalancing_event(&self, strategy_id: Uuid, trigger_reason: &str, duration_ms: u64) {
        self.metrics.rebalancing_events_total
            .with_label_values(&[trigger_reason])
            .inc();
        
        self.metrics.rebalancing_duration
            .with_label_values(&[&strategy_id.to_string()])
            .observe(duration_ms as f64 / 1000.0);
        
        tracing::info!(
            strategy_id = %strategy_id,
            trigger_reason = %trigger_reason,
            duration_ms = duration_ms,
            "Strategy rebalancing completed"
        );
    }

    /// Record circuit breaker activation
    pub fn record_circuit_breaker_activation(&self, protocol_id: &str, trigger: CircuitBreakerTrigger) {
        self.metrics.circuit_breaker_activations_total
            .with_label_values(&[protocol_id, &format!("{:?}", trigger)])
            .inc();
        
        self.metrics.circuit_breaker_status
            .with_label_values(&[protocol_id])
            .set(1.0); // 1.0 = tripped
        
        tracing::warn!(
            protocol_id = %protocol_id,
            trigger = ?trigger,
            "Circuit breaker activated"
        );
    }

    /// Record circuit breaker reset
    pub fn record_circuit_breaker_reset(&self, protocol_id: &str) {
        self.metrics.circuit_breaker_resets_total
            .with_label_values(&[protocol_id])
            .inc();
        
        self.metrics.circuit_breaker_status
            .with_label_values(&[protocol_id])
            .set(0.0); // 0.0 = normal
        
        tracing::info!(protocol_id = %protocol_id, "Circuit breaker reset");
    }

    /// Record governance approval
    pub fn record_governance_approval(&self, strategy_id: Uuid, committee_member: &str, approval_type: ApprovalType) {
        self.metrics.governance_approvals_total
            .with_label_values(&[&format!("{:?}", approval_type)])
            .inc();
        
        tracing::info!(
            strategy_id = %strategy_id,
            committee_member = %committee_member,
            approval_type = ?approval_type,
            "Governance approval recorded"
        );
    }

    /// Record savings account creation
    pub fn record_savings_account_created(&self, product_id: Uuid, deposit_amount: f64) {
        self.metrics.savings_accounts_created_total.inc();
        self.metrics.savings_total_deposited
            .with_label_values(&[&product_id.to_string()])
            .add(deposit_amount);
        
        tracing::info!(
            product_id = %product_id,
            deposit_amount = deposit_amount,
            "cNGN savings account created"
        );
    }

    /// Record savings deposit
    pub fn record_savings_deposit(&self, account_id: Uuid, amount: f64) {
        self.metrics.savings_deposits_total.inc();
        self.metrics.savings_total_deposited
            .with_label_values(&[&account_id.to_string()])
            .add(amount);
        
        tracing::info!(account_id = %account_id, amount = amount, "cNGN savings deposit");
    }

    /// Record savings withdrawal
    pub fn record_savings_withdrawal(&self, account_id: Uuid, amount: f64, early_withdrawal: bool) {
        let withdrawal_type = if early_withdrawal { "early" } else { "normal" };
        
        self.metrics.savings_withdrawals_total
            .with_label_values(&[withdrawal_type])
            .inc();
        
        self.metrics.savings_total_withdrawn
            .with_label_values(&[&account_id.to_string()])
            .add(amount);
        
        tracing::info!(
            account_id = %account_id,
            amount = amount,
            early_withdrawal = early_withdrawal,
            "cNGN savings withdrawal"
        );
    }

    /// Record yield accrual
    pub fn record_yield_accrual(&self, account_id: Uuid, yield_amount: f64, yield_rate: f64) {
        self.metrics.yield_accrual_events_total.inc();
        self.metrics.yield_accrual_amount
            .with_label_values(&[&account_id.to_string()])
            .add(yield_amount);
        
        self.metrics.savings_yield_rate
            .with_label_values(&[&account_id.to_string()])
            .set(yield_rate);
        
        tracing::debug!(
            account_id = %account_id,
            yield_amount = yield_amount,
            yield_rate = yield_rate,
            "Yield accrued"
        );
    }

    /// Record AMM pool discovery
    pub fn record_amm_pool_discovered(&self, pool_id: &str, asset_pair: &str) {
        self.metrics.amm_pools_discovered_total.inc();
        self.metrics.active_amm_pools_total.inc();
        
        tracing::info!(
            pool_id = %pool_id,
            asset_pair = %asset_pair,
            "AMM pool discovered"
        );
    }

    /// Record AMM liquidity position creation
    pub fn record_amm_position_created(&self, pool_id: &str, position_value: f64) {
        self.metrics.amm_positions_created_total.inc();
        self.metrics.amm_total_liquidity
            .with_label_values(&[pool_id])
            .add(position_value);
        
        tracing::info!(
            pool_id = %pool_id,
            position_value = position_value,
            "AMM liquidity position created"
        );
    }

    /// Record AMM swap execution
    pub fn record_amm_swap(&self, pool_id: &str, input_amount: f64, output_amount: f64, slippage_pct: f64) {
        self.metrics.amm_swaps_total
            .with_label_values(&[pool_id])
            .inc();
        
        self.metrics.amm_swap_volume
            .with_label_values(&[pool_id])
            .add(input_amount);
        
        self.metrics.amm_swap_slippage
            .with_label_values(&[pool_id])
            .observe(slippage_pct);
        
        tracing::info!(
            pool_id = %pool_id,
            input_amount = input_amount,
            output_amount = output_amount,
            slippage_pct = slippage_pct,
            "AMM swap executed"
        );
    }

    /// Record protocol health metrics
    pub fn record_protocol_health(&self, protocol_id: &str, health_score: f64, tvl: f64) {
        self.metrics.protocol_health_score
            .with_label_values(&[protocol_id])
            .set(health_score);
        
        self.metrics.protocol_tvl
            .with_label_values(&[protocol_id])
            .set(tvl);
        
        if health_score < 0.5 {
            tracing::warn!(
                protocol_id = %protocol_id,
                health_score = health_score,
                "Protocol health score below threshold"
            );
        }
    }

    /// Record treasury exposure
    pub fn record_treasury_exposure(&self, total_exposure: f64, protocol_exposures: HashMap<String, f64>) {
        self.metrics.treasury_defi_exposure.set(total_exposure);
        
        for (protocol_id, exposure) in protocol_exposures {
            self.metrics.treasury_protocol_exposure
                .with_label_values(&[&protocol_id])
                .set(exposure);
        }
        
        // Alert if exposure exceeds limits
        if total_exposure > 30.0 {
            tracing::warn!(
                total_exposure = total_exposure,
                "Treasury DeFi exposure exceeds safe limit"
            );
        }
    }

    /// Record risk metrics
    pub fn record_risk_metrics(&self, weighted_risk_score: f64, concentration_risk: f64, correlation_risk: f64) {
        self.metrics.risk_weighted_score.set(weighted_risk_score);
        self.metrics.risk_concentration.set(concentration_risk);
        self.metrics.risk_correlation.set(correlation_risk);
        
        if weighted_risk_score > 0.7 {
            tracing::warn!(
                weighted_risk_score = weighted_risk_score,
                "High overall risk score detected"
            );
        }
    }

    /// Record impermanent loss
    pub fn record_impermanent_loss(&self, position_id: Uuid, pool_id: &str, loss_pct: f64) {
        self.metrics.impermanent_loss
            .with_label_values(&[&position_id.to_string(), pool_id])
            .set(loss_pct);
        
        if loss_pct > 0.10 {
            tracing::warn!(
                position_id = %position_id,
                pool_id = %pool_id,
                loss_pct = loss_pct,
                "High impermanent loss detected"
            );
        }
    }

    /// Record error
    pub fn record_error(&self, error_type: &str, component: &str) {
        self.metrics.errors_total
            .with_label_values(&[error_type, component])
            .inc();
        
        tracing::error!(
            error_type = %error_type,
            component = %component,
            "DeFi operation error"
        );
    }
}

/// DeFi Prometheus metrics
pub struct DeFiMetrics {
    // Strategy metrics
    pub strategies_created_total: IntCounter,
    pub strategies_activated_total: IntCounter,
    pub active_strategies_total: IntGauge,
    pub strategy_total_allocation: Gauge,
    pub strategy_yield_rate: Gauge,
    pub strategy_max_drawdown: Gauge,
    
    // Rebalancing metrics
    pub rebalancing_events_total: IntCounter,
    pub rebalancing_duration: Histogram,
    
    // Circuit breaker metrics
    pub circuit_breaker_activations_total: IntCounter,
    pub circuit_breaker_resets_total: IntCounter,
    pub circuit_breaker_status: Gauge,
    
    // Governance metrics
    pub governance_approvals_total: IntCounter,
    
    // Savings metrics
    pub savings_accounts_created_total: IntCounter,
    pub savings_deposits_total: IntCounter,
    pub savings_withdrawals_total: IntCounter,
    pub savings_total_deposited: Gauge,
    pub savings_total_withdrawn: Gauge,
    pub yield_accrual_events_total: IntCounter,
    pub yield_accrual_amount: Gauge,
    pub savings_yield_rate: Gauge,
    
    // AMM metrics
    pub amm_pools_discovered_total: IntCounter,
    pub active_amm_pools_total: IntGauge,
    pub amm_positions_created_total: IntCounter,
    pub amm_total_liquidity: Gauge,
    pub amm_swaps_total: IntCounter,
    pub amm_swap_volume: Gauge,
    pub amm_swap_slippage: Histogram,
    
    // Protocol health metrics
    pub protocol_health_score: Gauge,
    pub protocol_tvl: Gauge,
    
    // Treasury metrics
    pub treasury_defi_exposure: Gauge,
    pub treasury_protocol_exposure: Gauge,
    
    // Risk metrics
    pub risk_weighted_score: Gauge,
    pub risk_concentration: Gauge,
    pub risk_correlation: Gauge,
    
    // Error metrics
    pub errors_total: IntCounter,
    
    // Impermanent loss metrics
    pub impermanent_loss: Gauge,
}

impl DeFiMetrics {
    fn new(registry: &Registry) -> Self {
        let strategies_created_total = IntCounter::new(
            "defi_strategies_created_total",
            "Total number of DeFi strategies created"
        ).unwrap();
        
        let strategies_activated_total = IntCounter::new(
            "defi_strategies_activated_total",
            "Total number of DeFi strategies activated"
        ).unwrap();
        
        let active_strategies_total = IntGauge::new(
            "defi_active_strategies_total",
            "Current number of active DeFi strategies"
        ).unwrap();
        
        let strategy_total_allocation = Gauge::new(
            "defi_strategy_total_allocation",
            "Total allocation amount for a strategy"
        ).unwrap();
        
        let strategy_yield_rate = Gauge::new(
            "defi_strategy_yield_rate",
            "Current yield rate for a strategy"
        ).unwrap();
        
        let strategy_max_drawdown = Gauge::new(
            "defi_strategy_max_drawdown",
            "Maximum drawdown for a strategy"
        ).unwrap();
        
        let rebalancing_events_total = IntCounter::new(
            "defi_rebalancing_events_total",
            "Total number of rebalancing events"
        ).unwrap();
        
        let rebalancing_duration = Histogram::with_opts(
            prometheus::HistogramOpts::new(
                "defi_rebalancing_duration_seconds",
                "Duration of rebalancing operations"
            ).buckets(vec![1.0, 5.0, 10.0, 30.0, 60.0, 300.0])
        ).unwrap();
        
        let circuit_breaker_activations_total = IntCounter::new(
            "defi_circuit_breaker_activations_total",
            "Total number of circuit breaker activations"
        ).unwrap();
        
        let circuit_breaker_resets_total = IntCounter::new(
            "defi_circuit_breaker_resets_total",
            "Total number of circuit breaker resets"
        ).unwrap();
        
        let circuit_breaker_status = Gauge::new(
            "defi_circuit_breaker_status",
            "Circuit breaker status (0=normal, 1=tripped)"
        ).unwrap();
        
        let governance_approvals_total = IntCounter::new(
            "defi_governance_approvals_total",
            "Total number of governance approvals"
        ).unwrap();
        
        let savings_accounts_created_total = IntCounter::new(
            "defi_savings_accounts_created_total",
            "Total number of savings accounts created"
        ).unwrap();
        
        let savings_deposits_total = IntCounter::new(
            "defi_savings_deposits_total",
            "Total number of savings deposits"
        ).unwrap();
        
        let savings_withdrawals_total = IntCounter::new(
            "defi_savings_withdrawals_total",
            "Total number of savings withdrawals"
        ).unwrap();
        
        let savings_total_deposited = Gauge::new(
            "defi_savings_total_deposited",
            "Total amount deposited in savings accounts"
        ).unwrap();
        
        let savings_total_withdrawn = Gauge::new(
            "defi_savings_total_withdrawn",
            "Total amount withdrawn from savings accounts"
        ).unwrap();
        
        let yield_accrual_events_total = IntCounter::new(
            "defi_yield_accrual_events_total",
            "Total number of yield accrual events"
        ).unwrap();
        
        let yield_accrual_amount = Gauge::new(
            "defi_yield_accrual_amount",
            "Amount of yield accrued"
        ).unwrap();
        
        let savings_yield_rate = Gauge::new(
            "defi_savings_yield_rate",
            "Current yield rate for savings accounts"
        ).unwrap();
        
        let amm_pools_discovered_total = IntCounter::new(
            "defi_amm_pools_discovered_total",
            "Total number of AMM pools discovered"
        ).unwrap();
        
        let active_amm_pools_total = IntGauge::new(
            "defi_active_amm_pools_total",
            "Current number of active AMM pools"
        ).unwrap();
        
        let amm_positions_created_total = IntCounter::new(
            "defi_amm_positions_created_total",
            "Total number of AMM positions created"
        ).unwrap();
        
        let amm_total_liquidity = Gauge::new(
            "defi_amm_total_liquidity",
            "Total liquidity in AMM positions"
        ).unwrap();
        
        let amm_swaps_total = IntCounter::new(
            "defi_amm_swaps_total",
            "Total number of AMM swaps"
        ).unwrap();
        
        let amm_swap_volume = Gauge::new(
            "defi_amm_swap_volume",
            "Total volume of AMM swaps"
        ).unwrap();
        
        let amm_swap_slippage = Histogram::with_opts(
            prometheus::HistogramOpts::new(
                "defi_amm_swap_slippage_percentage",
                "Slippage percentage of AMM swaps"
            ).buckets(vec![0.001, 0.005, 0.01, 0.02, 0.05, 0.1, 0.2])
        ).unwrap();
        
        let protocol_health_score = Gauge::new(
            "defi_protocol_health_score",
            "Health score for DeFi protocols"
        ).unwrap();
        
        let protocol_tvl = Gauge::new(
            "defi_protocol_tvl",
            "Total value locked for DeFi protocols"
        ).unwrap();
        
        let treasury_defi_exposure = Gauge::new(
            "defi_treasury_defi_exposure_percentage",
            "Percentage of treasury exposed to DeFi"
        ).unwrap();
        
        let treasury_protocol_exposure = Gauge::new(
            "defi_treasury_protocol_exposure_percentage",
            "Percentage of treasury exposed to each protocol"
        ).unwrap();
        
        let risk_weighted_score = Gauge::new(
            "defi_risk_weighted_score",
            "Weighted risk score for DeFi portfolio"
        ).unwrap();
        
        let risk_concentration = Gauge::new(
            "defi_risk_concentration_score",
            "Concentration risk score"
        ).unwrap();
        
        let risk_correlation = Gauge::new(
            "defi_risk_correlation_score",
            "Correlation risk score"
        ).unwrap();
        
        let errors_total = IntCounter::new(
            "defi_errors_total",
            "Total number of DeFi operation errors"
        ).unwrap();
        
        let impermanent_loss = Gauge::new(
            "defi_impermanent_loss_percentage",
            "Impermanent loss percentage for AMM positions"
        ).unwrap();
        
        // Register all metrics
        registry.register(Box::new(strategies_created_total.clone())).unwrap();
        registry.register(Box::new(strategies_activated_total.clone())).unwrap();
        registry.register(Box::new(active_strategies_total.clone())).unwrap();
        registry.register(Box::new(strategy_total_allocation.clone())).unwrap();
        registry.register(Box::new(strategy_yield_rate.clone())).unwrap();
        registry.register(Box::new(strategy_max_drawdown.clone())).unwrap();
        registry.register(Box::new(rebalancing_events_total.clone())).unwrap();
        registry.register(Box::new(rebalancing_duration.clone())).unwrap();
        registry.register(Box::new(circuit_breaker_activations_total.clone())).unwrap();
        registry.register(Box::new(circuit_breaker_resets_total.clone())).unwrap();
        registry.register(Box::new(circuit_breaker_status.clone())).unwrap();
        registry.register(Box::new(governance_approvals_total.clone())).unwrap();
        registry.register(Box::new(savings_accounts_created_total.clone())).unwrap();
        registry.register(Box::new(savings_deposits_total.clone())).unwrap();
        registry.register(Box::new(savings_withdrawals_total.clone())).unwrap();
        registry.register(Box::new(savings_total_deposited.clone())).unwrap();
        registry.register(Box::new(savings_total_withdrawn.clone())).unwrap();
        registry.register(Box::new(yield_accrual_events_total.clone())).unwrap();
        registry.register(Box::new(yield_accrual_amount.clone())).unwrap();
        registry.register(Box::new(savings_yield_rate.clone())).unwrap();
        registry.register(Box::new(amm_pools_discovered_total.clone())).unwrap();
        registry.register(Box::new(active_amm_pools_total.clone())).unwrap();
        registry.register(Box::new(amm_positions_created_total.clone())).unwrap();
        registry.register(Box::new(amm_total_liquidity.clone())).unwrap();
        registry.register(Box::new(amm_swaps_total.clone())).unwrap();
        registry.register(Box::new(amm_swap_volume.clone())).unwrap();
        registry.register(Box::new(amm_swap_slippage.clone())).unwrap();
        registry.register(Box::new(protocol_health_score.clone())).unwrap();
        registry.register(Box::new(protocol_tvl.clone())).unwrap();
        registry.register(Box::new(treasury_defi_exposure.clone())).unwrap();
        registry.register(Box::new(treasury_protocol_exposure.clone())).unwrap();
        registry.register(Box::new(risk_weighted_score.clone())).unwrap();
        registry.register(Box::new(risk_concentration.clone())).unwrap();
        registry.register(Box::new(risk_correlation.clone())).unwrap();
        registry.register(Box::new(errors_total.clone())).unwrap();
        registry.register(Box::new(impermanent_loss.clone())).unwrap();
        
        Self {
            strategies_created_total,
            strategies_activated_total,
            active_strategies_total,
            strategy_total_allocation,
            strategy_yield_rate,
            strategy_max_drawdown,
            rebalancing_events_total,
            rebalancing_duration,
            circuit_breaker_activations_total,
            circuit_breaker_resets_total,
            circuit_breaker_status,
            governance_approvals_total,
            savings_accounts_created_total,
            savings_deposits_total,
            savings_withdrawals_total,
            savings_total_deposited,
            savings_total_withdrawn,
            yield_accrual_events_total,
            yield_accrual_amount,
            savings_yield_rate,
            amm_pools_discovered_total,
            active_amm_pools_total,
            amm_positions_created_total,
            amm_total_liquidity,
            amm_swaps_total,
            amm_swap_volume,
            amm_swap_slippage,
            protocol_health_score,
            protocol_tvl,
            treasury_defi_exposure,
            treasury_protocol_exposure,
            risk_weighted_score,
            risk_concentration,
            risk_correlation,
            errors_total,
            impermanent_loss,
        }
    }
}

/// Alerting system for DeFi operations
pub struct DeFiAlerting {
    observability: Arc<DeFiObservability>,
    config: AlertingConfig,
}

impl DeFiAlerting {
    pub fn new(observability: Arc<DeFiObservability>, config: AlertingConfig) -> Self {
        Self { observability, config }
    }

    /// Check and trigger alerts based on current metrics
    pub async fn check_alerts(&self) -> Result<Vec<Alert>, AppError> {
        let mut alerts = Vec::new();

        // Check circuit breaker alerts
        if let Some(alert) = self.check_circuit_breaker_alerts().await? {
            alerts.push(alert);
        }

        // Check yield rate floor alerts
        if let Some(alert) = self.check_yield_rate_floor_alerts().await? {
            alerts.push(alert);
        }

        // Check impermanent loss alerts
        if let Some(alert) = self.check_impermanent_loss_alerts().await? {
            alerts.push(alert);
        }

        // Check treasury exposure alerts
        if let Some(alert) = self.check_treasury_exposure_alerts().await? {
            alerts.push(alert);
        }

        // Check protocol health alerts
        if let Some(alert) = self.check_protocol_health_alerts().await? {
            alerts.push(alert);
        }

        Ok(alerts)
    }

    async fn check_circuit_breaker_alerts(&self) -> Result<Option<Alert>, AppError> {
        // Implementation would check for tripped circuit breakers
        // For now, return placeholder
        Ok(None)
    }

    async fn check_yield_rate_floor_alerts(&self) -> Result<Option<Alert>, AppError> {
        // Implementation would check for yield rates below minimum
        // For now, return placeholder
        Ok(None)
    }

    async fn check_impermanent_loss_alerts(&self) -> Result<Option<Alert>, AppError> {
        // Implementation would check for high impermanent loss
        // For now, return placeholder
        Ok(None)
    }

    async fn check_treasury_exposure_alerts(&self) -> Result<Option<Alert>, AppError> {
        // Implementation would check for excessive treasury exposure
        // For now, return placeholder
        Ok(None)
    }

    async fn check_protocol_health_alerts(&self) -> Result<Option<Alert>, AppError> {
        // Implementation would check for low protocol health scores
        // For now, return placeholder
        Ok(None)
    }
}

/// Alert configuration
#[derive(Debug, Clone)]
pub struct AlertingConfig {
    pub circuit_breaker_alert_enabled: bool,
    pub yield_rate_floor_alert_enabled: bool,
    pub impermanent_loss_threshold: f64,
    pub treasury_exposure_threshold: f64,
    pub protocol_health_threshold: f64,
}

impl Default for AlertingConfig {
    fn default() -> Self {
        Self {
            circuit_breaker_alert_enabled: true,
            yield_rate_floor_alert_enabled: true,
            impermanent_loss_threshold: 0.10, // 10%
            treasury_exposure_threshold: 0.30, // 30%
            protocol_health_threshold: 0.5,
        }
    }
}

/// Alert representation
#[derive(Debug, Clone)]
pub struct Alert {
    pub alert_id: Uuid,
    pub alert_type: AlertType,
    pub severity: AlertSeverity,
    pub message: String,
    pub component: String,
    pub triggered_at: DateTime<Utc>,
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub enum AlertType {
    CircuitBreakerTripped,
    YieldRateFloorBreached,
    ImpermanentLossHigh,
    TreasuryExposureHigh,
    ProtocolHealthLow,
}

#[derive(Debug, Clone)]
pub enum AlertSeverity {
    Critical,
    Warning,
    Info,
}
