//! PEP Transaction Monitoring Service
//! Applies enhanced monitoring to PEP accounts and their family/associates

use crate::pep::extended_models::{PepTransactionMonitoring, MonitoringFlag, ReviewStatus};
use crate::pep::models::PepRiskTier;
use chrono::Utc;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

/// Transaction monitoring configuration
#[derive(Debug, Clone)]
pub struct TransactionMonitoringConfig {
    /// PEP-specific transaction threshold (in base currency units)
    pub pep_transaction_threshold: Decimal,
    /// Apply stricter AML rules multiplier
    pub aml_threshold_multiplier: f64,
    /// High-risk jurisdiction codes
    pub high_risk_jurisdictions: Vec<String>,
    /// Rapid fund movement threshold (number of transactions)
    pub rapid_movement_threshold: i32,
    /// Rapid fund movement time window (hours)
    pub rapid_movement_window_hours: i32,
    /// Cash-equivalent transaction types
    pub cash_equivalent_types: Vec<String>,
}

impl Default for TransactionMonitoringConfig {
    fn default() -> Self {
        Self {
            pep_transaction_threshold: Decimal::from(10000), // $10,000
            aml_threshold_multiplier: 0.5, // 50% of normal threshold
            high_risk_jurisdictions: vec![
                "KP".to_string(), // North Korea
                "IR".to_string(), // Iran
                "SY".to_string(), // Syria
                "CU".to_string(), // Cuba
                "VE".to_string(), // Venezuela
            ],
            rapid_movement_threshold: 5,
            rapid_movement_window_hours: 24,
            cash_equivalent_types: vec![
                "cash_deposit".to_string(),
                "cash_withdrawal".to_string(),
                "money_order".to_string(),
                "travelers_cheque".to_string(),
            ],
        }
    }
}

/// Transaction details for monitoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionDetails {
    pub transaction_id: Uuid,
    pub consumer_id: Uuid,
    pub amount: Decimal,
    pub currency: String,
    pub transaction_type: String,
    pub counterparty_country: Option<String>,
    pub counterparty_name: Option<String>,
    pub is_international: bool,
    pub created_at: chrono::DateTime<Utc>,
}

/// PEP Transaction Monitor Service
pub struct PepTransactionMonitor {
    config: TransactionMonitoringConfig,
}

impl PepTransactionMonitor {
    pub fn new(config: TransactionMonitoringConfig) -> Self {
        Self { config }
    }

    /// Evaluate a transaction against PEP monitoring rules
    pub fn evaluate_transaction(
        &self,
        tx: &TransactionDetails,
        pep_risk_tier: &PepRiskTier,
        is_pep_related: bool,
    ) -> Option<PepTransactionMonitoring> {
        let mut flags = Vec::new();

        // Check threshold breach (PEP-specific threshold)
        if tx.amount >= self.config.pep_transaction_threshold {
            flags.push(MonitoringFlag::ThresholdBreach);
            info!(
                transaction_id = %tx.transaction_id,
                amount = %tx.amount,
                "PEP transaction threshold breached"
            );
        }

        // Check high-risk jurisdiction
        if let Some(ref country) = tx.counterparty_country {
            if self.config.high_risk_jurisdictions.contains(country) {
                flags.push(MonitoringFlag::HighRiskJurisdiction);
                warn!(
                    transaction_id = %tx.transaction_id,
                    country = %country,
                    "Transaction to high-risk jurisdiction"
                );
            }
        }

        // Check cash-equivalent transactions
        if self.config.cash_equivalent_types.contains(&tx.transaction_type) {
            flags.push(MonitoringFlag::CashEquivalent);
            info!(
                transaction_id = %tx.transaction_id,
                tx_type = %tx.transaction_type,
                "Cash-equivalent transaction detected"
            );
        }

        // Note: Rapid fund movement would be evaluated by a separate
        // pattern analysis service that maintains transaction history

        // If any flags were raised, create monitoring record
        if !flags.is_empty() {
            // For simplicity, use the first flag - in production could be multiple
            let primary_flag = flags.remove(0);

            return Some(PepTransactionMonitoring::new(
                Uuid::nil(), // Would be linked to PEP profile in production
                Some(tx.transaction_id),
                primary_flag,
            ));
        }

        None
    }

    /// Calculate the effective AML threshold for a PEP account
    pub fn pep_aml_threshold(&self, base_threshold: Decimal) -> Decimal {
        // Apply PEP-specific multiplier to get stricter threshold
        let multiplier = self.config.aml_threshold_multiplier;
        (base_threshold * Decimal::from_f64(multiplier).unwrap_or(base_threshold))
            .round_dp(2)
    }

    /// Check if a PEP account should be blocked based on risk tier
    pub fn should_block_account(&self, risk_tier: &PepRiskTier, edd_completed: bool) -> bool {
        match risk_tier {
            PepRiskTier::Critical | PepRiskTier::High => !edd_completed,
            PepRiskTier::Medium => false, // Can proceed with monitoring
            PepRiskTier::Low => false,
        }
    }

    /// Apply enhanced monitoring to family members and associates
    pub fn is_enhanced_monitoring_required(&self, relationship: &str) -> bool {
        matches!(
            relationship,
            "spouse" | "child" | "parent" | "sibling" | "business_partner" | "known_associate"
        )
    }

    /// Evaluate if transaction pattern is unusual for PEP
    pub fn detect_unusual_pattern(
        &self,
        recent_transactions: &[TransactionDetails],
        current_transaction: &TransactionDetails,
    ) -> Option<MonitoringFlag> {
        if recent_transactions.is_empty() {
            return None;
        }

        // Calculate average transaction amount in recent window
        let total: Decimal = recent_transactions
            .iter()
            .map(|t| t.amount)
            .sum();
        let avg_amount = total / Decimal::from(recent_transactions.len() as i32);

        // If current transaction is significantly larger than average
        let ratio = current_transaction.amount / avg_amount;
        if ratio > Decimal::from(5) {
            return Some(MonitoringFlag::UnusualPattern);
        }

        // Check for rapid fund movement
        if recent_transactions.len() >= self.config.rapid_movement_threshold as usize {
            let window_start = Utc::now() - chrono::Duration::hours(self.config.rapid_movement_window_hours as i64);
            let recent_count = recent_transactions
                .iter()
                .filter(|t| t.created_at > window_start)
                .count();

            if recent_count >= self.config.rapid_movement_threshold as usize {
                return Some(MonitoringFlag::RapidFundMovement);
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aml_threshold_calculation() {
        let monitor = PepTransactionMonitor::new(TransactionMonitoringConfig::default());
        let base = Decimal::from(10000);
        let pep_threshold = monitor.pep_aml_threshold(base);

        // Should be 50% of base threshold
        assert!(pep_threshold < base);
    }

    #[test]
    fn test_account_blocking() {
        let monitor = PepTransactionMonitor::new(TransactionMonitoringConfig::default());

        // Critical tier without EDD should be blocked
        assert!(monitor.should_block_account(&PepRiskTier::Critical, false));

        // Critical tier with EDD should not be blocked
        assert!(!monitor.should_block_account(&PepRiskTier::Critical, true));

        // High tier without EDD should be blocked
        assert!(monitor.should_block_account(&PepRiskTier::High, false));

        // Medium tier should not be blocked regardless of EDD
        assert!(!monitor.should_block_account(&PepRiskTier::Medium, false));

        // Low tier should not be blocked
        assert!(!monitor.should_block_account(&PepRiskTier::Low, false));
    }

    #[test]
    fn test_enhanced_monitoring_required() {
        let monitor = PepTransactionMonitor::new(TransactionMonitoringConfig::default());

        assert!(monitor.is_enhanced_monitoring_required("spouse"));
        assert!(monitor.is_enhanced_monitoring_required("child"));
        assert!(monitor.is_enhanced_monitoring_required("business_partner"));
        assert!(!monitor.is_enhanced_monitoring_required("other"));
    }
}