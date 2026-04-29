use super::models::*;
use super::repository::AddressBookRepository;
use csv::{Reader, Writer};
use std::io::Cursor;
use std::sync::Arc;
use uuid::Uuid;

const MAX_IMPORT_BATCH_SIZE: usize = 1000;

pub struct ImportExportService {
    repository: Arc<AddressBookRepository>,
}

impl ImportExportService {
    pub fn new(repository: Arc<AddressBookRepository>) -> Self {
        Self { repository }
    }

    /// Import address book entries from CSV
    pub async fn import_from_csv(
        &self,
        owner_wallet_id: Uuid,
        csv_data: String,
        max_entries_per_wallet: i64,
    ) -> Result<ImportResult, Box<dyn std::error::Error>> {
        let mut reader = Reader::from_reader(Cursor::new(csv_data));
        let mut results = Vec::new();
        let mut successful = 0;
        let mut failed = 0;
        let mut row_number = 0;

        // Check current entry count
        let current_count = self.repository.count_entries_by_owner(owner_wallet_id).await?;

        for result in reader.records() {
            row_number += 1;

            if row_number > MAX_IMPORT_BATCH_SIZE {
                results.push(ImportRowResult {
                    row_number,
                    success: false,
                    entry_id: None,
                    error: Some(format!("Import batch size limit ({}) exceeded", MAX_IMPORT_BATCH_SIZE)),
                });
                failed += 1;
                break;
            }

            if current_count + successful as i64 >= max_entries_per_wallet {
                results.push(ImportRowResult {
                    row_number,
                    success: false,
                    entry_id: None,
                    error: Some("Maximum entries per wallet limit reached".to_string()),
                });
                failed += 1;
                continue;
            }

            match result {
                Ok(record) => {
                    match self.process_import_row(owner_wallet_id, &record).await {
                        Ok(entry_id) => {
                            results.push(ImportRowResult {
                                row_number,
                                success: true,
                                entry_id: Some(entry_id),
                                error: None,
                            });
                            successful += 1;
                        }
                        Err(e) => {
                            results.push(ImportRowResult {
                                row_number,
                                success: false,
                                entry_id: None,
                                error: Some(e.to_string()),
                            });
                            failed += 1;
                        }
                    }
                }
                Err(e) => {
                    results.push(ImportRowResult {
                        row_number,
                        success: false,
                        entry_id: None,
                        error: Some(format!("CSV parse error: {}", e)),
                    });
                    failed += 1;
                }
            }
        }

        Ok(ImportResult {
            total_rows: row_number,
            successful,
            failed,
            results,
        })
    }

    async fn process_import_row(
        &self,
        owner_wallet_id: Uuid,
        record: &csv::StringRecord,
    ) -> Result<Uuid, Box<dyn std::error::Error>> {
        // Expected CSV format:
        // entry_type,label,notes,field1,field2,field3,field4,field5,field6
        
        if record.len() < 3 {
            return Err("Invalid CSV format: insufficient columns".into());
        }

        let entry_type_str = record.get(0).ok_or("Missing entry_type")?;
        let label = record.get(1).ok_or("Missing label")?.to_string();
        let notes = record.get(2).and_then(|s| if s.is_empty() { None } else { Some(s.to_string()) });

        match entry_type_str {
            "stellar-wallet" => {
                let stellar_public_key = record.get(3).ok_or("Missing stellar_public_key")?.to_string();
                let network = record.get(4).ok_or("Missing network")?.to_string();

                let entry = self
                    .repository
                    .create_entry(owner_wallet_id, AddressEntryType::StellarWallet, label, notes)
                    .await?;

                self.repository
                    .create_stellar_wallet_entry(entry.id, stellar_public_key, network)
                    .await?;

                Ok(entry.id)
            }
            "mobile-money" => {
                let provider_name = record.get(3).ok_or("Missing provider_name")?.to_string();
                let phone_number = record.get(4).ok_or("Missing phone_number")?.to_string();
                let country_code = record.get(5).ok_or("Missing country_code")?.to_string();

                let entry = self
                    .repository
                    .create_entry(owner_wallet_id, AddressEntryType::MobileMoney, label, notes)
                    .await?;

                self.repository
                    .create_mobile_money_entry(entry.id, provider_name, phone_number, country_code)
                    .await?;

                Ok(entry.id)
            }
            "bank-account" => {
                let bank_name = record.get(3).ok_or("Missing bank_name")?.to_string();
                let account_number = record.get(4).ok_or("Missing account_number")?.to_string();
                let sort_code = record.get(5).and_then(|s| if s.is_empty() { None } else { Some(s.to_string()) });
                let routing_number = record.get(6).and_then(|s| if s.is_empty() { None } else { Some(s.to_string()) });
                let country_code = record.get(7).ok_or("Missing country_code")?.to_string();
                let currency = record.get(8).ok_or("Missing currency")?.to_string();

                let entry = self
                    .repository
                    .create_entry(owner_wallet_id, AddressEntryType::BankAccount, label, notes)
                    .await?;

                self.repository
                    .create_bank_account_entry(
                        entry.id,
                        bank_name,
                        account_number,
                        sort_code,
                        routing_number,
                        country_code,
                        currency,
                    )
                    .await?;

                Ok(entry.id)
            }
            _ => Err(format!("Unknown entry type: {}", entry_type_str).into()),
        }
    }

    /// Export address book entries to CSV
    pub async fn export_to_csv(
        &self,
        owner_wallet_id: Uuid,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let entries = self
            .repository
            .list_entries(owner_wallet_id, None, None, None, 10000, 0)
            .await?;

        let mut writer = Writer::from_writer(vec![]);

        // Write header
        writer.write_record(&[
            "entry_type",
            "label",
            "notes",
            "field1",
            "field2",
            "field3",
            "field4",
            "field5",
            "field6",
        ])?;

        for entry in entries {
            match entry.entry_type {
                AddressEntryType::StellarWallet => {
                    if let Some(stellar) = self.repository.get_stellar_wallet_entry(entry.id).await? {
                        writer.write_record(&[
                            "stellar-wallet",
                            &entry.label,
                            entry.notes.as_deref().unwrap_or(""),
                            &stellar.stellar_public_key,
                            &stellar.network,
                            "",
                            "",
                            "",
                            "",
                        ])?;
                    }
                }
                AddressEntryType::MobileMoney => {
                    if let Some(mobile) = self.repository.get_mobile_money_entry(entry.id).await? {
                        writer.write_record(&[
                            "mobile-money",
                            &entry.label,
                            entry.notes.as_deref().unwrap_or(""),
                            &mobile.provider_name,
                            &mobile.phone_number,
                            &mobile.country_code,
                            "",
                            "",
                            "",
                        ])?;
                    }
                }
                AddressEntryType::BankAccount => {
                    if let Some(bank) = self.repository.get_bank_account_entry(entry.id).await? {
                        writer.write_record(&[
                            "bank-account",
                            &entry.label,
                            entry.notes.as_deref().unwrap_or(""),
                            &bank.bank_name,
                            &bank.account_number,
                            bank.sort_code.as_deref().unwrap_or(""),
                            bank.routing_number.as_deref().unwrap_or(""),
                            &bank.country_code,
                            &bank.currency,
                        ])?;
                    }
                }
            }
        }

        let csv_data = String::from_utf8(writer.into_inner()?)?;
        Ok(csv_data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_csv_format() {
        let csv_data = "entry_type,label,notes,field1,field2,field3,field4,field5,field6\n\
                        stellar-wallet,My Wallet,Test note,GBRPYHIL2CI3FNQ4BXLFMNDLFJUNPU2HY3ZMFSHONUCEOASW7QC7OX2H,testnet,,,,,\n\
                        mobile-money,Mom's Phone,,MTN,+2348012345678,NG,,,,,\n\
                        bank-account,Savings Account,,First Bank,0123456789,,,NG,NGN,";

        let mut reader = Reader::from_reader(Cursor::new(csv_data));
        let mut count = 0;

        for result in reader.records() {
            assert!(result.is_ok());
            count += 1;
        }

        assert_eq!(count, 3);
    }
}
