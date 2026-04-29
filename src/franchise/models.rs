//! Data models for multi-store franchise management.

use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Database row types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct Organization {
    pub id: Uuid,
    pub owner_user_id: Uuid,
    pub name: String,
    pub slug: String,
    pub logo_url: Option<String>,
    pub settlement_mode: String,
    pub centralized_bank_account_id: Option<String>,
    pub global_policies: serde_json::Value,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct OrganizationRegion {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub code: String,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct OrganizationBranch {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub region_id: Option<Uuid>,
    pub name: String,
    pub branch_code: String,
    pub address: Option<String>,
    pub local_policies: serde_json::Value,
    pub wallet_address: Option<String>,
    pub settlement_mode: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct OrganizationRole {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub name: String,
    pub permissions: serde_json::Value,
    pub scope: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct OrganizationMember {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub user_id: Uuid,
    pub role_id: Uuid,
    pub branch_id: Option<Uuid>,
    pub is_active: bool,
    pub invited_at: DateTime<Utc>,
    pub accepted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct RevenueSnapshot {
    pub id: Uuid,
    pub organization_id: Uuid,
    pub branch_id: Option<Uuid>,
    pub snapshot_date: NaiveDate,
    pub total_revenue: sqlx::types::BigDecimal,
    pub transaction_count: i32,
    pub avg_transaction_value: Option<sqlx::types::BigDecimal>,
    pub currency: String,
    pub created_at: DateTime<Utc>,
}

// ---------------------------------------------------------------------------
// Request / Response DTOs
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct CreateOrganizationRequest {
    pub name: String,
    pub slug: String,
    pub logo_url: Option<String>,
    pub settlement_mode: Option<String>,
    pub centralized_bank_account_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateBranchRequest {
    pub name: String,
    pub branch_code: String,
    pub region_id: Option<Uuid>,
    pub address: Option<String>,
    pub wallet_address: Option<String>,
    pub settlement_mode: Option<String>,
    pub local_policies: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct CreateRegionRequest {
    pub name: String,
    pub code: String,
}

#[derive(Debug, Deserialize)]
pub struct AddMemberRequest {
    pub user_id: Uuid,
    pub role_id: Uuid,
    /// None = org-wide; Some = scoped to branch
    pub branch_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSettlementRequest {
    pub settlement_mode: String,
    pub centralized_bank_account_id: Option<String>,
}

/// Aggregated cross-store revenue response.
#[derive(Debug, Serialize)]
pub struct CrossStoreRevenueReport {
    pub organization_id: Uuid,
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
    pub total_revenue: String,
    pub total_transactions: i64,
    pub branches: Vec<BranchRevenueSummary>,
}

#[derive(Debug, Serialize)]
pub struct BranchRevenueSummary {
    pub branch_id: Option<Uuid>,
    pub branch_name: Option<String>,
    pub total_revenue: String,
    pub transaction_count: i64,
}

#[derive(Debug, Deserialize)]
pub struct RevenueReportQuery {
    pub period_start: NaiveDate,
    pub period_end: NaiveDate,
    pub branch_id: Option<Uuid>,
}
