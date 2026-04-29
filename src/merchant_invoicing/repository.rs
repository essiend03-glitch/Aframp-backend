//! Database access layer for merchant invoicing and tax.

use crate::database::error::DatabaseError;
use crate::merchant_invoicing::models::*;
use chrono::NaiveDate;
use sqlx::PgPool;
use uuid::Uuid;

pub struct InvoicingRepository {
    pool: PgPool,
}

impl InvoicingRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // -------------------------------------------------------------------------
    // Tax rules
    // -------------------------------------------------------------------------

    pub async fn create_tax_rule(
        &self,
        merchant_id: Uuid,
        req: &CreateTaxRuleRequest,
    ) -> Result<TaxRule, DatabaseError> {
        let applies: Vec<&str> = req.applies_to.iter().map(|s| s.as_str()).collect();
        sqlx::query_as::<_, TaxRule>(
            r#"
            INSERT INTO merchant_tax_rules
                (merchant_id, name, region, tax_type, rate_bps, is_inclusive,
                 applies_to, effective_from, effective_until)
            VALUES ($1,$2,$3,$4,$5,$6,$7,
                    COALESCE($8, now()),
                    $9)
            RETURNING *
            "#,
        )
        .bind(merchant_id)
        .bind(&req.name)
        .bind(&req.region)
        .bind(&req.tax_type)
        .bind(req.rate_bps)
        .bind(req.is_inclusive)
        .bind(applies)
        .bind(req.effective_from)
        .bind(req.effective_until)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    /// Fetch active tax rules for a merchant in a given region.
    pub async fn get_active_rules(
        &self,
        merchant_id: Uuid,
        region: &str,
    ) -> Result<Vec<TaxRule>, DatabaseError> {
        sqlx::query_as::<_, TaxRule>(
            r#"
            SELECT * FROM merchant_tax_rules
            WHERE merchant_id = $1
              AND region = $2
              AND is_active = TRUE
              AND effective_from <= now()
              AND (effective_until IS NULL OR effective_until > now())
            ORDER BY tax_type
            "#,
        )
        .bind(merchant_id)
        .bind(region)
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    pub async fn list_tax_rules(
        &self,
        merchant_id: Uuid,
    ) -> Result<Vec<TaxRule>, DatabaseError> {
        sqlx::query_as::<_, TaxRule>(
            "SELECT * FROM merchant_tax_rules WHERE merchant_id = $1 ORDER BY region, tax_type",
        )
        .bind(merchant_id)
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    // -------------------------------------------------------------------------
    // Invoices
    // -------------------------------------------------------------------------

    pub async fn create_invoice(
        &self,
        merchant_id: Uuid,
        invoice_number: &str,
        transaction_id: Option<Uuid>,
        wallet_address: Option<&str>,
        customer_profile_id: Option<Uuid>,
        subtotal: f64,
        tax_amount: f64,
        total_amount: f64,
        currency: &str,
        tax_breakdown: &serde_json::Value,
        line_items: &serde_json::Value,
        content_hash: &str,
        digital_signature: &str,
        qr_code_data: Option<&str>,
        due_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<Invoice, DatabaseError> {
        sqlx::query_as::<_, Invoice>(
            r#"
            INSERT INTO merchant_invoices
                (merchant_id, invoice_number, transaction_id, wallet_address,
                 customer_profile_id, subtotal, tax_amount, total_amount, currency,
                 tax_breakdown, line_items, status, content_hash, digital_signature,
                 qr_code_data, issued_at, due_at)
            VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,'issued',$12,$13,$14,now(),$15)
            RETURNING *
            "#,
        )
        .bind(merchant_id)
        .bind(invoice_number)
        .bind(transaction_id)
        .bind(wallet_address)
        .bind(customer_profile_id)
        .bind(subtotal)
        .bind(tax_amount)
        .bind(total_amount)
        .bind(currency)
        .bind(tax_breakdown)
        .bind(line_items)
        .bind(content_hash)
        .bind(digital_signature)
        .bind(qr_code_data)
        .bind(due_at)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    pub async fn get_invoice(
        &self,
        merchant_id: Uuid,
        invoice_id: Uuid,
    ) -> Result<Option<Invoice>, DatabaseError> {
        sqlx::query_as::<_, Invoice>(
            "SELECT * FROM merchant_invoices WHERE id = $1 AND merchant_id = $2",
        )
        .bind(invoice_id)
        .bind(merchant_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    pub async fn list_invoices(
        &self,
        merchant_id: Uuid,
        status: Option<&str>,
        page: i64,
        page_size: i64,
    ) -> Result<Vec<Invoice>, DatabaseError> {
        let offset = (page - 1) * page_size;
        sqlx::query_as::<_, Invoice>(
            r#"
            SELECT * FROM merchant_invoices
            WHERE merchant_id = $1
              AND ($2::text IS NULL OR status = $2)
            ORDER BY created_at DESC
            LIMIT $3 OFFSET $4
            "#,
        )
        .bind(merchant_id)
        .bind(status)
        .bind(page_size)
        .bind(offset)
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    // -------------------------------------------------------------------------
    // Tax reports
    // -------------------------------------------------------------------------

    pub async fn generate_tax_report(
        &self,
        merchant_id: Uuid,
        period_start: NaiveDate,
        period_end: NaiveDate,
        attestation_hash: &str,
    ) -> Result<TaxReport, DatabaseError> {
        // Aggregate from invoices in the period
        sqlx::query_as::<_, TaxReport>(
            r#"
            INSERT INTO merchant_tax_reports
                (merchant_id, report_period_start, report_period_end,
                 total_gross_revenue, total_tax_collected, total_net_revenue,
                 invoice_count, tax_breakdown_by_type,
                 attestation_hash, attestation_generated_at, status)
            SELECT
                $1,
                $2::date,
                $3::date,
                COALESCE(SUM(total_amount), 0),
                COALESCE(SUM(tax_amount), 0),
                COALESCE(SUM(subtotal), 0),
                COUNT(*),
                '{}',
                $4,
                now(),
                'finalized'
            FROM merchant_invoices
            WHERE merchant_id = $1
              AND status IN ('issued','paid')
              AND issued_at::date BETWEEN $2 AND $3
            ON CONFLICT (merchant_id, report_period_start, report_period_end) DO UPDATE SET
                total_gross_revenue      = EXCLUDED.total_gross_revenue,
                total_tax_collected      = EXCLUDED.total_tax_collected,
                total_net_revenue        = EXCLUDED.total_net_revenue,
                invoice_count            = EXCLUDED.invoice_count,
                attestation_hash         = EXCLUDED.attestation_hash,
                attestation_generated_at = EXCLUDED.attestation_generated_at,
                status                   = 'finalized',
                updated_at               = now()
            RETURNING *
            "#,
        )
        .bind(merchant_id)
        .bind(period_start)
        .bind(period_end)
        .bind(attestation_hash)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    pub async fn list_tax_reports(
        &self,
        merchant_id: Uuid,
    ) -> Result<Vec<TaxReport>, DatabaseError> {
        sqlx::query_as::<_, TaxReport>(
            "SELECT * FROM merchant_tax_reports WHERE merchant_id = $1 ORDER BY report_period_start DESC",
        )
        .bind(merchant_id)
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }
}
