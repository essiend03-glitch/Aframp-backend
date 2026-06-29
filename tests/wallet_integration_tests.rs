/// Integration tests for wallet registration, auth, backup, recovery, and history flows.
/// These tests require a running database (DATABASE_URL env var).
#[cfg(test)]
#[cfg(feature = "integration")]
mod wallet_integration_tests {
    use sqlx::PgPool;
    use uuid::Uuid;

    async fn test_pool() -> Result<PgPool, sqlx::Error> {
        let url = std::env::var("DATABASE_URL")
            .map_err(|_| sqlx::Error::Configuration("DATABASE_URL required for integration tests".into()))?;
        PgPool::connect(&url).await
    }

    // ── Wallet Registration ──────────────────────────────────────────────────

    #[tokio::test]
    async fn test_wallet_registration_and_lookup() -> Result<(), Box<dyn std::error::Error>> {
        use crate::wallet::repository::WalletRegistryRepository;
        let pool = test_pool().await?;
        let repo = WalletRegistryRepository::new(pool);
        let user_id = Uuid::new_v4();
        let pubkey = format!("G{}", "A".repeat(55));
        let w = repo
            .create(user_id, &pubkey, Some("Test Wallet"), "personal", Some("127.0.0.1"), 0)
            .await?;
        assert_eq!(w.stellar_public_key, pubkey);
        assert_eq!(w.wallet_type, "personal");

        let found = repo.find_by_public_key(&pubkey).await?;
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, w.id);
        Ok(())
    }

    #[tokio::test]
    async fn test_duplicate_pubkey_rejected() -> Result<(), Box<dyn std::error::Error>> {
        use crate::wallet::repository::WalletRegistryRepository;
        let pool = test_pool().await?;
        let repo = WalletRegistryRepository::new(pool);
        let user_id = Uuid::new_v4();
        let pubkey = format!("G{}", "B".repeat(55));
        let _ = repo.create(user_id, &pubkey, None, "personal", None, 0).await;
        let second = repo.create(user_id, &pubkey, None, "personal", None, 0).await;
        assert!(second.is_err());
        Ok(())
    }

    // ── Auth Challenge ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_challenge_single_use() -> Result<(), Box<dyn std::error::Error>> {
        use crate::wallet::repository::WalletRegistryRepository;
        let pool = test_pool().await?;
        let repo = WalletRegistryRepository::new(pool);
        let pubkey = format!("G{}", "C".repeat(55));
        let challenge = repo.create_challenge(&pubkey, "test-challenge-value", 300).await?;

        let consumed = repo.consume_challenge(challenge.id).await?;
        assert!(consumed.is_some());

        // Second consume must be empty (already used)
        let consumed2 = repo.consume_challenge(challenge.id).await?;
        assert!(consumed2.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn test_expired_challenge_rejected() -> Result<(), Box<dyn std::error::Error>> {
        use crate::wallet::repository::WalletRegistryRepository;
        let pool = test_pool().await?;
        let repo = WalletRegistryRepository::new(pool);
        let pubkey = format!("G{}", "D".repeat(55));
        // TTL of -1 means already expired
        let challenge = repo.create_challenge(&pubkey, "expired-challenge", -1).await?;
        let consumed = repo.consume_challenge(challenge.id).await?;
        assert!(consumed.is_none());
        Ok(())
    }

    // ── Backup Confirmation ──────────────────────────────────────────────────

    #[tokio::test]
    async fn test_backup_confirmation_flow() -> Result<(), Box<dyn std::error::Error>> {
        use crate::wallet::repository::WalletRegistryRepository;
        let pool = test_pool().await?;
        let repo = WalletRegistryRepository::new(pool);
        let user_id = Uuid::new_v4();
        let pubkey = format!("G{}", "E".repeat(55));
        let wallet = repo.create(user_id, &pubkey, None, "personal", None, 0).await?;

        let status = repo.get_backup_status(wallet.id).await?;
        assert!(status.is_none());

        repo.confirm_backup(wallet.id).await?;
        let status = repo.get_backup_status(wallet.id).await?;
        assert!(status.is_some());
        Ok(())
    }

    // ── Recovery Rate Limiting ───────────────────────────────────────────────

    #[tokio::test]
    async fn test_recovery_rate_limiting() -> Result<(), Box<dyn std::error::Error>> {
        use crate::wallet::repository::WalletRegistryRepository;
        use chrono::{Duration, Utc};
        let pool = test_pool().await?;
        let repo = WalletRegistryRepository::new(pool);
        let ip = "10.0.0.1";

        let cooloff = Utc::now() + Duration::minutes(5);
        let _ = repo.record_recovery_attempt(ip, None, false, Some(cooloff)).await;

        let active_cooloff = repo.get_cooloff(ip).await?;
        assert!(active_cooloff.is_some());
        assert!(active_cooloff.unwrap() > Utc::now());
        Ok(())
    }

    // ── Guardian Management ──────────────────────────────────────────────────

    #[tokio::test]
    async fn test_guardian_designation() -> Result<(), Box<dyn std::error::Error>> {
        use crate::wallet::repository::WalletRegistryRepository;
        let pool = test_pool().await?;
        let repo = WalletRegistryRepository::new(pool);
        let user_id = Uuid::new_v4();
        let pubkey = format!("G{}", "F".repeat(55));
        let wallet = repo.create(user_id, &pubkey, None, "personal", None, 0).await?;

        let guardians = vec![
            (None, Some("guardian1@example.com".to_string())),
            (None, Some("guardian2@example.com".to_string())),
            (None, Some("guardian3@example.com".to_string())),
        ];
        repo.set_guardians(wallet.id, &guardians).await?;

        let stored = repo.get_guardians(wallet.id).await?;
        assert_eq!(stored.len(), 3);
        Ok(())
    }

    // ── Transaction History ──────────────────────────────────────────────────

    #[tokio::test]
    async fn test_history_insert_and_paginate() -> Result<(), Box<dyn std::error::Error>> {
        use crate::wallet::models::HistoryQuery;
        use crate::wallet::repository::{
            InsertHistoryEntry, TransactionHistoryRepository, WalletRegistryRepository,
        };
        use sqlx::types::BigDecimal;
        use std::str::FromStr;

        let pool = test_pool().await?;
        let wallet_repo = WalletRegistryRepository::new(pool.clone());
        let history_repo = TransactionHistoryRepository::new(pool);

        let user_id = Uuid::new_v4();
        let pubkey = format!("G{}", "H".repeat(55));
        let wallet = wallet_repo.create(user_id, &pubkey, None, "personal", None, 0).await?;

        let entry = InsertHistoryEntry {
            wallet_id: wallet.id,
            entry_type: "payment".to_string(),
            direction: "credit".to_string(),
            asset_code: "cNGN".to_string(),
            asset_issuer: None,
            amount: BigDecimal::from_str("100.00")?,
            fiat_equivalent: Some(BigDecimal::from_str("100.00")?),
            fiat_currency: Some("NGN".to_string()),
            exchange_rate: Some(BigDecimal::from_str("1.0")?),
            counterparty: Some("GSENDER".to_string()),
            platform_transaction_id: None,
            stellar_transaction_hash: Some("abc123".to_string()),
            parent_entry_id: None,
            status: Some("confirmed".to_string()),
            description: Some("Test payment".to_string()),
            failure_reason: None,
            horizon_cursor: Some("cursor1".to_string()),
            confirmed_at: None,
        };
        history_repo.insert(&entry).await?;

        let query = HistoryQuery {
            cursor: None,
            limit: Some(10),
            entry_type: None,
            direction: None,
            asset_code: None,
            status: None,
            date_from: None,
            date_to: None,
            sort: None,
        };
        let (entries, next_cursor) = history_repo.list_paginated(wallet.id, &query).await?;
        assert!(!entries.is_empty());
        assert!(next_cursor.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn test_history_deduplication_by_stellar_hash() -> Result<(), Box<dyn std::error::Error>> {
        use crate::wallet::repository::{
            InsertHistoryEntry, TransactionHistoryRepository, WalletRegistryRepository,
        };
        use sqlx::types::BigDecimal;
        use std::str::FromStr;

        let pool = test_pool().await?;
        let wallet_repo = WalletRegistryRepository::new(pool.clone());
        let history_repo = TransactionHistoryRepository::new(pool);

        let user_id = Uuid::new_v4();
        let pubkey = format!("G{}", "I".repeat(55));
        let wallet = wallet_repo.create(user_id, &pubkey, None, "personal", None, 0).await?;

        let hash = "dedup_test_hash_xyz";
        let entry = InsertHistoryEntry {
            wallet_id: wallet.id,
            entry_type: "payment".to_string(),
            direction: "debit".to_string(),
            asset_code: "XLM".to_string(),
            asset_issuer: None,
            amount: BigDecimal::from_str("5.0")?,
            fiat_equivalent: None,
            fiat_currency: None,
            exchange_rate: None,
            counterparty: None,
            platform_transaction_id: None,
            stellar_transaction_hash: Some(hash.to_string()),
            parent_entry_id: None,
            status: Some("confirmed".to_string()),
            description: None,
            failure_reason: None,
            horizon_cursor: None,
            confirmed_at: None,
        };
        history_repo.insert(&entry).await?;

        let exists = history_repo.exists_by_stellar_hash(wallet.id, hash).await?;
        assert!(exists);
        let not_exists = history_repo.exists_by_stellar_hash(wallet.id, "other_hash").await?;
        assert!(!not_exists);
        Ok(())
    }

    // ── Stellar Sync Cursor ──────────────────────────────────────────────────

    #[tokio::test]
    async fn test_sync_cursor_upsert() -> Result<(), Box<dyn std::error::Error>> {
        use crate::wallet::repository::{TransactionHistoryRepository, WalletRegistryRepository};
        let pool = test_pool().await?;
        let wallet_repo = WalletRegistryRepository::new(pool.clone());
        let history_repo = TransactionHistoryRepository::new(pool);

        let user_id = Uuid::new_v4();
        let pubkey = format!("G{}", "J".repeat(55));
        let wallet = wallet_repo.create(user_id, &pubkey, None, "personal", None, 0).await?;

        let cursor = history_repo.get_sync_cursor(wallet.id).await?;
        assert!(cursor.is_none());

        history_repo.update_sync_cursor(wallet.id, "cursor_abc").await?;
        let cursor = history_repo.get_sync_cursor(wallet.id).await?;
        assert_eq!(cursor.as_deref(), Some("cursor_abc"));

        history_repo.update_sync_cursor(wallet.id, "cursor_xyz").await?;
        let cursor = history_repo.get_sync_cursor(wallet.id).await?;
        assert_eq!(cursor.as_deref(), Some("cursor_xyz"));
        Ok(())
    }

    // ── Portfolio Preferences ────────────────────────────────────────────────

    #[tokio::test]
    async fn test_portfolio_currency_preference() -> Result<(), Box<dyn std::error::Error>> {
        use crate::wallet::repository::PortfolioRepository;
        let pool = test_pool().await?;
        let repo = PortfolioRepository::new(pool);
        let user_id = Uuid::new_v4();

        let currency = repo.get_preferred_currency(user_id).await?;
        assert_eq!(currency, "NGN");

        repo.set_preferred_currency(user_id, "USD").await?;
        let currency = repo.get_preferred_currency(user_id).await?;
        assert_eq!(currency, "USD");
        Ok(())
    }

    // ── Multi-wallet enforcement ─────────────────────────────────────────────

    #[tokio::test]
    async fn test_max_wallet_count() -> Result<(), Box<dyn std::error::Error>> {
        use crate::wallet::repository::WalletRegistryRepository;
        let pool = test_pool().await?;
        let repo = WalletRegistryRepository::new(pool);
        let user_id = Uuid::new_v4();

        for i in 0..3 {
            let pubkey = format!("G{}{}", "K".repeat(54), i);
            let w = repo.create(user_id, &pubkey, None, "personal", None, 0).await?;
            repo.update_status(w.id, "active").await?;
        }

        let count = repo.count_active_wallets(user_id).await?;
        assert_eq!(count, 3);
        Ok(())
    }
}
