//! Banking Integration Tests (Issue #407)
//!
//! Tests cover: account linkage, mandate lifecycle, idempotent transfers,
//! reconciliation engine, and webhook processing.
//!
//! Requires: DATABASE_URL env var pointing to a test PostgreSQL instance.

#[cfg(test)]
#[cfg(feature = "integration")]
mod banking_integration {
    use sqlx::PgPool;
    use std::sync::Arc;
    use uuid::Uuid;

    async fn test_pool() -> PgPool {
        let url =
            std::env::var("DATABASE_URL").expect("DATABASE_URL required for integration tests");
        PgPool::connect(&url).await.expect("DB connect failed")
    }

    // ── Repository Unit Tests (no external calls) ─────────────────────────────

    #[tokio::test]
    async fn test_link_and_list_accounts() {
        use Bitmesh_backend::banking::repository::BankingRepository;

        let pool = test_pool().await;
        let repo = BankingRepository::new(pool);
        let user_id = Uuid::new_v4();

        let account = repo
            .insert_linked_account(
                user_id,
                &format!("token-{}", Uuid::new_v4()),
                "****1234",
                "JOHN DOE",
                "058",
                "GTBank",
                "NGN",
                Some("deadbeef"),
                "flutterwave",
            )
            .await
            .expect("insert linked account");

        assert_eq!(account.user_id, user_id);
        assert_eq!(account.account_mask, "****1234");
        assert_eq!(account.status, "active");

        let accounts = repo
            .list_linked_accounts_for_user(user_id)
            .await
            .expect("list accounts");
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].id, account.id);
    }

    #[tokio::test]
    async fn test_unlink_account() {
        use Bitmesh_backend::banking::repository::BankingRepository;

        let pool = test_pool().await;
        let repo = BankingRepository::new(pool);
        let user_id = Uuid::new_v4();

        let account = repo
            .insert_linked_account(
                user_id,
                &format!("token-{}", Uuid::new_v4()),
                "****5678",
                "JANE DOE",
                "033",
                "UBA",
                "NGN",
                None,
                "paystack",
            )
            .await
            .expect("insert");

        repo.update_linked_account_status(account.id, "unlinked")
            .await
            .expect("unlink");

        let accounts = repo
            .list_linked_accounts_for_user(user_id)
            .await
            .expect("list");
        assert!(accounts.is_empty(), "Unlinked account should not appear in list");
    }

    // ── Mandate Lifecycle ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_mandate_create_and_revoke() {
        use Bitmesh_backend::banking::repository::BankingRepository;

        let pool = test_pool().await;
        let repo = BankingRepository::new(pool);
        let user_id = Uuid::new_v4();

        let account = repo
            .insert_linked_account(
                user_id,
                &format!("token-{}", Uuid::new_v4()),
                "****9999",
                "TEST USER",
                "044",
                "Access Bank",
                "NGN",
                None,
                "paystack",
            )
            .await
            .expect("insert account");

        let mandate = repo
            .insert_mandate(
                account.id,
                user_id,
                "debit",
                500_000, // ₦5,000 in kobo
                &format!("AUTH-{}", Uuid::new_v4()),
                "paystack",
            )
            .await
            .expect("insert mandate");

        assert_eq!(mandate.status, "active");
        assert_eq!(mandate.max_amount, 500_000);

        // Active mandate should be retrievable
        let active = repo
            .get_active_mandate(account.id, "debit")
            .await
            .expect("get active mandate");
        assert!(active.is_some());
        assert_eq!(active.unwrap().id, mandate.id);

        // Revoke
        repo.revoke_mandate(mandate.id).await.expect("revoke");

        let after_revoke = repo
            .get_active_mandate(account.id, "debit")
            .await
            .expect("get after revoke");
        assert!(after_revoke.is_none(), "Revoked mandate should not be active");
    }

    // ── Idempotent Transfer ───────────────────────────────────────────────────

    #[tokio::test]
    async fn test_transfer_idempotency() {
        use Bitmesh_backend::banking::repository::BankingRepository;

        let pool = test_pool().await;
        let repo = BankingRepository::new(pool);
        let user_id = Uuid::new_v4();
        let idempotency_key = format!("idem-{}", Uuid::new_v4());

        let account = repo
            .insert_linked_account(
                user_id,
                &format!("token-{}", Uuid::new_v4()),
                "****0001",
                "IDEM USER",
                "058",
                "GTBank",
                "NGN",
                None,
                "paystack",
            )
            .await
            .expect("insert account");

        // First insert
        let t1 = repo
            .upsert_transfer(
                &idempotency_key,
                None,
                account.id,
                "debit",
                100_000,
                "NGN",
                "paystack",
            )
            .await
            .expect("first upsert");

        // Second insert with same key — must return same record
        let t2 = repo
            .upsert_transfer(
                &idempotency_key,
                None,
                account.id,
                "debit",
                100_000,
                "NGN",
                "paystack",
            )
            .await
            .expect("second upsert");

        assert_eq!(t1.id, t2.id, "Idempotent upsert must return same record");
    }

    #[tokio::test]
    async fn test_transfer_status_update() {
        use Bitmesh_backend::banking::repository::BankingRepository;

        let pool = test_pool().await;
        let repo = BankingRepository::new(pool);
        let user_id = Uuid::new_v4();
        let key = format!("key-{}", Uuid::new_v4());

        let account = repo
            .insert_linked_account(
                user_id,
                &format!("token-{}", Uuid::new_v4()),
                "****0002",
                "STATUS USER",
                "058",
                "GTBank",
                "NGN",
                None,
                "paystack",
            )
            .await
            .expect("insert");

        let transfer = repo
            .upsert_transfer(&key, None, account.id, "credit", 200_000, "NGN", "paystack")
            .await
            .expect("upsert");

        repo.update_transfer_status(transfer.id, "success", Some("TRF-REF-001"), None, None)
            .await
            .expect("update status");

        let updated = repo
            .get_transfer_by_idempotency_key(&key)
            .await
            .expect("get by key")
            .expect("should exist");

        assert_eq!(updated.status, "success");
        assert_eq!(updated.provider_reference.as_deref(), Some("TRF-REF-001"));
        assert!(updated.settled_at.is_some());
    }

    // ── Webhook Processing ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_webhook_idempotency() {
        use Bitmesh_backend::banking::repository::BankingRepository;

        let pool = test_pool().await;
        let repo = BankingRepository::new(pool);
        let event_id = format!("evt-{}", Uuid::new_v4());

        let payload = serde_json::json!({
            "event": "charge.success",
            "data": { "id": event_id, "reference": "REF-001" }
        });

        let e1 = repo
            .upsert_webhook_event("paystack", "charge.success", &event_id, &payload)
            .await
            .expect("first insert");

        let e2 = repo
            .upsert_webhook_event("paystack", "charge.success", &event_id, &payload)
            .await
            .expect("second insert");

        assert_eq!(e1.id, e2.id, "Duplicate webhook must return same record");
    }

    #[tokio::test]
    async fn test_webhook_processor_success_event() {
        use Bitmesh_backend::banking::{repository::BankingRepository, webhook::BankWebhookProcessor};

        let pool = test_pool().await;
        let user_id = Uuid::new_v4();
        let key = format!("ref-{}", Uuid::new_v4());

        let repo = Arc::new(BankingRepository::new(pool.clone()));

        // Set up a pending transfer
        let account = repo
            .insert_linked_account(
                user_id,
                &format!("token-{}", Uuid::new_v4()),
                "****0003",
                "WEBHOOK USER",
                "058",
                "GTBank",
                "NGN",
                None,
                "paystack",
            )
            .await
            .expect("insert account");

        let transfer = repo
            .upsert_transfer(&key, None, account.id, "debit", 50_000, "NGN", "paystack")
            .await
            .expect("upsert transfer");

        assert_eq!(transfer.status, "pending");

        // Process success webhook
        let processor = BankWebhookProcessor::new(repo.clone());
        let payload = serde_json::json!({
            "event": "charge.success",
            "data": {
                "id": format!("evt-{}", Uuid::new_v4()),
                "reference": key,
                "gateway_response": "Approved"
            }
        });

        processor
            .process("paystack", &payload)
            .await
            .expect("process webhook");

        // Transfer should now be 'success'
        let updated = repo
            .get_transfer_by_idempotency_key(&key)
            .await
            .expect("get transfer")
            .expect("should exist");

        assert_eq!(updated.status, "success");
    }

    // ── Reconciliation ────────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_reconciliation_run_upsert() {
        use Bitmesh_backend::banking::repository::BankingRepository;
        use sqlx::types::BigDecimal;
        use std::str::FromStr;

        let pool = test_pool().await;
        let repo = BankingRepository::new(pool);
        let date = chrono::Utc::now().date_naive();

        let run = repo
            .upsert_reconciliation_run(
                date,
                "058",
                &BigDecimal::from_str("1000000").unwrap(),
                &BigDecimal::from_str("1000000").unwrap(),
                &BigDecimal::from(0),
                0,
                "equilibrium",
                None,
            )
            .await
            .expect("upsert recon run");

        assert_eq!(run.status, "equilibrium");
        assert_eq!(run.flagged_count, 0);

        // Upsert again with discrepancy — should update
        let updated = repo
            .upsert_reconciliation_run(
                date,
                "058",
                &BigDecimal::from_str("1000000").unwrap(),
                &BigDecimal::from_str("990000").unwrap(),
                &BigDecimal::from_str("10000").unwrap(),
                1,
                "discrepancy",
                None,
            )
            .await
            .expect("upsert updated");

        assert_eq!(updated.status, "discrepancy");
        assert_eq!(updated.flagged_count, 1);
    }
}
