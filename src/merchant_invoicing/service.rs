//! Business logic for Merchant Invoicing & Automated Tax Calculation (Issue #333).

use crate::error::Error;
use crate::merchant_invoicing::{
    models::*,
    repository::InvoicingRepository,
    tax_engine::calculate_tax,
};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tracing::info;
use uuid::Uuid;

pub struct MerchantInvoicingService {
    repo: Arc<InvoicingRepository>,
}

impl MerchantInvoicingService {
    pub fn new(repo: Arc<InvoicingRepository>) -> Self {
        Self { repo }
    }

    // -------------------------------------------------------------------------
    // Tax rules
    // -------------------------------------------------------------------------

    pub async fn create_tax_rule(
        &self,
        merchant_id: Uuid,
        req: CreateTaxRuleRequest,
    ) -> Result<TaxRule, Error> {
        if req.rate_bps < 0 || req.rate_bps > 10_000 {
            return Err(Error::BadRequest(
                "rate_bps must be between 0 and 10000".into(),
            ));
        }
        self.repo
            .create_tax_rule(merchant_id, &req)
            .await
            .map_err(|e| Error::Internal(e.to_string()))
    }

    pub async fn list_tax_rules(&self, merchant_id: Uuid) -> Result<Vec<TaxRule>, Error> {
        self.repo
            .list_tax_rules(merchant_id)
            .await
            .map_err(|e| Error::Internal(e.to_string()))
    }

    // -------------------------------------------------------------------------
    // Invoice generation
    // -------------------------------------------------------------------------

    pub async fn create_invoice(
        &self,
        merchant_id: Uuid,
        req: CreateInvoiceRequest,
    ) -> Result<Invoice, Error> {
        if req.line_items.is_empty() {
            return Err(Error::BadRequest("At least one line item is required".into()));
        }

        // Fetch applicable tax rules
        let rules = self
            .repo
            .get_active_rules(merchant_id, &req.region)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;

        // Calculate tax
        let tax_result = calculate_tax(&req.line_items, &rules);

        // Build canonical content for hashing / signing
        let invoice_number = generate_invoice_number(merchant_id);
        let currency = req.currency.as_deref().unwrap_or("cNGN");

        let line_items_json = serde_json::to_value(&req.line_items)
            .map_err(|e| Error::Internal(e.to_string()))?;
        let tax_breakdown_json = serde_json::to_value(&tax_result.tax_breakdown)
            .map_err(|e| Error::Internal(e.to_string()))?;

        let canonical = format!(
            "{}|{}|{}|{}|{}|{}",
            invoice_number,
            merchant_id,
            tax_result.subtotal,
            tax_result.tax_amount,
            tax_result.total_amount,
            currency
        );
        let content_hash = hex::encode(Sha256::digest(canonical.as_bytes()));
        // Platform signature: HMAC-SHA256 of content_hash with ENCRYPTION_KEY
        let digital_signature = sign_content(&content_hash);

        // QR code data: JSON with invoice number + total
        let qr_data = format!(
            r#"{{"invoice":"{}","total":"{}","currency":"{}"}}"#,
            invoice_number, tax_result.total_amount, currency
        );

        let invoice = self
            .repo
            .create_invoice(
                merchant_id,
                &invoice_number,
                req.transaction_id,
                req.wallet_address.as_deref(),
                req.customer_profile_id,
                tax_result.subtotal,
                tax_result.tax_amount,
                tax_result.total_amount,
                currency,
                &tax_breakdown_json,
                &line_items_json,
                &content_hash,
                &digital_signature,
                Some(&qr_data),
                req.due_at,
            )
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;

        info!(
            merchant_id = %merchant_id,
            invoice_number = %invoice.invoice_number,
            total = %invoice.total_amount,
            "Invoice created"
        );

        Ok(invoice)
    }

    pub async fn get_invoice(
        &self,
        merchant_id: Uuid,
        invoice_id: Uuid,
    ) -> Result<Invoice, Error> {
        self.repo
            .get_invoice(merchant_id, invoice_id)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?
            .ok_or_else(|| Error::NotFound("Invoice not found".into()))
    }

    pub async fn list_invoices(
        &self,
        merchant_id: Uuid,
        query: &InvoiceListQuery,
    ) -> Result<Vec<Invoice>, Error> {
        let page = query.page.unwrap_or(1).max(1);
        let page_size = query.page_size.unwrap_or(50).min(200);
        self.repo
            .list_invoices(merchant_id, query.status.as_deref(), page, page_size)
            .await
            .map_err(|e| Error::Internal(e.to_string()))
    }

    // -------------------------------------------------------------------------
    // Tax preview (before payment)
    // -------------------------------------------------------------------------

    pub async fn preview_tax(
        &self,
        merchant_id: Uuid,
        line_items: Vec<LineItem>,
        region: &str,
    ) -> Result<TaxCalculationResult, Error> {
        let rules = self
            .repo
            .get_active_rules(merchant_id, region)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;
        Ok(calculate_tax(&line_items, &rules))
    }

    // -------------------------------------------------------------------------
    // Tax reports (FIRS)
    // -------------------------------------------------------------------------

    pub async fn generate_tax_report(
        &self,
        merchant_id: Uuid,
        req: GenerateTaxReportRequest,
    ) -> Result<TaxReport, Error> {
        if req.period_end < req.period_start {
            return Err(Error::BadRequest(
                "period_end must be >= period_start".into(),
            ));
        }

        // Attestation hash: SHA-256 of "merchant_id|start|end|timestamp"
        let attestation_input = format!(
            "{}|{}|{}|{}",
            merchant_id,
            req.period_start,
            req.period_end,
            chrono::Utc::now().timestamp()
        );
        let attestation_hash = hex::encode(Sha256::digest(attestation_input.as_bytes()));

        let report = self
            .repo
            .generate_tax_report(
                merchant_id,
                req.period_start,
                req.period_end,
                &attestation_hash,
            )
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;

        info!(
            merchant_id = %merchant_id,
            period = %req.period_start,
            total_tax = %report.total_tax_collected,
            "Tax report generated"
        );

        Ok(report)
    }

    pub async fn list_tax_reports(&self, merchant_id: Uuid) -> Result<Vec<TaxReport>, Error> {
        self.repo
            .list_tax_reports(merchant_id)
            .await
            .map_err(|e| Error::Internal(e.to_string()))
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn generate_invoice_number(merchant_id: Uuid) -> String {
    let ts = chrono::Utc::now().format("%Y%m%d%H%M%S");
    let short_id = &merchant_id.to_string()[..8];
    format!("INV-{}-{}", short_id.to_uppercase(), ts)
}

fn sign_content(content_hash: &str) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    type HmacSha256 = Hmac<Sha256>;

    let key = std::env::var("ENCRYPTION_KEY").unwrap_or_else(|_| "default-key".into());
    let mut mac = HmacSha256::new_from_slice(key.as_bytes())
        .expect("HMAC can take key of any size");
    mac.update(content_hash.as_bytes());
    hex::encode(mac.finalize().into_bytes())
}
