use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::BigDecimal;
use std::collections::HashMap;
use uuid::Uuid;

use crate::error::AppError;
use super::{DeFiProtocol, ProtocolHealthMetrics, RiskTier};

/// Risk control manager for DeFi operations
pub struct RiskController {
    config: RiskControlConfig,
    circuit_breakers: HashMap<String, CircuitBreaker>,
}

impl RiskController {
    pub fn new(config: RiskControlConfig) -> Self {
        Self {
            config,
            circuit_breakers: HashMap::new(),
        }
    }

    /// Validate a deposit operation against risk controls
    pub async fn validate_deposit(
        &self,
        protocol: &dyn DeFiProtocol,
        amount: &BigDecimal,
        current_exposure: &BigDecimal,
        max_exposure: &BigDecimal,
    ) -> Result<RiskValidationResult, AppError> {
        let mut validations = Vec::new();

        // Check single transaction limit
        let single_tx_check = self.check_single_transaction_limit(amount);
        validations.push(single_tx_check);

        // Check protocol exposure limit
        let exposure_check = self.check_protocol_exposure_limit(
            current_exposure,
            max_exposure,
            amount,
        );
        validations.push(exposure_check);

        // Check protocol health gate
        let health_check = self.check_protocol_health_gate(protocol).await?;
        validations.push(health_check);

        // Check circuit breaker status
        let circuit_breaker_check = self.check_circuit_breaker_status(protocol.protocol_id());
        validations.push(circuit_breaker_check);

        // Check slippage tolerance
        let slippage_check = self.check_slippage_tolerance(self.config.default_slippage_tolerance);
        validations.push(slippage_check);

        // Check risk tier restrictions
        let risk_tier_check = self.check_risk_tier_restrictions(protocol.risk_tier());
        validations.push(risk_tier_check);

        // Aggregate results
        let passed = validations.iter().all(|v| v.passed);
        let failed_validations = validations.iter().filter(|v| !v.passed).cloned().collect();

        Ok(RiskValidationResult {
            passed,
            validations,
            failed_validations,
            risk_score: self.calculate_risk_score(&validations),
        })
    }

    /// Validate a withdrawal operation
    pub async fn validate_withdrawal(
        &self,
        protocol: &dyn DeFiProtocol,
        amount: &BigDecimal,
    ) -> Result<RiskValidationResult, AppError> {
        let mut validations = Vec::new();

        // Check single transaction limit
        let single_tx_check = self.check_single_transaction_limit(amount);
        validations.push(single_tx_check);

        // Check minimum withdrawal amount
        let min_withdrawal_check = self.check_minimum_withdrawal_amount(amount);
        validations.push(min_withdrawal_check);

        // Check circuit breaker status (emergency withdrawal allowed)
        let circuit_breaker_check = self.check_circuit_breaker_withdrawal_status(protocol.protocol_id());
        validations.push(circuit_breaker_check);

        let passed = validations.iter().all(|v| v.passed);
        let failed_validations = validations.iter().filter(|v| !v.passed).cloned().collect();

        Ok(RiskValidationResult {
            passed,
            validations,
            failed_validations,
            risk_score: self.calculate_risk_score(&validations),
        })
    }

    /// Update circuit breaker status based on protocol health metrics
    pub async fn update_circuit_breaker_status(
        &mut self,
        protocol_id: &str,
        health_metrics: &ProtocolHealthMetrics,
    ) -> Result<CircuitBreakerStatus, AppError> {
        let circuit_breaker = self.circuit_breakers
            .entry(protocol_id.to_string())
            .or_insert_with(|| CircuitBreaker::new(protocol_id, self.config.circuit_breaker_config.clone()));

        circuit_breaker.evaluate_health_metrics(health_metrics).await
    }

    /// Get current circuit breaker status for a protocol
    pub fn get_circuit_breaker_status(&self, protocol_id: &str) -> Option<&CircuitBreaker> {
        self.circuit_breakers.get(protocol_id)
    }

    /// Reset a tripped circuit breaker (requires governance approval)
    pub async fn reset_circuit_breaker(
        &mut self,
        protocol_id: &str,
        approved_by: &str,
        reason: &str,
    ) -> Result<(), AppError> {
        if let Some(circuit_breaker) = self.circuit_breakers.get_mut(protocol_id) {
            circuit_breaker.reset(approved_by, reason).await
        } else {
            Err(AppError::BadRequest(format!("Circuit breaker not found for protocol: {}", protocol_id)))
        }
    }

    // Individual validation methods

    fn check_single_transaction_limit(&self, amount: &BigDecimal) -> RiskValidation {
        let amount_f64: f64 = amount.to_string().parse().unwrap_or(0.0);
        let passed = amount_f64 <= self.config.max_single_transaction_amount;

        RiskValidation {
            validation_type: ValidationType::SingleTransactionLimit,
            passed,
            message: if passed {
                "Single transaction limit check passed".to_string()
            } else {
                format!("Amount {} exceeds maximum single transaction limit {}", amount, self.config.max_single_transaction_amount)
            },
        }
    }

    fn check_protocol_exposure_limit(
        &self,
        current_exposure: &BigDecimal,
        max_exposure: &BigDecimal,
        additional_amount: &BigDecimal,
    ) -> RiskValidation {
        let new_exposure = current_exposure + additional_amount;
        let passed = new_exposure <= *max_exposure;

        RiskValidation {
            validation_type: ValidationType::ProtocolExposureLimit,
            passed,
            message: if passed {
                "Protocol exposure limit check passed".to_string()
            } else {
                format!("New exposure {} would exceed maximum protocol exposure {}", new_exposure, max_exposure)
            },
        }
    }

    async fn check_protocol_health_gate(&self, protocol: &dyn DeFiProtocol) -> Result<RiskValidation, AppError> {
        let health_metrics = protocol.get_health_metrics().await?;
        let passed = health_metrics.health_score >= self.config.min_health_score;

        Ok(RiskValidation {
            validation_type: ValidationType::ProtocolHealthGate,
            passed,
            message: if passed {
                "Protocol health gate check passed".to_string()
            } else {
                format!("Protocol health score {} below minimum threshold {}", health_metrics.health_score, self.config.min_health_score)
            },
        })
    }

    fn check_circuit_breaker_status(&self, protocol_id: &str) -> RiskValidation {
        if let Some(circuit_breaker) = self.circuit_breakers.get(protocol_id) {
            let passed = !circuit_breaker.is_tripped();

            RiskValidation {
                validation_type: ValidationType::CircuitBreakerStatus,
                passed,
                message: if passed {
                    "Circuit breaker status check passed".to_string()
                } else {
                    format!("Circuit breaker is tripped for protocol: {}", protocol_id)
                },
            }
        } else {
            // No circuit breaker exists, which means it's not tripped
            RiskValidation {
                validation_type: ValidationType::CircuitBreakerStatus,
                passed: true,
                message: "No circuit breaker configured".to_string(),
            }
        }
    }

    fn check_circuit_breaker_withdrawal_status(&self, protocol_id: &str) -> RiskValidation {
        if let Some(circuit_breaker) = self.circuit_breakers.get(protocol_id) {
            // Withdrawals are allowed even if circuit breaker is tripped (emergency withdrawal)
            RiskValidation {
                validation_type: ValidationType::CircuitBreakerStatus,
                passed: true,
                message: "Emergency withdrawal allowed".to_string(),
            }
        } else {
            RiskValidation {
                validation_type: ValidationType::CircuitBreakerStatus,
                passed: true,
                message: "No circuit breaker configured".to_string(),
            }
        }
    }

    fn check_slippage_tolerance(&self, slippage_tolerance: f64) -> RiskValidation {
        let passed = slippage_tolerance <= self.config.max_slippage_tolerance;

        RiskValidation {
            validation_type: ValidationType::SlippageTolerance,
            passed,
            message: if passed {
                "Slippage tolerance check passed".to_string()
            } else {
                format!("Slippage tolerance {} exceeds maximum {}", slippage_tolerance, self.config.max_slippage_tolerance)
            },
        }
    }

    fn check_risk_tier_restrictions(&self, risk_tier: RiskTier) -> RiskValidation {
        let passed = match risk_tier {
            RiskTier::Tier1 | RiskTier::Tier2 => true,
            RiskTier::Tier3 => false, // Tier 3 protocols are blocked
        };

        RiskValidation {
            validation_type: ValidationType::RiskTierRestriction,
            passed,
            message: if passed {
                "Risk tier restriction check passed".to_string()
            } else {
                "Tier 3 protocols are not permitted for platform funds".to_string()
            },
        }
    }

    fn check_minimum_withdrawal_amount(&self, amount: &BigDecimal) -> RiskValidation {
        let amount_f64: f64 = amount.to_string().parse().unwrap_or(0.0);
        let passed = amount_f64 >= self.config.min_withdrawal_amount;

        RiskValidation {
            validation_type: ValidationType::MinimumWithdrawalAmount,
            passed,
            message: if passed {
                "Minimum withdrawal amount check passed".to_string()
            } else {
                format!("Amount {} below minimum withdrawal threshold {}", amount, self.config.min_withdrawal_amount)
            },
        }
    }

    fn calculate_risk_score(&self, validations: &[RiskValidation]) -> f64 {
        let passed_count = validations.iter().filter(|v| v.passed).count();
        let total_count = validations.len();
        
        if total_count == 0 {
            1.0
        } else {
            passed_count as f64 / total_count as f64
        }
    }
}

/// Circuit breaker for individual protocols
#[derive(Debug, Clone)]
pub struct CircuitBreaker {
    protocol_id: String,
    config: CircuitBreakerConfig,
    status: CircuitBreakerState,
    trip_history: Vec<CircuitBreakerTrip>,
}

impl CircuitBreaker {
    pub fn new(protocol_id: &str, config: CircuitBreakerConfig) -> Self {
        Self {
            protocol_id: protocol_id.to_string(),
            config,
            status: CircuitBreakerState::Closed,
            trip_history: Vec::new(),
        }
    }

    pub fn is_tripped(&self) -> bool {
        matches!(self.status, CircuitBreakerState::Tripped { .. })
    }

    pub async fn evaluate_health_metrics(
        &mut self,
        health_metrics: &ProtocolHealthMetrics,
    ) -> Result<CircuitBreakerStatus, AppError> {
        // Check TVL drop condition
        if health_metrics.tvl_change_24h <= -self.config.tvl_drop_threshold {
            return self.trip(
                CircuitBreakerTrigger::TVLDrop,
                &format!("TVL dropped by {:.2}%", health_metrics.tvl_change_24h.abs()),
            ).await;
        }

        // Check health score condition
        if health_metrics.health_score < self.config.min_health_score {
            return self.trip(
                CircuitBreakerTrigger::HealthScoreDrop,
                &format!("Health score dropped to {}", health_metrics.health_score),
            ).await;
        }

        // Check for smart contract pause (placeholder - would check on-chain status)
        // This would involve checking if the protocol contracts are paused

        Ok(CircuitBreakerStatus {
            protocol_id: self.protocol_id.clone(),
            state: self.status.clone(),
            last_trip: self.trip_history.last().cloned(),
            next_evaluation_at: Utc::now() + chrono::Duration::seconds(self.config.evaluation_interval_secs as i64),
        })
    }

    async fn trip(
        &mut self,
        trigger: CircuitBreakerTrigger,
        reason: &str,
    ) -> Result<CircuitBreakerStatus, AppError> {
        let trip_record = CircuitBreakerTrip {
            trip_id: Uuid::new_v4(),
            protocol_id: self.protocol_id.clone(),
            trigger,
            reason: reason.to_string(),
            tripped_at: Utc::now(),
            resolved_at: None,
            resolved_by: None,
            resolution_reason: None,
        };

        self.trip_history.push(trip_record.clone());
        self.status = CircuitBreakerState::Tripped {
            tripped_at: Utc::now(),
            trigger: trigger.clone(),
            reason: reason.to_string(),
        };

        tracing::warn!(
            protocol_id = %self.protocol_id,
            trigger = ?trigger,
            reason = %reason,
            "Circuit breaker tripped"
        );

        Ok(CircuitBreakerStatus {
            protocol_id: self.protocol_id.clone(),
            state: self.status.clone(),
            last_trip: Some(trip_record),
            next_evaluation_at: Utc::now() + chrono::Duration::seconds(self.config.evaluation_interval_secs as i64),
        })
    }

    pub async fn reset(&mut self, approved_by: &str, reason: &str) -> Result<(), AppError> {
        if let Some(last_trip) = self.trip_history.last_mut() {
            last_trip.resolved_at = Some(Utc::now());
            last_trip.resolved_by = Some(approved_by.to_string());
            last_trip.resolution_reason = Some(reason.to_string());
        }

        self.status = CircuitBreakerState::Closed;

        tracing::info!(
            protocol_id = %self.protocol_id,
            approved_by = %approved_by,
            reason = %reason,
            "Circuit breaker reset"
        );

        Ok(())
    }
}

/// Configuration for risk controls
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskControlConfig {
    pub max_single_transaction_amount: f64,
    pub max_slippage_tolerance: f64,
    pub min_health_score: f64,
    pub min_withdrawal_amount: f64,
    pub circuit_breaker_config: CircuitBreakerConfig,
}

/// Circuit breaker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerConfig {
    pub tvl_drop_threshold: f64,
    pub tvl_drop_window_hours: i64,
    pub min_health_score: f64,
    pub evaluation_interval_secs: u64,
    pub emergency_withdrawal_enabled: bool,
}

/// Circuit breaker state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CircuitBreakerState {
    Closed,
    Tripped {
        tripped_at: DateTime<Utc>,
        trigger: CircuitBreakerTrigger,
        reason: String,
    },
}

/// Circuit breaker trigger conditions
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CircuitBreakerTrigger {
    TVLDrop,
    HealthScoreDrop,
    SmartContractPause,
    AbnormalVolumeSpike,
    YieldRateCollapse,
}

/// Circuit breaker status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircuitBreakerStatus {
    pub protocol_id: String,
    pub state: CircuitBreakerState,
    pub last_trip: Option<CircuitBreakerTrip>,
    pub next_evaluation_at: DateTime<Utc>,
}

/// Circuit breaker trip record
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CircuitBreakerTrip {
    pub trip_id: Uuid,
    pub protocol_id: String,
    pub trigger: CircuitBreakerTrigger,
    pub reason: String,
    pub tripped_at: DateTime<Utc>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolved_by: Option<String>,
    pub resolution_reason: Option<String>,
}

/// Risk validation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskValidationResult {
    pub passed: bool,
    pub validations: Vec<RiskValidation>,
    pub failed_validations: Vec<RiskValidation>,
    pub risk_score: f64,
}

/// Individual risk validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RiskValidation {
    pub validation_type: ValidationType,
    pub passed: bool,
    pub message: String,
}

/// Validation types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ValidationType {
    SingleTransactionLimit,
    ProtocolExposureLimit,
    ProtocolHealthGate,
    CircuitBreakerStatus,
    SlippageTolerance,
    RiskTierRestriction,
    MinimumWithdrawalAmount,
}
