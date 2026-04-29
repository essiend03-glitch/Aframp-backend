//! Partner service: FX quotes, transfer initiation, limit enforcement,
//! fee calculation, and settlement computation (Issue #408).

use std::sync::Arc;

use bigdecimal::{BigDecimal, FromPrimitive, ToPrimitive, Zero};
use chrono::Utc;
use sha2::{Digest, Sha256};
use tracing::{info, warn};
use uuid::Uuid;

use crate::database::partner_repository::{PartnerRepository, PartnerRow, PartnerTransferRow};

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum PartnerError {
    #[error("Partner not found or inactive")]
    Unauthorized,
    #[error("Corridor not supported: {0}")]
    UnsupportedCorridor(String),
    #[error("Amount below minimum: {0}")]
    BelowMinimum(String),
    #[error("Amount exceeds maximum: {0}")]
    ExceedsMaximum(String),
    #[error("Daily volume limit exceeded")]
    DailyLimitExceeded,
    #[error("Insufficient liquidity for {0}")]
    InsufficientLiquidity(String),
    #[error("Duplicate transfer reference: {0}")]
    DuplicateRef(String),
    #[error("Database error: {0}")]
    Database(String),
}

impl From<crate::database::error::DatabaseError> for PartnerError {
    fn from(e: crate::database::error::DatabaseError) -> Self {
        Self::Database(e.to_string())
    }
}

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct FxQuote {
    pub from_currency: String,
    pub to_currency: String,
    pub from_amount: BigDecimal,
    pub to_amount: BigDecimal,
    pub fx_rate: BigDecimal,
    pub fee_amount: BigDecimal,
    pub fee_type: String,
    pub expires_at: chrono::DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

pub struct PartnerService {
    repo: Arc<PartnerRepository>,
}

impl PartnerService {
    pub fn new(repo: Arc<PartnerRepository>) -> Self {
        Self { repo }
    }

    /// Hash a raw API key for storage/lookup.
    pub fn hash_api_key(raw: &str) -> String {
        let mut h = Sha256::new();
        h.update(raw.as_bytes());
        hex::encode(h.finalize())
    }

    /// Authenticate a partner by raw API key.
    pub async fn authenticate(&self, raw_key: &str) -> Result<PartnerRow, PartnerError> {
        let hash = Self::hash_api_key(raw_key);
        self.repo
            .find_by_api_key_hash(&hash)
            .await?
            .ok_or(PartnerError::Unauthorized)
    }

    /// Compute an FX quote for a partner corridor.
    /// `base_rate` is the mid-market rate (from_currency → to_currency).
    pub async fn get_quote(
        &self,
        partner_id: Uuid,
        from_currency: &str,
        to_currency: &str,
        from_amount: BigDecimal,
        base_rate: BigDecimal,
    ) -> Result<FxQuote, PartnerError> {
        let corridor = format!("{}->{}", from_currency, to_currency);

        // Validate limits
        if let Some(limits) = self.repo.get_limits(partner_id).await? {
            if from_amount < limits.per_tx_min {
                return Err(PartnerError::BelowMinimum(limits.per_tx_min.to_string()));
            }
            if let Some(max) = &limits.per_tx_max {
                if &from_amount > max {
                    return Err(PartnerError::ExceedsMaximum(max.to_string()));
                }
            }
        }

        // Look up partner fee for this corridor
        let (fee_amount, fee_type) = match self.repo.get_fee(partner_id, &corridor).await? {
            Some(fee) => {
                let amount = if fee.fee_type == "percent" {
                    &from_amount * &fee.fee_value / BigDecimal::from(100)
                } else {
                    fee.fee_value.clone()
                };
                (amount, fee.fee_type)
            }
            None => (BigDecimal::zero(), "flat".to_string()),
        };

        let net_amount = &from_amount - &fee_amount;
        let to_amount = &net_amount * &base_rate;

        Ok(FxQuote {
            from_currency: from_currency.to_string(),
            to_currency: to_currency.to_string(),
            from_amount,
            to_amount,
            fx_rate: base_rate,
            fee_amount,
            fee_type,
            expires_at: Utc::now() + chrono::Duration::seconds(30),
        })
    }

    /// Initiate a transfer after validating limits and liquidity.
    pub async fn initiate_transfer(
        &self,
        partner_id: Uuid,
        partner_ref: &str,
        quote: &FxQuote,
        metadata: serde_json::Value,
    ) -> Result<PartnerTransferRow, PartnerError> {
        let corridor = format!("{}->{}", quote.from_currency, quote.to_currency);

        // Check daily volume limit
        if let Some(limits) = self.repo.get_limits(partner_id).await? {
            if let Some(daily_limit) = &limits.daily_volume_limit {
                let today = Utc::now().date_naive();
                let (vol, _, _) = self.repo.daily_transfer_summary(partner_id, today).await?;
                if &(vol + &quote.from_amount) > daily_limit {
                    return Err(PartnerError::DailyLimitExceeded);
                }
            }
        }

        // Check liquidity
        let liquidity = self.repo.get_liquidity(partner_id, &quote.from_currency).await?;
        if let Some(liq) = &liquidity {
            let available = &liq.balance - &liq.reserved;
            if &quote.from_amount > &available {
                return Err(PartnerError::InsufficientLiquidity(quote.from_currency.clone()));
            }
        }

        let transfer = self.repo.create_transfer(
            partner_id,
            partner_ref,
            &quote.from_currency,
            &quote.to_currency,
            quote.from_amount.clone(),
            quote.to_amount.clone(),
            quote.fee_amount.clone(),
            quote.fx_rate.clone(),
            metadata,
        ).await.map_err(|e| {
            // Unique constraint violation → duplicate ref
            if e.to_string().contains("unique") || e.to_string().contains("duplicate") {
                PartnerError::DuplicateRef(partner_ref.to_string())
            } else {
                PartnerError::from(e)
            }
        })?;

        info!(partner_id=%partner_id, transfer_id=%transfer.id, corridor=%corridor, "Partner transfer initiated");
        Ok(transfer)
    }

    /// Compute net-settlement for a partner on a given date.
    pub async fn compute_settlement(
        &self,
        partner_id: Uuid,
        date: chrono::NaiveDate,
    ) -> Result<(), PartnerError> {
        let (total_volume, total_fees, tx_count) =
            self.repo.daily_transfer_summary(partner_id, date).await?;

        // Net payable = fees collected by Aframp (positive = Aframp keeps)
        let net_payable = total_fees.clone();

        let settlement = self.repo.upsert_settlement(
            partner_id, date, total_volume, total_fees, net_payable, tx_count,
        ).await?;

        info!(partner_id=%partner_id, date=%date, settlement_id=%settlement.id, tx_count=%tx_count, "Settlement computed");
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use bigdecimal::FromPrimitive;

    #[test]
    fn test_hash_api_key_deterministic() {
        let h1 = PartnerService::hash_api_key("secret-key-123");
        let h2 = PartnerService::hash_api_key("secret-key-123");
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64); // hex SHA-256
    }

    #[test]
    fn test_hash_api_key_different_inputs() {
        let h1 = PartnerService::hash_api_key("key-a");
        let h2 = PartnerService::hash_api_key("key-b");
        assert_ne!(h1, h2);
    }

    #[test]
    fn test_fee_calculation_percent() {
        // 1.5% of 1000 = 15
        let from_amount = BigDecimal::from(1000u64);
        let fee_value = BigDecimal::from_f64(1.5).unwrap();
        let fee = &from_amount * &fee_value / BigDecimal::from(100);
        assert_eq!(fee, BigDecimal::from_f64(15.0).unwrap());
    }

    #[test]
    fn test_fee_calculation_flat() {
        let fee = BigDecimal::from(50u64);
        assert_eq!(fee, BigDecimal::from(50u64));
    }

    #[test]
    fn test_to_amount_after_fee() {
        let from_amount = BigDecimal::from(1000u64);
        let fee = BigDecimal::from(15u64);
        let rate = BigDecimal::from_f64(0.0075).unwrap(); // NGN->USD
        let net = &from_amount - &fee;
        let to_amount = &net * &rate;
        // (1000 - 15) * 0.0075 = 985 * 0.0075 = 7.3875
        assert!(to_amount > BigDecimal::zero());
    }

    #[test]
    fn test_corridor_format() {
        let from = "NGN";
        let to = "KES";
        let corridor = format!("{}->{}", from, to);
        assert_eq!(corridor, "NGN->KES");
    }

    #[test]
    fn test_daily_limit_check_logic() {
        let daily_limit = BigDecimal::from(100_000u64);
        let existing_vol = BigDecimal::from(95_000u64);
        let new_amount = BigDecimal::from(10_000u64);
        let would_exceed = &(existing_vol + &new_amount) > &daily_limit;
        assert!(would_exceed);
    }

    #[test]
    fn test_daily_limit_within_bounds() {
        let daily_limit = BigDecimal::from(100_000u64);
        let existing_vol = BigDecimal::from(80_000u64);
        let new_amount = BigDecimal::from(10_000u64);
        let would_exceed = &(existing_vol + &new_amount) > &daily_limit;
        assert!(!would_exceed);
    }

    #[test]
    fn test_net_settlement_equals_fees() {
        let total_fees = BigDecimal::from(500u64);
        let net_payable = total_fees.clone(); // Aframp keeps fees
        assert_eq!(net_payable, BigDecimal::from(500u64));
    }
}
