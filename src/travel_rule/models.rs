use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

/// Status of a Travel Rule exchange
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "travel_rule_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum TravelRuleStatus {
    /// Awaiting counterparty VASP acknowledgement
    Pending,
    /// Counterparty acknowledged receipt of PII
    Acknowledged,
    /// Exchange failed — manual compliance review required
    Failed,
    /// Destination is an unhosted/unknown wallet — manual review
    ManualReview,
    /// Exchange timed out
    TimedOut,
    /// Completed successfully
    Completed,
}

/// Protocol used for VASP-to-VASP communication
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "travel_rule_protocol", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum TravelRuleProtocol {
    Trisa,
    Trust,
    OpenVasp,
    Ivms101Direct,
    Unknown,
}

/// IVMS101-compliant natural person identity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ivms101NaturalPerson {
    pub first_name: String,
    pub last_name: String,
    pub date_of_birth: Option<String>,
    pub national_id: Option<String>,
    pub address: Option<String>,
    pub country_of_residence: Option<String>,
}

/// IVMS101-compliant legal person identity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ivms101LegalPerson {
    pub legal_name: String,
    pub registration_number: Option<String>,
    pub country_of_registration: Option<String>,
    pub address: Option<String>,
}

/// IVMS101 person — either natural or legal
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Ivms101Person {
    Natural(Ivms101NaturalPerson),
    Legal(Ivms101LegalPerson),
}

/// Travel Rule exchange record
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct TravelRuleExchange {
    pub exchange_id: Uuid,
    pub transaction_id: String,
    pub originator_vasp_id: String,
    pub beneficiary_vasp_id: String,
    pub protocol_used: TravelRuleProtocol,
    pub status: TravelRuleStatus,
    /// IVMS101 originator PII — encrypted at rest
    pub originator_ivms101: Value,
    /// IVMS101 beneficiary PII — encrypted at rest
    pub beneficiary_ivms101: Value,
    pub transfer_amount: String,
    pub asset_code: String,
    pub handshake_initiated_at: DateTime<Utc>,
    pub acknowledged_at: Option<DateTime<Utc>>,
    pub timeout_at: DateTime<Utc>,
    pub failure_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// VASP registry entry for counterparty due diligence
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct VaspRegistryEntry {
    pub vasp_id: String,
    pub vasp_name: String,
    pub supported_protocols: Vec<String>,
    pub travel_rule_endpoint: Option<String>,
    pub is_verified: bool,
    pub jurisdiction: String,
    pub last_verified_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Request to initiate a Travel Rule exchange for an outbound transfer
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitiateTravelRuleRequest {
    pub transaction_id: String,
    pub beneficiary_vasp_id: String,
    pub originator: Ivms101Person,
    pub beneficiary: Ivms101Person,
    pub transfer_amount: String,
    pub asset_code: String,
}

/// Inbound Travel Rule data received from another VASP
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundTravelRuleData {
    pub exchange_id: Uuid,
    pub originator_vasp_id: String,
    pub transaction_id: String,
    pub originator: Ivms101Person,
    pub beneficiary: Ivms101Person,
    pub transfer_amount: String,
    pub asset_code: String,
    pub protocol_used: TravelRuleProtocol,
}
