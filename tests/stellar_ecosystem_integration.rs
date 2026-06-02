//! Integration and unit tests for Stellar Ecosystem Partner Integration (Issue #470).

#[cfg(feature = "database")]
mod tests {
    use rust_decimal::prelude::*;

    fn d(s: &str) -> Decimal {
        Decimal::from_str(s).unwrap()
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Unit: slippage enforcement
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_slippage_within_tolerance() {
        use aframp_backend::stellar_ecosystem::dex_pathfinding::enforce_slippage;
        use aframp_backend::stellar_ecosystem::models::PathfindingResult;

        let result = PathfindingResult {
            source_asset: "cNGN".into(),
            source_amount: d("1000"),
            destination_asset: "USDC".into(),
            destination_amount: d("0.625"),
            path: vec!["cNGN".into(), "USDC".into()],
            spread: d("0.003"), // 0.3% — within 0.5% limit
            within_tolerance: true,
        };
        assert!(enforce_slippage(&result, d("0.005"), "cNGN", "USDC").is_ok());
    }

    #[test]
    fn test_slippage_exceeds_threshold() {
        use aframp_backend::stellar_ecosystem::dex_pathfinding::enforce_slippage;
        use aframp_backend::stellar_ecosystem::models::PathfindingResult;

        let result = PathfindingResult {
            source_asset: "cNGN".into(),
            source_amount: d("1000"),
            destination_asset: "USDC".into(),
            destination_amount: d("0.600"),
            path: vec!["cNGN".into(), "USDC".into()],
            spread: d("0.008"), // 0.8% — exceeds 0.5% limit
            within_tolerance: false,
        };
        let err = enforce_slippage(&result, d("0.005"), "cNGN", "USDC").unwrap_err();
        assert!(err.to_string().contains("Slippage"), "got: {}", err);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Unit: Stellar amount precision (7 decimal places)
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_stellar_precision_valid() {
        use aframp_backend::stellar_ecosystem::transaction_builder::validate_stellar_amount;
        assert!(validate_stellar_amount(d("100.1234567"), "amount").is_ok());
        assert!(validate_stellar_amount(d("0.0000001"), "amount").is_ok()); // 1 stroop
    }

    #[test]
    fn test_stellar_precision_too_many_decimals() {
        use aframp_backend::stellar_ecosystem::transaction_builder::validate_stellar_amount;
        let too_precise = Decimal::from_str("0.00000001").unwrap(); // 8 dp
        assert!(validate_stellar_amount(too_precise, "amount").is_err());
    }

    #[test]
    fn test_stellar_precision_zero_rejected() {
        use aframp_backend::stellar_ecosystem::transaction_builder::validate_stellar_amount;
        assert!(validate_stellar_amount(Decimal::ZERO, "amount").is_err());
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Unit: slippage buffer
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_apply_slippage_buffer() {
        use aframp_backend::stellar_ecosystem::transaction_builder::apply_slippage_buffer;
        let buffered = apply_slippage_buffer(d("1000.0000000"), d("0.005"));
        assert_eq!(buffered, d("1005.0000000"));
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Unit: transaction builder
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_transaction_builder_path_payment() {
        use aframp_backend::stellar_ecosystem::transaction_builder::StellarTransactionBuilder;

        let tx = StellarTransactionBuilder::new(
            "GABC123",
            100,
            "Test SDF Network ; September 2015",
        )
        .add_path_payment_strict_receive(
            "cNGN:GISSUER",
            d("1005.0000000"),
            "GDEST456",
            "USDC:GCIRCLE",
            d("0.6250000"),
            vec!["cNGN:GISSUER".into(), "USDC:GCIRCLE".into()],
        )
        .unwrap()
        .build()
        .unwrap();

        assert!(!tx.xdr_base64.is_empty());
        assert_eq!(tx.operations.len(), 1);
        assert!(tx.operations[0].contains("PathPaymentStrictReceive"));
    }

    #[test]
    fn test_transaction_builder_empty_fails() {
        use aframp_backend::stellar_ecosystem::transaction_builder::StellarTransactionBuilder;
        let result = StellarTransactionBuilder::new("GABC", 1, "Test SDF Network ; September 2015")
            .build();
        assert!(result.is_err());
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Unit: parse_stellar_amount
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_parse_stellar_amount() {
        use aframp_backend::stellar_ecosystem::dex_pathfinding::parse_stellar_amount;
        assert_eq!(parse_stellar_amount("100.0000000").unwrap(), d("100.0000000"));
        assert_eq!(parse_stellar_amount("0.0000001").unwrap(), d("0.0000001"));
        assert!(parse_stellar_amount("not_a_number").is_err());
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Integration: DB persistence (requires "integration" feature + live DB)
    // ─────────────────────────────────────────────────────────────────────────

    #[cfg(feature = "integration")]
    #[tokio::test]
    async fn test_anchor_connection_lifecycle() {
        use aframp_backend::stellar_ecosystem::{models::CreateAnchorConnectionRequest, repository};

        let pool = sqlx::PgPool::connect(&std::env::var("DATABASE_URL").unwrap())
            .await
            .unwrap();

        let req = CreateAnchorConnectionRequest {
            domain: "test-anchor.example.com".into(),
            display_name: "Test Anchor".into(),
            supported_assets: vec!["USDC".into()],
            sep24_enabled: true,
            sep31_enabled: true,
            signing_key: None,
            horizon_url: None,
        };

        let anchor = repository::insert_anchor_connection(&pool, &req).await.unwrap();
        assert_eq!(anchor.domain, "test-anchor.example.com");
        assert_eq!(anchor.status, "pending_verification");

        let fetched = repository::get_anchor_by_domain(&pool, "test-anchor.example.com")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(fetched.id, anchor.id);

        sqlx::query!("DELETE FROM stellar_anchor_connections WHERE id = $1", anchor.id)
            .execute(&pool)
            .await
            .unwrap();
    }

    #[cfg(feature = "integration")]
    #[tokio::test]
    async fn test_order_book_snapshot_cache() {
        use aframp_backend::stellar_ecosystem::repository;

        let pool = sqlx::PgPool::connect(&std::env::var("DATABASE_URL").unwrap())
            .await
            .unwrap();

        let snap = repository::upsert_order_book_snapshot(
            &pool,
            "cNGN:GISSUER",
            "USDC:GCIRCLE",
            Some(d("0.0006250")),
            Some(d("0.0006300")),
            Some(d("0.0006275")),
            Some(d("0.0080000")),
            serde_json::json!([{"price": "0.000625", "amount": "50000"}]),
            serde_json::json!([{"price": "0.000630", "amount": "45000"}]),
            d("50000"),
            d("31.25"),
        )
        .await
        .unwrap();

        assert!(snap.best_bid.is_some());

        let fetched = repository::get_latest_snapshot(&pool, "cNGN:GISSUER", "USDC:GCIRCLE")
            .await
            .unwrap();
        assert!(fetched.is_some());

        sqlx::query!("DELETE FROM dex_order_book_snapshots WHERE id = $1", snap.id)
            .execute(&pool)
            .await
            .unwrap();
    }

    #[cfg(feature = "integration")]
    #[tokio::test]
    async fn test_cross_anchor_transfer_status_lifecycle() {
        use aframp_backend::stellar_ecosystem::{models::InitiateTransferRequest, repository};

        let pool = sqlx::PgPool::connect(&std::env::var("DATABASE_URL").unwrap())
            .await
            .unwrap();

        // Requires an existing anchor in the DB
        let anchors = repository::list_anchor_connections(&pool).await.unwrap();
        if anchors.is_empty() {
            return; // skip if no anchors seeded
        }
        let anchor = &anchors[0];

        let req = InitiateTransferRequest {
            receiving_anchor_domain: anchor.domain.clone(),
            send_asset: "cNGN:GISSUER".into(),
            receive_asset: "USDC:GCIRCLE".into(),
            send_amount: d("1000.0000000"),
            sender_account: "GSENDER123".into(),
            receiver_account: None,
            max_slippage: Some(d("0.005")),
        };

        let transfer = repository::insert_cross_anchor_transfer(&pool, &req, anchor.id)
            .await
            .unwrap();
        assert_eq!(transfer.status, "initiated");

        repository::update_transfer_status(&pool, transfer.id, "completed", None, None)
            .await
            .unwrap();

        let updated = repository::get_transfer_by_id(&pool, transfer.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.status, "completed");
        assert!(updated.completed_at.is_some());

        sqlx::query!("DELETE FROM cross_anchor_transfers WHERE id = $1", transfer.id)
            .execute(&pool)
            .await
            .unwrap();
    }
}
