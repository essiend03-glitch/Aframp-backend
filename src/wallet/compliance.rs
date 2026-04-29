use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SanctionsMatchType {
    ExactName,
    Identifier,
    Address,
    Country,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SanctionsEntry {
    pub id: Uuid,
    pub name: String,
    pub identifier: Option<String>,
    pub address: Option<String>,
    pub country: Option<String>,
    pub listed_at: DateTime<Utc>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WalletSanctionsCheck {
    pub wallet_id: Uuid,
    pub matched: bool,
    pub match_type: Option<SanctionsMatchType>,
    pub matched_entry_id: Option<Uuid>,
    pub matched_value: Option<String>,
    pub details: Option<String>,
}

pub fn check_wallet_against_sanctions(
    wallet_id: Uuid,
    wallet_name: &str,
    wallet_identifier: Option<&str>,
    wallet_address: Option<&str>,
    wallet_country: Option<&str>,
    sanctions_list: &[SanctionsEntry],
) -> WalletSanctionsCheck {
    let normalized_name = wallet_name.trim().to_ascii_lowercase();
    let identifier = wallet_identifier.map(str::trim).filter(|v| !v.is_empty());
    let address = wallet_address.map(str::trim).filter(|v| !v.is_empty());
    let country = wallet_country.map(str::trim).filter(|v| !v.is_empty());

    for entry in sanctions_list {
        if entry.name.trim().to_ascii_lowercase() == normalized_name {
            return WalletSanctionsCheck {
                wallet_id,
                matched: true,
                match_type: Some(SanctionsMatchType::ExactName),
                matched_entry_id: Some(entry.id),
                matched_value: Some(entry.name.clone()),
                details: entry.reason.clone(),
            };
        }

        if let (Some(entry_id), Some(wallet_id_value)) = (entry.identifier.as_deref(), identifier) {
            if entry_id.eq_ignore_ascii_case(wallet_id_value) {
                return WalletSanctionsCheck {
                    wallet_id,
                    matched: true,
                    match_type: Some(SanctionsMatchType::Identifier),
                    matched_entry_id: Some(entry.id),
                    matched_value: Some(entry_id.to_string()),
                    details: entry.reason.clone(),
                };
            }
        }

        if let (Some(entry_address), Some(wallet_address_value)) = (entry.address.as_deref(), address) {
            if entry_address.eq_ignore_ascii_case(wallet_address_value) {
                return WalletSanctionsCheck {
                    wallet_id,
                    matched: true,
                    match_type: Some(SanctionsMatchType::Address),
                    matched_entry_id: Some(entry.id),
                    matched_value: Some(entry_address.to_string()),
                    details: entry.reason.clone(),
                };
            }
        }

        if let (Some(entry_country), Some(wallet_country_value)) = (entry.country.as_deref(), country) {
            if entry_country.eq_ignore_ascii_case(wallet_country_value) {
                return WalletSanctionsCheck {
                    wallet_id,
                    matched: true,
                    match_type: Some(SanctionsMatchType::Country),
                    matched_entry_id: Some(entry.id),
                    matched_value: Some(entry_country.to_string()),
                    details: entry.reason.clone(),
                };
            }
        }
    }

    WalletSanctionsCheck {
        wallet_id,
        matched: false,
        match_type: None,
        matched_entry_id: None,
        matched_value: None,
        details: None,
    }
}
