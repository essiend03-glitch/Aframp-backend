//! Banking Integration — Database Repository (Issue #407)

use super::models::{
    BankMandate, BankReconciliationRun, BankTransferLog, BankWebhookEvent, LinkedBankAccount,
};
use chrono::NaiveDate;
use sqlx::types::BigDecimal;
use sqlx::PgPool;
use uuid::Uuid;

pub struct BankingRepository {
    pool: PgPool,
}

impl BankingRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // ── Linked Bank Accounts ──────────────────────────────────────────────────

    pub async fn insert_linked_account(
        &self,
        user_id: Uuid,
        account_token: &str,
        account_mask: &str,
        account_name: &str,
        bank_code: &str,
        bank_name: &str,
        currency: &str,
        identity_hash: Option<&str>,
        verified_by: &str,
    ) -> anyhow::Result<LinkedBankAccount> {
        Ok(sqlx::query_as::<_, LinkedBankAccount>(
            r#"
            INSERT INTO linked_bank_accounts
                (user_id, account_token, account_mask, account_name, bank_code, bank_name,
                 currency, identity_hash, verified_by)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
            RETURNING *
            "#,
        )
        .bind(user_id)
        .bind(account_token)
        .bind(account_mask)
        .bind(account_name)
        .bind(bank_code)
        .bind(bank_name)
        .bind(currency)
        .bind(identity_hash)
        .bind(verified_by)
        .fetch_one(&self.pool)
        .await?)
    }

    pub async fn get_linked_account(&self, id: Uuid) -> anyhow::Result<LinkedBankAccount> {
        Ok(sqlx::query_as::<_, LinkedBankAccount>(
            "SELECT * FROM linked_bank_accounts WHERE id = $1",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?)
    }

    pub async fn list_linked_accounts_for_user(
        &self,
        user_id: Uuid,
    ) -> anyhow::Result<Vec<LinkedBankAccount>> {
        Ok(sqlx::query_as::<_, LinkedBankAccount>(
            "SELECT * FROM linked_bank_accounts WHERE user_id = $1 AND status != 'unlinked' ORDER BY created_at DESC",
        )
        .bind(user_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn update_linked_account_status(
        &self,
        id: Uuid,
        status: &str,
    ) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE linked_bank_accounts SET status = $2, updated_at = NOW() WHERE id = $1",
        )
        .bind(id)
        .bind(status)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // ── Mandates ──────────────────────────────────────────────────────────────

    pub async fn insert_mandate(
        &self,
        linked_account_id: Uuid,
        user_id: Uuid,
        mandate_type: &str,
        max_amount: i64,
        provider_reference: &str,
        provider: &str,
    ) -> anyhow::Result<BankMandate> {
        Ok(sqlx::query_as::<_, BankMandate>(
            r#"
            INSERT INTO bank_mandates
                (linked_account_id, user_id, mandate_type, max_amount, provider_reference, provider)
            VALUES ($1,$2,$3,$4,$5,$6)
            RETURNING *
            "#,
        )
        .bind(linked_account_id)
        .bind(user_id)
        .bind(mandate_type)
        .bind(max_amount)
        .bind(provider_reference)
        .bind(provider)
        .fetch_one(&self.pool)
        .await?)
    }

    pub async fn get_active_mandate(
        &self,
        linked_account_id: Uuid,
        mandate_type: &str,
    ) -> anyhow::Result<Option<BankMandate>> {
        Ok(sqlx::query_as::<_, BankMandate>(
            r#"
            SELECT * FROM bank_mandates
            WHERE linked_account_id = $1
              AND mandate_type = $2
              AND status = 'active'
              AND (expires_at IS NULL OR expires_at > NOW())
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(linked_account_id)
        .bind(mandate_type)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn revoke_mandate(&self, id: Uuid) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE bank_mandates SET status = 'revoked', revoked_at = NOW(), updated_at = NOW() WHERE id = $1",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // ── Transfer Log ──────────────────────────────────────────────────────────

    /// Returns existing entry if idempotency_key already exists (idempotent insert).
    pub async fn upsert_transfer(
        &self,
        idempotency_key: &str,
        mandate_id: Option<Uuid>,
        linked_account_id: Uuid,
        direction: &str,
        amount: i64,
        currency: &str,
        provider: &str,
    ) -> anyhow::Result<BankTransferLog> {
        Ok(sqlx::query_as::<_, BankTransferLog>(
            r#"
            INSERT INTO bank_transfer_log
                (idempotency_key, mandate_id, linked_account_id, direction, amount, currency, provider)
            VALUES ($1,$2,$3,$4,$5,$6,$7)
            ON CONFLICT (idempotency_key) DO UPDATE SET updated_at = bank_transfer_log.updated_at
            RETURNING *
            "#,
        )
        .bind(idempotency_key)
        .bind(mandate_id)
        .bind(linked_account_id)
        .bind(direction)
        .bind(amount)
        .bind(currency)
        .bind(provider)
        .fetch_one(&self.pool)
        .await?)
    }

    pub async fn update_transfer_status(
        &self,
        id: Uuid,
        status: &str,
        provider_reference: Option<&str>,
        provider_response: Option<&serde_json::Value>,
        failure_reason: Option<&str>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            UPDATE bank_transfer_log
            SET status = $2,
                provider_reference = COALESCE($3, provider_reference),
                provider_response  = COALESCE($4, provider_response),
                failure_reason     = COALESCE($5, failure_reason),
                settled_at         = CASE WHEN $2 = 'success' THEN NOW() ELSE settled_at END,
                updated_at         = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(status)
        .bind(provider_reference)
        .bind(provider_response)
        .bind(failure_reason)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_transfer_by_idempotency_key(
        &self,
        key: &str,
    ) -> anyhow::Result<Option<BankTransferLog>> {
        Ok(sqlx::query_as::<_, BankTransferLog>(
            "SELECT * FROM bank_transfer_log WHERE idempotency_key = $1",
        )
        .bind(key)
        .fetch_optional(&self.pool)
        .await?)
    }

    // ── Reconciliation ────────────────────────────────────────────────────────

    pub async fn upsert_reconciliation_run(
        &self,
        run_date: NaiveDate,
        bank_code: &str,
        aframp_total: &BigDecimal,
        bank_total: &BigDecimal,
        discrepancy: &BigDecimal,
        flagged_count: i32,
        status: &str,
        metadata: Option<&serde_json::Value>,
    ) -> anyhow::Result<BankReconciliationRun> {
        Ok(sqlx::query_as::<_, BankReconciliationRun>(
            r#"
            INSERT INTO bank_reconciliation_runs
                (run_date, bank_code, aframp_total, bank_total, discrepancy, flagged_count, status, metadata)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
            ON CONFLICT (run_date, bank_code) DO UPDATE
                SET aframp_total  = EXCLUDED.aframp_total,
                    bank_total    = EXCLUDED.bank_total,
                    discrepancy   = EXCLUDED.discrepancy,
                    flagged_count = EXCLUDED.flagged_count,
                    status        = EXCLUDED.status,
                    metadata      = EXCLUDED.metadata
            RETURNING *
            "#,
        )
        .bind(run_date)
        .bind(bank_code)
        .bind(aframp_total)
        .bind(bank_total)
        .bind(discrepancy)
        .bind(flagged_count)
        .bind(status)
        .bind(metadata)
        .fetch_one(&self.pool)
        .await?)
    }

    pub async fn list_reconciliation_runs(
        &self,
        limit: i64,
    ) -> anyhow::Result<Vec<BankReconciliationRun>> {
        Ok(sqlx::query_as::<_, BankReconciliationRun>(
            "SELECT * FROM bank_reconciliation_runs ORDER BY run_date DESC LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?)
    }

    /// Sum of successful transfers for a bank on a given date (for reconciliation)
    pub async fn sum_settled_transfers(
        &self,
        bank_code: &str,
        date: NaiveDate,
    ) -> anyhow::Result<BigDecimal> {
        let row: Option<(BigDecimal,)> = sqlx::query_as(
            r#"
            SELECT COALESCE(SUM(btl.amount), 0)::NUMERIC
            FROM bank_transfer_log btl
            JOIN linked_bank_accounts lba ON lba.id = btl.linked_account_id
            WHERE lba.bank_code = $1
              AND btl.status = 'success'
              AND DATE(btl.settled_at) = $2
            "#,
        )
        .bind(bank_code)
        .bind(date)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(|(v,)| v).unwrap_or_else(|| BigDecimal::from(0)))
    }

    // ── Webhook Events ────────────────────────────────────────────────────────

    /// Idempotent insert — returns existing row if (provider, provider_event_id) already exists.
    pub async fn upsert_webhook_event(
        &self,
        provider: &str,
        event_type: &str,
        provider_event_id: &str,
        payload: &serde_json::Value,
    ) -> anyhow::Result<BankWebhookEvent> {
        Ok(sqlx::query_as::<_, BankWebhookEvent>(
            r#"
            INSERT INTO bank_webhook_events (provider, event_type, provider_event_id, payload)
            VALUES ($1,$2,$3,$4)
            ON CONFLICT (provider, provider_event_id) DO UPDATE SET status = bank_webhook_events.status
            RETURNING *
            "#,
        )
        .bind(provider)
        .bind(event_type)
        .bind(provider_event_id)
        .bind(payload)
        .fetch_one(&self.pool)
        .await?)
    }

    pub async fn mark_webhook_processed(
        &self,
        id: Uuid,
        linked_account_id: Option<Uuid>,
        transfer_log_id: Option<Uuid>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            UPDATE bank_webhook_events
            SET status = 'processed',
                linked_account_id = COALESCE($2, linked_account_id),
                transfer_log_id   = COALESCE($3, transfer_log_id),
                processed_at      = NOW()
            WHERE id = $1
            "#,
        )
        .bind(id)
        .bind(linked_account_id)
        .bind(transfer_log_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn mark_webhook_failed(&self, id: Uuid, error: &str) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE bank_webhook_events SET status = 'failed', error_message = $2, processed_at = NOW() WHERE id = $1",
        )
        .bind(id)
        .bind(error)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
