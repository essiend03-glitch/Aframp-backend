//! CRM data models for merchant customer profiling and segmentation.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Database row types
// ---------------------------------------------------------------------------

/// A customer profile derived from wallet transaction clustering.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct CustomerProfile {
    pub id: Uuid,
    pub merchant_id: Uuid,
    pub wallet_address: String,
    pub display_name: Option<String>,
    /// AES-256 encrypted contact fields (stored ciphertext)
    pub encrypted_email: Option<String>,
    pub encrypted_phone: Option<String>,
    pub encrypted_name: Option<String>,
    pub consent_given: bool,
    pub consent_given_at: Option<DateTime<Utc>>,
    pub total_spent: sqlx::types::BigDecimal,
    pub total_transactions: i32,
    pub first_transaction_at: Option<DateTime<Utc>>,
    pub last_transaction_at: Option<DateTime<Utc>>,
    pub is_repeat_customer: bool,
    pub tags: Vec<String>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A named customer segment with filter criteria.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct CustomerSegment {
    pub id: Uuid,
    pub merchant_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub filter_criteria: serde_json::Value,
    pub customer_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Daily analytics snapshot per customer per merchant.
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct CustomerAnalyticsSnapshot {
    pub id: Uuid,
    pub merchant_id: Uuid,
    pub wallet_address: String,
    pub avg_purchase_frequency_days: Option<sqlx::types::BigDecimal>,
    pub days_since_last_purchase: Option<i32>,
    pub avg_transaction_value: Option<sqlx::types::BigDecimal>,
    pub max_transaction_value: Option<sqlx::types::BigDecimal>,
    pub min_transaction_value: Option<sqlx::types::BigDecimal>,
    pub retention_score: Option<sqlx::types::BigDecimal>,
    pub snapshot_date: NaiveDate,
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Request / Response DTOs
// ---------------------------------------------------------------------------

/// Request to upsert a customer profile (consent opt-in flow).
#[derive(Debug, Deserialize)]
pub struct UpsertCustomerProfileRequest {
    pub wallet_address: String,
    pub display_name: Option<String>,
    /// Plaintext contact details — encrypted before storage.
    pub email: Option<String>,
    pub phone: Option<String>,
    pub name: Option<String>,
    pub consent_given: bool,
    pub consent_ip_address: Option<String>,
    pub tags: Option<Vec<String>>,
}

/// Request to create or update a customer segment.
#[derive(Debug, Deserialize)]
pub struct UpsertSegmentRequest {
    pub name: String,
    pub description: Option<String>,
    pub filter_criteria: SegmentFilterCriteria,
}

/// Filter criteria for customer segmentation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentFilterCriteria {
    /// Minimum total spend in cNGN (e.g. 100_000)
    pub min_total_spent: Option<f64>,
    pub max_total_spent: Option<f64>,
    /// Only customers with transactions in the last N days
    pub active_within_days: Option<i32>,
    /// Minimum number of transactions
    pub min_transactions: Option<i32>,
    /// Must have all of these tags
    pub required_tags: Option<Vec<String>>,
    /// Must be a repeat customer
    pub repeat_only: Option<bool>,
}

/// Public-facing customer profile (decrypted contact details for authorized merchants).
#[derive(Debug, Serialize)]
pub struct CustomerProfileResponse {
    pub id: Uuid,
    pub wallet_address: String,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub name: Option<String>,
    pub consent_given: bool,
    pub consent_given_at: Option<DateTime<Utc>>,
    pub total_spent: String,
    pub total_transactions: i32,
    pub first_transaction_at: Option<DateTime<Utc>>,
    pub last_transaction_at: Option<DateTime<Utc>>,
    pub is_repeat_customer: bool,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
}

/// Anonymised export row (GDPR/NDPR compliant).
#[derive(Debug, Serialize)]
pub struct AnonymisedCustomerExport {
    /// Truncated wallet address (first 6 + last 4 chars)
    pub wallet_address_masked: String,
    pub total_spent: String,
    pub total_transactions: i32,
    pub is_repeat_customer: bool,
    pub tags: Vec<String>,
    pub first_transaction_at: Option<DateTime<Utc>>,
    pub last_transaction_at: Option<DateTime<Utc>>,
}

/// Merchant-level retention metric.
#[derive(Debug, Serialize)]
pub struct RetentionMetrics {
    pub total_customers: i64,
    pub repeat_customers: i64,
    /// Percentage of customers who returned (0.0 – 100.0)
    pub retention_rate_pct: f64,
    pub avg_purchase_frequency_days: Option<f64>,
    pub avg_days_since_last_purchase: Option<f64>,
}

/// Query parameters for customer list endpoint.
#[derive(Debug, Deserialize)]
pub struct CustomerListQuery {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
    pub min_spent: Option<f64>,
    pub active_within_days: Option<i32>,
    pub tag: Option<String>,
    pub repeat_only: Option<bool>,
    /// "full" (decrypted) or "anonymised"
    pub export_mode: Option<String>,
}
