use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

/// Address book entry types
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "text")]
#[serde(rename_all = "kebab-case")]
pub enum AddressEntryType {
    #[sqlx(rename = "stellar-wallet")]
    StellarWallet,
    #[sqlx(rename = "mobile-money")]
    MobileMoney,
    #[sqlx(rename = "bank-account")]
    BankAccount,
}

impl std::fmt::Display for AddressEntryType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AddressEntryType::StellarWallet => write!(f, "stellar-wallet"),
            AddressEntryType::MobileMoney => write!(f, "mobile-money"),
            AddressEntryType::BankAccount => write!(f, "bank-account"),
        }
    }
}

/// Entry status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "text")]
pub enum EntryStatus {
    #[sqlx(rename = "active")]
    Active,
    #[sqlx(rename = "deleted")]
    Deleted,
}

/// Verification status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "text")]
pub enum VerificationStatus {
    #[sqlx(rename = "verified")]
    Verified,
    #[sqlx(rename = "pending")]
    Pending,
    #[sqlx(rename = "failed")]
    Failed,
    #[sqlx(rename = "stale")]
    Stale,
    #[sqlx(rename = "not-supported")]
    NotSupported,
}

/// Main address book entry record
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct AddressBookEntry {
    pub id: Uuid,
    pub owner_wallet_id: Uuid,
    pub entry_type: AddressEntryType,
    pub label: String,
    pub notes: Option<String>,
    pub entry_status: EntryStatus,
    pub verification_status: VerificationStatus,
    pub last_used_at: Option<DateTime<Utc>>,
    pub use_count: i32,
    pub deleted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Stellar wallet entry details
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct StellarWalletEntry {
    pub entry_id: Uuid,
    pub stellar_public_key: String,
    pub network: String,
    pub account_exists_on_stellar: bool,
    pub cngn_trustline_active: bool,
    pub last_verified_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Mobile money entry details
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct MobileMoneyEntry {
    pub entry_id: Uuid,
    pub provider_name: String,
    pub phone_number: String,
    pub account_name: Option<String>,
    pub country_code: String,
    pub last_verified_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Bank account entry details
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct BankAccountEntry {
    pub entry_id: Uuid,
    pub bank_name: String,
    pub account_number: String,
    pub account_name: Option<String>,
    pub sort_code: Option<String>,
    pub routing_number: Option<String>,
    pub country_code: String,
    pub currency: String,
    pub last_verified_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Address group record
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct AddressGroup {
    pub id: Uuid,
    pub owner_wallet_id: Uuid,
    pub group_name: String,
    pub group_description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Group membership record
#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct GroupMembership {
    pub group_id: Uuid,
    pub entry_id: Uuid,
    pub added_at: DateTime<Utc>,
}

// Request/Response DTOs

#[derive(Debug, Deserialize)]
#[serde(tag = "entry_type", rename_all = "kebab-case")]
pub enum CreateAddressBookEntryRequest {
    #[serde(rename = "stellar-wallet")]
    StellarWallet {
        label: String,
        notes: Option<String>,
        stellar_public_key: String,
        network: String,
    },
    #[serde(rename = "mobile-money")]
    MobileMoney {
        label: String,
        notes: Option<String>,
        provider_name: String,
        phone_number: String,
        country_code: String,
    },
    #[serde(rename = "bank-account")]
    BankAccount {
        label: String,
        notes: Option<String>,
        bank_name: String,
        account_number: String,
        sort_code: Option<String>,
        routing_number: Option<String>,
        country_code: String,
        currency: String,
    },
}

#[derive(Debug, Serialize)]
pub struct AddressBookEntryResponse {
    pub id: Uuid,
    pub entry_type: AddressEntryType,
    pub label: String,
    pub notes: Option<String>,
    pub verification_status: VerificationStatus,
    pub last_used_at: Option<DateTime<Utc>>,
    pub use_count: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(flatten)]
    pub details: EntryDetails,
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum EntryDetails {
    StellarWallet {
        stellar_public_key: String,
        network: String,
        account_exists_on_stellar: bool,
        cngn_trustline_active: bool,
        last_verified_at: Option<DateTime<Utc>>,
    },
    MobileMoney {
        provider_name: String,
        phone_number: String,
        account_name: Option<String>,
        country_code: String,
        last_verified_at: Option<DateTime<Utc>>,
    },
    BankAccount {
        bank_name: String,
        account_number: String,
        account_name: Option<String>,
        sort_code: Option<String>,
        routing_number: Option<String>,
        country_code: String,
        currency: String,
        last_verified_at: Option<DateTime<Utc>>,
    },
}

#[derive(Debug, Deserialize)]
pub struct UpdateAddressBookEntryRequest {
    pub label: Option<String>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListAddressBookEntriesQuery {
    pub entry_type: Option<AddressEntryType>,
    pub group_id: Option<Uuid>,
    pub search: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct AddressBookEntriesListResponse {
    pub entries: Vec<AddressBookEntryResponse>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Serialize)]
pub struct VerificationResult {
    pub success: bool,
    pub verification_status: VerificationStatus,
    pub message: Option<String>,
    pub verified_account_name: Option<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateAddressGroupRequest {
    pub group_name: String,
    pub group_description: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AddressGroupResponse {
    pub id: Uuid,
    pub group_name: String,
    pub group_description: Option<String>,
    pub member_count: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateAddressGroupRequest {
    pub group_name: Option<String>,
    pub group_description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddGroupMembersRequest {
    pub entry_ids: Vec<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct ImportEntriesRequest {
    pub csv_data: String,
}

#[derive(Debug, Serialize)]
pub struct ImportResult {
    pub total_rows: usize,
    pub successful: usize,
    pub failed: usize,
    pub results: Vec<ImportRowResult>,
}

#[derive(Debug, Serialize)]
pub struct ImportRowResult {
    pub row_number: usize,
    pub success: bool,
    pub entry_id: Option<Uuid>,
    pub error: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SearchEntriesQuery {
    pub query: String,
    pub entry_type: Option<AddressEntryType>,
    pub group_id: Option<Uuid>,
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct GetSuggestionsQuery {
    pub transaction_type: String,
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct SuggestionResponse {
    pub entries: Vec<AddressBookEntryResponse>,
    pub reason: String,
}
