//! Data models for merchant invoicing and tax calculation.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Database row types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct TaxRule {
    pub id: Uuid,
    pub merchant_id: Uuid,
    pub name: String,
    pub region: String,
    pub tax_type: String,
    pub rate_bps: i32,
    pub is_inclusive: bool,
    pub applies_to: Vec<String>,
    pub is_active: bool,
    pub effective_from: DateTime<Utc>,
    pub effective_until: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Invoice {
    pub id: Uuid,
    pub merchant_id: Uuid,
    pub invoice_number: String,
    pub transaction_id: Option<Uuid>,
    pub wallet_address: Option<String>,
    pub customer_profile_id: Option<Uuid>,
    pub subtotal: sqlx::types::BigDecimal,
    pub tax_amount: sqlx::types::BigDecimal,
    pub total_amount: sqlx::types::BigDecimal,
    pub currency: String,
    pub tax_breakdown: serde_json::Value,
    pub line_items: serde_json::Value,
    pub status: String,
    pub content_hash: Option<String>,
    pub digital_signature: Option<String>,
    pub pdf_storage_key: Option<String>,
    pub qr_code_data: Option<String>,
    pub accounting_sync_status: Option<String>,
    pub accounting_external_id: Option<String>,
    pub accounting_platform: Option<String>,
    pub issued_at: Option<DateTime<Utc>>,
    pub due_at: Option<DateTime<Utc>>,
    pub paid_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct TaxReport {
    pub id: Uuid,
    pub merchant_id: Uuid,
    pub report_period_start: NaiveDate,
    pub report_period_end: NaiveDate,
    pub total_gross_revenue: sqlx::types::BigDecimal,
    pub total_tax_collected: sqlx::types::BigDecimal,
    pub total_net_revenue: sqlx::types::BigDecimal,
    pub invoice_count: i32,
    pub tax_breakdown_by_type: serde_json::Value,
    pub attestation_hash: Option<String>,
    pub attestation_generated_at: Option<DateTime<Utc>>,
    pub status: String,
    pub submitted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Request / Response DTOs
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CreateTaxRuleRequest {
    pub name: String,
    pub region: String,
    pub tax_type: String,
    /// Rate in basis points (e.g. 750 = 7.5%)
    pub rate_bps: i32,
    pub is_inclusive: bool,
    pub applies_to: Vec<String>,
    pub effective_from: Option<DateTime<Utc>>,
    pub effective_until: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct CreateInvoiceRequest {
    pub transaction_id: Option<Uuid>,
    pub wallet_address: Option<String>,
    pub customer_profile_id: Option<Uuid>,
    pub line_items: Vec<LineItem>,
    pub region: String,
    pub currency: Option<String>,
    pub due_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LineItem {
    pub description: String,
    pub quantity: f64,
    pub unit_price: f64,
    pub category: Option<String>,
}

/// Result of tax calculation for a set of line items.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxCalculationResult {
    pub subtotal: f64,
    pub tax_amount: f64,
    pub total_amount: f64,
    pub tax_breakdown: Vec<TaxBreakdownEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaxBreakdownEntry {
    pub tax_type: String,
    pub rate_bps: i32,
    pub taxable_amount: f64,
    pub tax_amount: f64,
}

#[derive(Debug, Deserialize)]
pub struct GenerateTaxReportRequest {
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
}

#[derive(Debug, Deserialize)]
pub struct InvoiceListQuery {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
    pub status: Option<String>,
}
