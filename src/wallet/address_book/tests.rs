#[cfg(test)]
mod tests {
    use super::super::*;
    use uuid::Uuid;

    #[test]
    fn test_address_entry_type_display() {
        assert_eq!(AddressEntryType::StellarWallet.to_string(), "stellar-wallet");
        assert_eq!(AddressEntryType::MobileMoney.to_string(), "mobile-money");
        assert_eq!(AddressEntryType::BankAccount.to_string(), "bank-account");
    }

    #[test]
    fn test_entry_status_variants() {
        let active = EntryStatus::Active;
        let deleted = EntryStatus::Deleted;
        
        assert_ne!(active, deleted);
    }

    #[test]
    fn test_verification_status_variants() {
        let verified = VerificationStatus::Verified;
        let pending = VerificationStatus::Pending;
        let failed = VerificationStatus::Failed;
        let stale = VerificationStatus::Stale;
        let not_supported = VerificationStatus::NotSupported;

        assert_ne!(verified, pending);
        assert_ne!(verified, failed);
        assert_ne!(verified, stale);
        assert_ne!(verified, not_supported);
    }

    #[test]
    fn test_create_stellar_wallet_request_deserialization() {
        let json = r#"{
            "entry_type": "stellar-wallet",
            "label": "My Wallet",
            "notes": "Test wallet",
            "stellar_public_key": "GBRPYHIL2CI3FNQ4BXLFMNDLFJUNPU2HY3ZMFSHONUCEOASW7QC7OX2H",
            "network": "testnet"
        }"#;

        let request: CreateAddressBookEntryRequest = serde_json::from_str(json).unwrap();
        
        match request {
            CreateAddressBookEntryRequest::StellarWallet { label, stellar_public_key, network, .. } => {
                assert_eq!(label, "My Wallet");
                assert_eq!(stellar_public_key, "GBRPYHIL2CI3FNQ4BXLFMNDLFJUNPU2HY3ZMFSHONUCEOASW7QC7OX2H");
                assert_eq!(network, "testnet");
            }
            _ => panic!("Expected StellarWallet variant"),
        }
    }

    #[test]
    fn test_create_mobile_money_request_deserialization() {
        let json = r#"{
            "entry_type": "mobile-money",
            "label": "Mom's Phone",
            "notes": null,
            "provider_name": "MTN",
            "phone_number": "+2348012345678",
            "country_code": "NG"
        }"#;

        let request: CreateAddressBookEntryRequest = serde_json::from_str(json).unwrap();
        
        match request {
            CreateAddressBookEntryRequest::MobileMoney { label, provider_name, phone_number, country_code, .. } => {
                assert_eq!(label, "Mom's Phone");
                assert_eq!(provider_name, "MTN");
                assert_eq!(phone_number, "+2348012345678");
                assert_eq!(country_code, "NG");
            }
            _ => panic!("Expected MobileMoney variant"),
        }
    }

    #[test]
    fn test_create_bank_account_request_deserialization() {
        let json = r#"{
            "entry_type": "bank-account",
            "label": "Savings Account",
            "notes": "Main savings",
            "bank_name": "First Bank",
            "account_number": "0123456789",
            "sort_code": null,
            "routing_number": null,
            "country_code": "NG",
            "currency": "NGN"
        }"#;

        let request: CreateAddressBookEntryRequest = serde_json::from_str(json).unwrap();
        
        match request {
            CreateAddressBookEntryRequest::BankAccount { label, bank_name, account_number, currency, .. } => {
                assert_eq!(label, "Savings Account");
                assert_eq!(bank_name, "First Bank");
                assert_eq!(account_number, "0123456789");
                assert_eq!(currency, "NGN");
            }
            _ => panic!("Expected BankAccount variant"),
        }
    }

    #[test]
    fn test_update_request_deserialization() {
        let json = r#"{
            "label": "Updated Label",
            "notes": "Updated notes"
        }"#;

        let request: UpdateAddressBookEntryRequest = serde_json::from_str(json).unwrap();
        assert_eq!(request.label.unwrap(), "Updated Label");
        assert_eq!(request.notes.unwrap(), "Updated notes");
    }

    #[test]
    fn test_list_query_deserialization() {
        let json = r#"{
            "entry_type": "stellar-wallet",
            "search": "test",
            "limit": 50,
            "offset": 0
        }"#;

        let query: ListAddressBookEntriesQuery = serde_json::from_str(json).unwrap();
        assert_eq!(query.entry_type.unwrap(), AddressEntryType::StellarWallet);
        assert_eq!(query.search.unwrap(), "test");
        assert_eq!(query.limit.unwrap(), 50);
        assert_eq!(query.offset.unwrap(), 0);
    }

    #[test]
    fn test_verification_result_serialization() {
        let result = VerificationResult {
            success: true,
            verification_status: VerificationStatus::Verified,
            message: Some("Account verified".to_string()),
            verified_account_name: Some("John Doe".to_string()),
            warnings: vec!["Warning 1".to_string()],
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("\"verification_status\":\"verified\""));
    }

    #[test]
    fn test_import_result_serialization() {
        let result = ImportResult {
            total_rows: 10,
            successful: 8,
            failed: 2,
            results: vec![
                ImportRowResult {
                    row_number: 1,
                    success: true,
                    entry_id: Some(Uuid::new_v4()),
                    error: None,
                },
                ImportRowResult {
                    row_number: 2,
                    success: false,
                    entry_id: None,
                    error: Some("Invalid format".to_string()),
                },
            ],
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"total_rows\":10"));
        assert!(json.contains("\"successful\":8"));
        assert!(json.contains("\"failed\":2"));
    }
}
