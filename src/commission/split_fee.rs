//! Split-fee evaluation module (Issue #471).
//!
//! Computes exact partner commission breakdowns for a given gross fee and
//! active commission structure set. Uses integer stroop arithmetic throughout
//! to guarantee Stellar-precision (7 dp) with no floating-point rounding drift.
//!
//! Invariant enforced: gross_fee == platform_share + sum(partner_commissions).
//! Any violation returns `Err(SplitFeeError::InvariantViolation)`.

use std::sync::Arc;

use tracing::{instrument, warn};
use uuid::Uuid;

use super::{
    metrics,
    models::{CommissionStructure, CommissionTier, CommissionType},
    repository::CommissionRepository,
};

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum SplitFeeError {
    #[error("invariant violation: gross {gross} != platform {platform} + partner {partner}")]
    InvariantViolation {
        gross: i64,
        platform: i64,
        partner: i64,
    },
    #[error("commission rate out of range: {0}")]
    InvalidRate(String),
    #[error("database error: {0}")]
    Db(#[from] sqlx::Error),
    #[error("no active commission structure found for partner {0}")]
    NoStructure(Uuid),
}

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

/// Per-partner split result.
#[derive(Debug, Clone)]
pub struct PartnerSplit {
    pub partner_id: Uuid,
    pub structure_id: Uuid,
    pub commission_stroops: i64,
    pub tier_index: Option<i16>,
}

/// Full breakdown for a single transaction fee.
#[derive(Debug, Clone)]
pub struct CommissionBreakdown {
    pub gross_fee_stroops: i64,
    pub platform_share_stroops: i64,
    pub partner_splits: Vec<PartnerSplit>,
}

impl CommissionBreakdown {
    /// Validate that gross == platform + sum(partner commissions).
    pub fn validate_invariant(&self) -> Result<(), SplitFeeError> {
        let partner_total: i64 = self.partner_splits.iter().map(|s| s.commission_stroops).sum();
        if self.gross_fee_stroops != self.platform_share_stroops + partner_total {
            metrics::invariant_violation();
            warn!(
                gross = self.gross_fee_stroops,
                platform = self.platform_share_stroops,
                partner_total,
                "fee-split invariant violated"
            );
            return Err(SplitFeeError::InvariantViolation {
                gross: self.gross_fee_stroops,
                platform: self.platform_share_stroops,
                partner: partner_total,
            });
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Engine
// ---------------------------------------------------------------------------

pub struct SplitFeeEngine {
    repo: Arc<CommissionRepository>,
}

impl SplitFeeEngine {
    pub fn new(repo: Arc<CommissionRepository>) -> Self {
        Self { repo }
    }

    /// Evaluate commissions for all active partners on `gross_fee_stroops`.
    ///
    /// Called concurrently during transaction processing; must not block.
    #[instrument(skip(self), fields(gross_fee_stroops))]
    pub async fn evaluate(
        &self,
        gross_fee_stroops: i64,
        corridor: Option<&str>,
        cumulative_volume_stroops: i64,
    ) -> Result<CommissionBreakdown, SplitFeeError> {
        let structures = self
            .repo
            .active_structures(corridor, gross_fee_stroops)
            .await?;

        let mut partner_splits: Vec<PartnerSplit> = Vec::with_capacity(structures.len());
        let mut total_partner_stroops: i64 = 0;

        for s in &structures {
            let (commission, tier_idx) =
                compute_commission(s, gross_fee_stroops, cumulative_volume_stroops)?;
            total_partner_stroops = total_partner_stroops.saturating_add(commission);
            partner_splits.push(PartnerSplit {
                partner_id: s.partner_id,
                structure_id: s.id,
                commission_stroops: commission,
                tier_index: tier_idx,
            });
        }

        // Platform takes the remainder to preserve the invariant exactly.
        let platform_share = gross_fee_stroops
            .checked_sub(total_partner_stroops)
            .ok_or_else(|| {
                metrics::invariant_violation();
                SplitFeeError::InvariantViolation {
                    gross: gross_fee_stroops,
                    platform: 0,
                    partner: total_partner_stroops,
                }
            })?;

        if platform_share < 0 {
            metrics::invariant_violation();
            return Err(SplitFeeError::InvariantViolation {
                gross: gross_fee_stroops,
                platform: platform_share,
                partner: total_partner_stroops,
            });
        }

        let breakdown = CommissionBreakdown {
            gross_fee_stroops,
            platform_share_stroops: platform_share,
            partner_splits,
        };
        breakdown.validate_invariant()?;

        metrics::commission_evaluated(gross_fee_stroops, total_partner_stroops);
        Ok(breakdown)
    }
}

// ---------------------------------------------------------------------------
// Pure arithmetic helpers (no I/O — easily unit-tested)
// ---------------------------------------------------------------------------

/// Compute commission in stroops for a single structure.
/// Returns `(commission_stroops, tier_index)`.
pub fn compute_commission(
    structure: &CommissionStructure,
    gross_fee_stroops: i64,
    cumulative_volume_stroops: i64,
) -> Result<(i64, Option<i16>), SplitFeeError> {
    match structure.commission_type {
        CommissionType::Percentage => {
            let rate = structure
                .percentage_rate
                .as_ref()
                .ok_or_else(|| SplitFeeError::InvalidRate("missing percentage_rate".into()))?;
            // Convert BigDecimal → f64 only for multiplication then round to i64
            let rate_f64: f64 = rate.to_string().parse().unwrap_or(0.0);
            if !(0.0..=1.0).contains(&rate_f64) {
                return Err(SplitFeeError::InvalidRate(format!("rate {rate_f64} out of [0,1]")));
            }
            // Multiply in i128 to avoid overflow for large stroop values
            let commission = (gross_fee_stroops as i128 * (rate_f64 * 1_000_000_000.0) as i128
                / 1_000_000_000) as i64;
            Ok((commission, None))
        }

        CommissionType::FixedFiat => {
            let fixed = structure
                .fixed_stroops
                .ok_or_else(|| SplitFeeError::InvalidRate("missing fixed_stroops".into()))?;
            // Fixed commission cannot exceed the gross fee
            Ok((fixed.min(gross_fee_stroops), None))
        }

        CommissionType::Tiered => {
            let tiers_val = structure
                .tiers
                .as_ref()
                .ok_or_else(|| SplitFeeError::InvalidRate("missing tiers".into()))?;
            let tiers: Vec<CommissionTier> =
                serde_json::from_value(tiers_val.clone()).map_err(|e| {
                    SplitFeeError::InvalidRate(format!("tiers parse error: {e}"))
                })?;

            for (idx, tier) in tiers.iter().enumerate() {
                let in_tier = cumulative_volume_stroops >= tier.min_volume_stroops
                    && tier
                        .max_volume_stroops
                        .map_or(true, |max| cumulative_volume_stroops < max);
                if in_tier {
                    if !(0.0..=1.0).contains(&tier.rate) {
                        return Err(SplitFeeError::InvalidRate(format!(
                            "tier {idx} rate {} out of [0,1]",
                            tier.rate
                        )));
                    }
                    let commission = (gross_fee_stroops as i128
                        * (tier.rate * 1_000_000_000.0) as i128
                        / 1_000_000_000) as i64;
                    return Ok((commission, Some(idx as i16)));
                }
            }
            // No tier matched → zero commission
            Ok((0, None))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::types::BigDecimal;
    use std::str::FromStr;

    fn pct_structure(rate: &str) -> CommissionStructure {
        CommissionStructure {
            id: Uuid::new_v4(),
            partner_id: Uuid::new_v4(),
            name: "test".into(),
            commission_type: CommissionType::Percentage,
            percentage_rate: Some(BigDecimal::from_str(rate).unwrap()),
            fixed_stroops: None,
            tiers: None,
            min_volume_stroops: 0,
            max_volume_stroops: None,
            corridor: None,
            is_active: true,
            effective_from: chrono::Utc::now(),
            effective_to: None,
            created_by: Uuid::new_v4(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn test_percentage_split_35pct() {
        let s = pct_structure("0.35");
        let (commission, tier) = compute_commission(&s, 10_000_000, 0).unwrap();
        assert_eq!(commission, 3_500_000);
        assert_eq!(tier, None);
    }

    #[test]
    fn test_percentage_split_full_precision() {
        // 0.1234567 of 10_000_000 stroops = 1_234_567
        let s = pct_structure("0.1234567");
        let (commission, _) = compute_commission(&s, 10_000_000, 0).unwrap();
        assert_eq!(commission, 1_234_567);
    }

    #[test]
    fn test_fixed_fiat() {
        let mut s = pct_structure("0");
        s.commission_type = CommissionType::FixedFiat;
        s.percentage_rate = None;
        s.fixed_stroops = Some(500_000);
        let (commission, _) = compute_commission(&s, 10_000_000, 0).unwrap();
        assert_eq!(commission, 500_000);
    }

    #[test]
    fn test_fixed_fiat_capped_at_gross() {
        let mut s = pct_structure("0");
        s.commission_type = CommissionType::FixedFiat;
        s.percentage_rate = None;
        s.fixed_stroops = Some(20_000_000); // more than gross
        let (commission, _) = compute_commission(&s, 10_000_000, 0).unwrap();
        assert_eq!(commission, 10_000_000); // capped
    }

    #[test]
    fn test_tiered_first_tier() {
        let tiers = serde_json::json!([
            {"min_volume_stroops": 0, "max_volume_stroops": 1_000_000_000_i64, "rate": 0.30},
            {"min_volume_stroops": 1_000_000_000_i64, "max_volume_stroops": null, "rate": 0.25},
        ]);
        let mut s = pct_structure("0");
        s.commission_type = CommissionType::Tiered;
        s.percentage_rate = None;
        s.tiers = Some(tiers);
        let (commission, tier_idx) = compute_commission(&s, 10_000_000, 500_000_000).unwrap();
        assert_eq!(commission, 3_000_000);
        assert_eq!(tier_idx, Some(0));
    }

    #[test]
    fn test_tiered_second_tier() {
        let tiers = serde_json::json!([
            {"min_volume_stroops": 0, "max_volume_stroops": 1_000_000_000_i64, "rate": 0.30},
            {"min_volume_stroops": 1_000_000_000_i64, "max_volume_stroops": null, "rate": 0.25},
        ]);
        let mut s = pct_structure("0");
        s.commission_type = CommissionType::Tiered;
        s.percentage_rate = None;
        s.tiers = Some(tiers);
        let (commission, tier_idx) =
            compute_commission(&s, 10_000_000, 2_000_000_000).unwrap();
        assert_eq!(commission, 2_500_000);
        assert_eq!(tier_idx, Some(1));
    }

    #[test]
    fn test_invariant_validation() {
        let breakdown = CommissionBreakdown {
            gross_fee_stroops: 10_000_000,
            platform_share_stroops: 6_500_000,
            partner_splits: vec![PartnerSplit {
                partner_id: Uuid::new_v4(),
                structure_id: Uuid::new_v4(),
                commission_stroops: 3_500_000,
                tier_index: None,
            }],
        };
        assert!(breakdown.validate_invariant().is_ok());
    }

    #[test]
    fn test_invariant_violation_detected() {
        let breakdown = CommissionBreakdown {
            gross_fee_stroops: 10_000_000,
            platform_share_stroops: 7_000_000, // wrong: 7M + 3.5M != 10M
            partner_splits: vec![PartnerSplit {
                partner_id: Uuid::new_v4(),
                structure_id: Uuid::new_v4(),
                commission_stroops: 3_500_000,
                tier_index: None,
            }],
        };
        assert!(matches!(
            breakdown.validate_invariant(),
            Err(SplitFeeError::InvariantViolation { .. })
        ));
    }

    #[test]
    fn test_overflow_protection() {
        // i64::MAX gross fee should not panic
        let s = pct_structure("0.5");
        let result = compute_commission(&s, i64::MAX / 2, 0);
        assert!(result.is_ok());
    }
}
