//! Commission engine tests (Issue #471).
//!
//! Unit tests are in split_fee.rs (pure arithmetic, no I/O).
//! Integration tests here require a live DB via TEST_DATABASE_URL.

#[cfg(test)]
mod unit {
    use aframp_backend::commission::split_fee::{compute_commission, CommissionBreakdown, PartnerSplit, SplitFeeError};
    use aframp_backend::commission::models::{CommissionStructure, CommissionType};
    use sqlx::types::BigDecimal;
    use std::str::FromStr;
    use uuid::Uuid;

    fn make_structure(ct: CommissionType, rate: Option<&str>, fixed: Option<i64>) -> CommissionStructure {
        CommissionStructure {
            id: Uuid::new_v4(),
            partner_id: Uuid::new_v4(),
            name: "test".into(),
            commission_type: ct,
            percentage_rate: rate.map(|r| BigDecimal::from_str(r).unwrap()),
            fixed_stroops: fixed,
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
    fn percentage_splits_correctly() {
        let s = make_structure(CommissionType::Percentage, Some("0.35"), None);
        let (c, _) = compute_commission(&s, 10_000_000, 0).unwrap();
        assert_eq!(c, 3_500_000);
    }

    #[test]
    fn seven_decimal_precision() {
        let s = make_structure(CommissionType::Percentage, Some("0.1234567"), None);
        let (c, _) = compute_commission(&s, 10_000_000, 0).unwrap();
        assert_eq!(c, 1_234_567);
    }

    #[test]
    fn fixed_fiat_basic() {
        let s = make_structure(CommissionType::FixedFiat, None, Some(500_000));
        let (c, _) = compute_commission(&s, 10_000_000, 0).unwrap();
        assert_eq!(c, 500_000);
    }

    #[test]
    fn fixed_fiat_capped_at_gross() {
        let s = make_structure(CommissionType::FixedFiat, None, Some(20_000_000));
        let (c, _) = compute_commission(&s, 10_000_000, 0).unwrap();
        assert_eq!(c, 10_000_000);
    }

    #[test]
    fn tiered_selects_correct_tier() {
        let tiers = serde_json::json!([
            {"min_volume_stroops": 0, "max_volume_stroops": 1_000_000_000_i64, "rate": 0.30},
            {"min_volume_stroops": 1_000_000_000_i64, "max_volume_stroops": null, "rate": 0.20},
        ]);
        let mut s = make_structure(CommissionType::Tiered, None, None);
        s.tiers = Some(tiers);
        // Volume in tier 0
        let (c0, idx0) = compute_commission(&s, 10_000_000, 500_000_000).unwrap();
        assert_eq!(c0, 3_000_000);
        assert_eq!(idx0, Some(0));
        // Volume in tier 1
        let (c1, idx1) = compute_commission(&s, 10_000_000, 2_000_000_000).unwrap();
        assert_eq!(c1, 2_000_000);
        assert_eq!(idx1, Some(1));
    }

    #[test]
    fn invariant_holds() {
        let bd = CommissionBreakdown {
            gross_fee_stroops: 10_000_000,
            platform_share_stroops: 6_500_000,
            partner_splits: vec![PartnerSplit {
                partner_id: Uuid::new_v4(),
                structure_id: Uuid::new_v4(),
                commission_stroops: 3_500_000,
                tier_index: None,
            }],
        };
        assert!(bd.validate_invariant().is_ok());
    }

    #[test]
    fn invariant_violation_detected() {
        let bd = CommissionBreakdown {
            gross_fee_stroops: 10_000_000,
            platform_share_stroops: 7_000_000,
            partner_splits: vec![PartnerSplit {
                partner_id: Uuid::new_v4(),
                structure_id: Uuid::new_v4(),
                commission_stroops: 3_500_000,
                tier_index: None,
            }],
        };
        assert!(matches!(
            bd.validate_invariant(),
            Err(SplitFeeError::InvariantViolation { .. })
        ));
    }

    #[test]
    fn zero_rate_gives_zero_commission() {
        let s = make_structure(CommissionType::Percentage, Some("0.0"), None);
        let (c, _) = compute_commission(&s, 10_000_000, 0).unwrap();
        assert_eq!(c, 0);
    }

    #[test]
    fn full_rate_gives_full_gross() {
        let s = make_structure(CommissionType::Percentage, Some("1.0"), None);
        let (c, _) = compute_commission(&s, 10_000_000, 0).unwrap();
        assert_eq!(c, 10_000_000);
    }

    #[test]
    fn overflow_does_not_panic() {
        let s = make_structure(CommissionType::Percentage, Some("0.5"), None);
        assert!(compute_commission(&s, i64::MAX / 2, 0).is_ok());
    }
}

#[cfg(all(test, feature = "integration"))]
mod integration {
    use aframp_backend::commission::{
        models::{CommissionType, CreateCommissionStructureInput, LedgerDirection, ManualAdjustmentInput},
        service::CommissionService,
    };
    use sqlx::PgPool;
    use uuid::Uuid;

    async fn pool() -> PgPool {
        let url = std::env::var("TEST_DATABASE_URL")
            .unwrap_or_else(|_| "postgres://localhost/aframp_test".into());
        PgPool::connect(&url).await.expect("test db connection")
    }

    #[tokio::test]
    async fn test_ledger_atomic_write_and_balance() {
        let pool = pool().await;
        let svc = CommissionService::new(pool.clone());

        // Insert a minimal partner for the test
        let partner_id: Uuid = sqlx::query_scalar(
            "INSERT INTO partners (name, finance_email) VALUES ('TestPa', 'test@pa.io') RETURNING id",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        // Create a commission structure
        let structure = svc
            .configure_structure(CreateCommissionStructureInput {
                partner_id,
                name: "35pct".into(),
                commission_type: CommissionType::Percentage,
                percentage_rate: Some(0.35),
                fixed_stroops: None,
                tiers: None,
                min_volume_stroops: None,
                max_volume_stroops: None,
                corridor: None,
                effective_from: None,
                effective_to: None,
                created_by: Uuid::nil(),
            })
            .await
            .unwrap();

        let tx_id = Uuid::new_v4();
        let entries = svc
            .record_transaction_commissions(tx_id, 10_000_000, None, 0)
            .await
            .unwrap();

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].amount_stroops, 3_500_000);
        assert_eq!(entries[0].gross_fee_stroops, 10_000_000);
        assert_eq!(entries[0].platform_share_stroops, 6_500_000);
        assert_eq!(
            entries[0].gross_fee_stroops,
            entries[0].platform_share_stroops + entries[0].amount_stroops,
            "invariant must hold in persisted entry"
        );

        let stmt = svc.revenue_statement(partner_id, 10, 0).await.unwrap();
        assert_eq!(stmt.accrued_stroops, 3_500_000);
        assert_eq!(stmt.unpaid_stroops, 3_500_000);

        // Cleanup
        sqlx::query("DELETE FROM partner_revenue_ledger WHERE partner_id = $1")
            .bind(partner_id)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM partner_commission_balances WHERE partner_id = $1")
            .bind(partner_id)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM commission_structures WHERE partner_id = $1")
            .bind(partner_id)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM partners WHERE id = $1")
            .bind(partner_id)
            .execute(&pool)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_manual_adjustment_narrative_required() {
        let pool = pool().await;
        let svc = CommissionService::new(pool.clone());

        let partner_id: Uuid = sqlx::query_scalar(
            "INSERT INTO partners (name, finance_email) VALUES ('AdjPa', 'adj@pa.io') RETURNING id",
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        let entry = svc
            .manual_adjustment(ManualAdjustmentInput {
                partner_id,
                transaction_id: Uuid::new_v4(),
                amount_stroops: 1_000_000,
                direction: LedgerDirection::Credit,
                gross_fee_stroops: 2_000_000,
                platform_share_stroops: 1_000_000,
                narrative: "Correction for over-deduction on 2026-06-01".into(),
                initiated_by: Uuid::new_v4(),
            })
            .await
            .unwrap();

        assert!(entry.narrative.contains("MANUAL ADJUSTMENT"));
        assert_eq!(entry.amount_stroops, 1_000_000);

        // Cleanup
        sqlx::query("DELETE FROM partner_revenue_ledger WHERE partner_id = $1")
            .bind(partner_id).execute(&pool).await.unwrap();
        sqlx::query("DELETE FROM partner_commission_balances WHERE partner_id = $1")
            .bind(partner_id).execute(&pool).await.unwrap();
        sqlx::query("DELETE FROM partners WHERE id = $1")
            .bind(partner_id).execute(&pool).await.unwrap();
    }
}
