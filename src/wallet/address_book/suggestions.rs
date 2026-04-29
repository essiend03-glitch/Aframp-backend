use super::models::*;
use super::repository::AddressBookRepository;
use std::sync::Arc;
use uuid::Uuid;

pub struct SuggestionService {
    repository: Arc<AddressBookRepository>,
}

impl SuggestionService {
    pub fn new(repository: Arc<AddressBookRepository>) -> Self {
        Self { repository }
    }

    /// Get suggested entries based on transaction type and usage patterns
    pub async fn get_suggestions(
        &self,
        owner_wallet_id: Uuid,
        transaction_type: &str,
        limit: i64,
    ) -> Result<(Vec<AddressBookEntry>, String), sqlx::Error> {
        let (entry_type, reason) = match transaction_type {
            "cngn-transfer" | "stellar-payment" => {
                (Some(AddressEntryType::StellarWallet), "Recently and frequently used Stellar wallets")
            }
            "mobile-money-offramp" => {
                (Some(AddressEntryType::MobileMoney), "Recently and frequently used mobile money accounts")
            }
            "bank-offramp" => {
                (Some(AddressEntryType::BankAccount), "Recently and frequently used bank accounts")
            }
            _ => (None, "Recently and frequently used entries"),
        };

        let entries = self
            .repository
            .get_suggested_entries(owner_wallet_id, entry_type, limit)
            .await?;

        Ok((entries, reason.to_string()))
    }
}
