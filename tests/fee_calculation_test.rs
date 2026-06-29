use sqlx::PgPool;
use std::str::FromStr;
use Bitmesh_backend::services::fee_calculation::{FeeBreakdown, FeeCalculationService};

type BigDecimal = sqlx::types::BigDecimal;

async fn setup_test_db() -> PgPool {
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://postgres:postgres@localhost/aframp_test".to_string());

    PgPool::connect(&database_url)
        .await
        .expect("Failed to connect to test database")
}

async fn seed_fee_structures(pool: &PgPool) {
    // Clear existing test data
    // Test setup: failure indicates test environment issue
    sqlx::query("DELETE FROM fee_structures WHERE transaction_type LIKE 'test_%'")
        .execute(pool)
        .await
        .expect("Failed to clear existing test fee structures");

    // Tier 1: Small amounts (₦1,000 - ₦50,000)
    // Test setup: failure indicates test environment issue
    sqlx::query(
        r#"
        INSERT INTO fee_structures 
        (transaction_type, payment_provider, payment_method, min_amount, max_amount,
         provider_fee_percent, provider_fee_flat, provider_fee_cap, platform_fee_percent, is_active)
        VALUES ('onramp', 'flutterwave', 'card', 1000, 50000, 1.4, 100, 2000, 0.5, true)
        "#,
    )
    .execute(pool)
    .await
    .expect("Failed to insert tier 1 test fee structure");

    // Tier 2: Medium amounts (₦50,001 - ₦500,000)
    // Test setup: failure indicates test environment issue
    sqlx::query(
        r#"
        INSERT INTO fee_structures 
        (transaction_type, payment_provider, payment_method, min_amount, max_amount,
         provider_fee_percent, provider_fee_flat, provider_fee_cap, platform_fee_percent, is_active)
        VALUES ('onramp', 'flutterwave', 'card', 50001, 500000, 1.4, 0, 2000, 0.3, true)
        "#,
    )
    .execute(pool)
    .await
    .expect("Failed to insert tier 2 test fee structure");

    // Tier 3: Large amounts (₦500,001+)
    // Test setup: failure indicates test environment issue
    sqlx::query(
        r#"
        INSERT INTO fee_structures 
        (transaction_type, payment_provider, payment_method, min_amount, max_amount,
         provider_fee_percent, provider_fee_flat, provider_fee_cap, platform_fee_percent, is_active)
        VALUES ('onramp', 'flutterwave', 'card', 500001, NULL, 1.4, 0, 2000, 0.2, true)
        "#,
    )
    .execute(pool)
    .await
    .expect("Failed to insert tier 3 test fee structure");

    // Paystack fees
    // Test setup: failure indicates test environment issue
    sqlx::query(
        r#"
        INSERT INTO fee_structures 
        (transaction_type, payment_provider, payment_method, min_amount, max_amount,
         provider_fee_percent, provider_fee_flat, provider_fee_cap, platform_fee_percent, is_active)
        VALUES ('onramp', 'paystack', 'card', 1000, 50000, 1.5, 0, 2000, 0.5, true)
        "#,
    )
    .execute(pool)
    .await
    .expect("Failed to insert paystack test fee structure");

    // Offramp fees
    // Test setup: failure indicates test environment issue
    sqlx::query(
        r#"
        INSERT INTO fee_structures 
        (transaction_type, payment_provider, payment_method, min_amount, max_amount,
         provider_fee_percent, provider_fee_flat, provider_fee_cap, platform_fee_percent, is_active)
        VALUES ('offramp', 'flutterwave', 'bank_transfer', 1000, NULL, 0.8, 50, 5000, 0.5, true)
        "#,
    )
    .execute(pool)
    .await
    .expect("Failed to insert offramp test fee structure");
}

#[tokio::test]
async fn test_tier1_small_amount_fees() {
    let pool = setup_test_db().await;
    seed_fee_structures(&pool).await;

    let service = FeeCalculationService::new(pool);
    // Test fixture: valid decimal string
    let amount = BigDecimal::from_str("10000").expect("Failed to parse test amount");

    let breakdown = service
        .calculate_fees("onramp", amount.clone(), Some("flutterwave"), Some("card"))
        .await
        .expect("Failed to calculate fees");

    // Provider fee: 10,000 × 1.4% + 100 = 140 + 100 = 240
    // Platform fee: 10,000 × 0.5% = 50
    // Total: 290
    assert_eq!(breakdown.amount, amount);
    assert!(breakdown.provider.is_some());

    // Test assertion: provider fee should be present
    let provider_fee = breakdown.provider.expect("Provider fee should be present");
    assert_eq!(provider_fee.calculated, BigDecimal::from_str("240").expect("Failed to parse expected value"));
    assert_eq!(breakdown.platform.calculated, BigDecimal::from_str("50").expect("Failed to parse expected value"));
    assert_eq!(breakdown.total, BigDecimal::from_str("290").expect("Failed to parse expected value"));
    assert_eq!(breakdown.net_amount, BigDecimal::from_str("9710").expect("Failed to parse expected value"));
}

#[tokio::test]
async fn test_tier2_medium_amount_fees() {
    let pool = setup_test_db().await;
    seed_fee_structures(&pool).await;

    let service = FeeCalculationService::new(pool);
    // Test fixture: valid decimal string
    let amount = BigDecimal::from_str("100000").expect("Failed to parse test amount");

    let breakdown = service
        .calculate_fees("onramp", amount.clone(), Some("flutterwave"), Some("card"))
        .await
        .expect("Failed to calculate fees");

    // Provider fee: 100,000 × 1.4% = 1,400 (no flat fee in tier 2)
    // Platform fee: 100,000 × 0.3% = 300
    // Total: 1,700
    // Test assertion: provider fee should be present
    let provider_fee = breakdown.provider.expect("Provider fee should be present");
    assert_eq!(provider_fee.calculated, BigDecimal::from_str("1400").expect("Failed to parse expected value"));
    assert_eq!(breakdown.platform.calculated, BigDecimal::from_str("300").expect("Failed to parse expected value"));
    assert_eq!(breakdown.total, BigDecimal::from_str("1700").expect("Failed to parse expected value"));
}

#[tokio::test]
async fn test_tier3_large_amount_with_cap() {
    let pool = setup_test_db().await;
    seed_fee_structures(&pool).await;

    let service = FeeCalculationService::new(pool);
    // Test fixture: valid decimal string
    let amount = BigDecimal::from_str("1000000").expect("Failed to parse test amount");

    let breakdown = service
        .calculate_fees("onramp", amount.clone(), Some("flutterwave"), Some("card"))
        .await
        .expect("Failed to calculate fees");

    // Provider fee: 1,000,000 × 1.4% = 14,000 BUT capped at 2,000
    // Platform fee: 1,000,000 × 0.2% = 2,000
    // Total: 4,000
    // Test assertion: provider fee should be present
    let provider_fee = breakdown.provider.expect("Provider fee should be present");
    assert_eq!(provider_fee.calculated, BigDecimal::from_str("2000").expect("Failed to parse expected value"));
    assert_eq!(breakdown.platform.calculated, BigDecimal::from_str("2000").expect("Failed to parse expected value"));
    assert_eq!(breakdown.total, BigDecimal::from_str("4000").expect("Failed to parse expected value"));

    // Effective rate should be 0.4%
    // Test fixture: valid decimal string
    let expected_rate = BigDecimal::from_str("0.4").expect("Failed to parse expected value");
    assert!(breakdown.effective_rate >= expected_rate && breakdown.effective_rate <= BigDecimal::from_str("0.41").expect("Failed to parse expected value"));
}

#[tokio::test]
async fn test_boundary_amount_tier_selection() {
    let pool = setup_test_db().await;
    seed_fee_structures(&pool).await;

    let service = FeeCalculationService::new(pool);

    // Test at tier boundary: 50,000 (should use tier 1)
    let breakdown1 = service
        .calculate_fees(
            "onramp",
            // Test fixture: valid decimal string
            BigDecimal::from_str("50000").expect("Failed to parse test amount"),
            Some("flutterwave"),
            Some("card"),
        )
        .await
        .expect("Failed to calculate fees");

    // Should have flat fee (tier 1)
    // Test assertion: provider fee should be present
    let provider_fee1 = breakdown1.provider.expect("Provider fee should be present");
    assert_eq!(provider_fee1.flat, BigDecimal::from_str("100").expect("Failed to parse expected value"));

    // Test at tier boundary: 50,001 (should use tier 2)
    let breakdown2 = service
        .calculate_fees(
            "onramp",
            // Test fixture: valid decimal string
            BigDecimal::from_str("50001").expect("Failed to parse test amount"),
            Some("flutterwave"),
            Some("card"),
        )
        .await
        .expect("Failed to calculate fees");

    // Should have no flat fee (tier 2)
    // Test assertion: provider fee should be present
    let provider_fee2 = breakdown2.provider.expect("Provider fee should be present");
    assert_eq!(provider_fee2.flat, BigDecimal::from_str("0").expect("Failed to parse expected value"));
}

#[tokio::test]
async fn test_paystack_vs_flutterwave_fees() {
    let pool = setup_test_db().await;
    seed_fee_structures(&pool).await;

    let service = FeeCalculationService::new(pool);
    // Test fixture: valid decimal string
    let amount = BigDecimal::from_str("10000").expect("Failed to parse test amount");

    let flutterwave = service
        .calculate_fees("onramp", amount.clone(), Some("flutterwave"), Some("card"))
        .await
        .expect("Failed to calculate fees");

    let paystack = service
        .calculate_fees("onramp", amount.clone(), Some("paystack"), Some("card"))
        .await
        .expect("Failed to calculate fees");

    // Flutterwave: 1.4% + 100 = 240
    // Paystack: 1.5% = 150
    // Test assertion: provider fee should be present
    assert_eq!(flutterwave.provider.as_ref().expect("Provider fee should be present").calculated, BigDecimal::from_str("240").expect("Failed to parse expected value"));
    // Test assertion: provider fee should be present
    assert_eq!(paystack.provider.as_ref().expect("Provider fee should be present").calculated, BigDecimal::from_str("150").expect("Failed to parse expected value"));
}

#[tokio::test]
async fn test_offramp_fees() {
    let pool = setup_test_db().await;
    seed_fee_structures(&pool).await;

    let service = FeeCalculationService::new(pool);
    // Test fixture: valid decimal string
    let amount = BigDecimal::from_str("100000").expect("Failed to parse test amount");

    let breakdown = service
        .calculate_fees(
            "offramp",
            amount.clone(),
            Some("flutterwave"),
            Some("bank_transfer"),
        )
        .await
        .expect("Failed to calculate fees");

    // Provider fee: 100,000 × 0.8% = 800 (min 50, max 5000)
    // Platform fee: 100,000 × 0.5% = 500
    // Total: 1,300
    // Test assertion: provider fee should be present
    let provider_fee = breakdown.provider.expect("Provider fee should be present");
    assert_eq!(provider_fee.calculated, BigDecimal::from_str("800").expect("Failed to parse expected value"));
    assert_eq!(breakdown.platform.calculated, BigDecimal::from_str("500").expect("Failed to parse expected value"));
    assert_eq!(breakdown.total, BigDecimal::from_str("1300").expect("Failed to parse expected value"));
}

#[tokio::test]
async fn test_fee_estimation() {
    let pool = setup_test_db().await;
    seed_fee_structures(&pool).await;

    let service = FeeCalculationService::new(pool);
    // Test fixture: valid decimal string
    let amount = BigDecimal::from_str("10000").expect("Failed to parse test amount");

    let (min_fee, max_fee) = service
        .estimate_fees("onramp", amount)
        .await
        .expect("Failed to estimate fees");

    // Should return range based on different providers
    assert!(min_fee > BigDecimal::from_str("0").expect("Failed to parse expected value"));
    assert!(max_fee >= min_fee);
}

#[tokio::test]
async fn test_stellar_fee_absorbed() {
    let pool = setup_test_db().await;
    seed_fee_structures(&pool).await;

    let service = FeeCalculationService::new(pool);
    // Test fixture: valid decimal string
    let amount = BigDecimal::from_str("10000").expect("Failed to parse test amount");

    let breakdown = service
        .calculate_fees("onramp", amount, Some("flutterwave"), Some("card"))
        .await
        .expect("Failed to calculate fees");

    // Stellar fee should be absorbed (NGN = 0)
    assert_eq!(breakdown.stellar.ngn, BigDecimal::from_str("0").expect("Failed to parse expected value"));
    assert!(breakdown.stellar.absorbed);
    assert_eq!(breakdown.stellar.xlm, BigDecimal::from_str("0.00001").expect("Failed to parse expected value"));
}

#[tokio::test]
async fn test_cache_invalidation() {
    let pool = setup_test_db().await;
    seed_fee_structures(&pool).await;

    let service = FeeCalculationService::new(pool);
    // Test fixture: valid decimal string
    let amount = BigDecimal::from_str("10000").expect("Failed to parse test amount");

    // First call - loads from DB
    let _ = service
        .calculate_fees("onramp", amount.clone(), Some("flutterwave"), Some("card"))
        .await
        .expect("Failed to calculate fees");

    // Invalidate cache
    service.invalidate_cache().await;

    // Second call - should reload from DB
    let breakdown = service
        .calculate_fees("onramp", amount, Some("flutterwave"), Some("card"))
        .await
        .expect("Failed to calculate fees");

    assert!(breakdown.total > BigDecimal::from_str("0").expect("Failed to parse expected value"));
}

#[tokio::test]
async fn test_effective_rate_calculation() {
    let pool = setup_test_db().await;
    seed_fee_structures(&pool).await;

    let service = FeeCalculationService::new(pool);

    // Test tier 1: ~2.9% effective rate
    let breakdown1 = service
        .calculate_fees(
            "onramp",
            // Test fixture: valid decimal string
            BigDecimal::from_str("10000").expect("Failed to parse test amount"),
            Some("flutterwave"),
            Some("card"),
        )
        .await
        .expect("Failed to calculate fees");

    assert!(breakdown1.effective_rate >= BigDecimal::from_str("2.8").expect("Failed to parse expected value"));
    assert!(breakdown1.effective_rate <= BigDecimal::from_str("3.0").expect("Failed to parse expected value"));

    // Test tier 3: ~0.4% effective rate
    let breakdown3 = service
        .calculate_fees(
            "onramp",
            // Test fixture: valid decimal string
            BigDecimal::from_str("1000000").expect("Failed to parse test amount"),
            Some("flutterwave"),
            Some("card"),
        )
        .await
        .expect("Failed to calculate fees");

    assert!(breakdown3.effective_rate >= BigDecimal::from_str("0.3").expect("Failed to parse expected value"));
    assert!(breakdown3.effective_rate <= BigDecimal::from_str("0.5").expect("Failed to parse expected value"));
}
