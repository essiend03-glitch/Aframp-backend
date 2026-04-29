use crate::wallet::address_book::{
    AddressBookRepository, AddressEntryType, StellarAddressVerifier,
    VerificationStatus,
};
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info, warn};

pub struct AddressBookMaintenanceWorker {
    pool: PgPool,
    repository: Arc<AddressBookRepository>,
    stellar_verifier: Arc<StellarAddressVerifier>,
    stale_threshold_hours: i64,
    stale_alert_threshold: i64,
    re_verification_batch_size: usize,
}

impl AddressBookMaintenanceWorker {
    pub fn new(
        pool: PgPool,
        horizon_url: String,
        cngn_issuer: String,
        stale_threshold_hours: i64,
        stale_alert_threshold: i64,
    ) -> Self {
        let repository = Arc::new(AddressBookRepository::new(pool.clone()));
        let stellar_verifier = Arc::new(StellarAddressVerifier::new(horizon_url, cngn_issuer));

        Self {
            pool,
            repository,
            stellar_verifier,
            stale_threshold_hours,
            stale_alert_threshold,
            re_verification_batch_size: 100,
        }
    }

    /// Start the maintenance worker
    pub async fn run(&self) {
        info!("Starting address book maintenance worker");

        loop {
            // Run cleanup task
            if let Err(e) = self.cleanup_deleted_entries().await {
                error!("Error during cleanup task: {}", e);
            }

            // Run re-verification task
            if let Err(e) = self.re_verify_stale_entries().await {
                error!("Error during re-verification task: {}", e);
            }

            // Check stale verification count and alert if needed
            if let Err(e) = self.check_stale_verification_alert().await {
                error!("Error checking stale verifications: {}", e);
            }

            // Sleep for 1 hour before next run
            sleep(Duration::from_secs(3600)).await;
        }
    }

    /// Cleanup soft-deleted entries older than 30 days
    async fn cleanup_deleted_entries(&self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Running cleanup of deleted address book entries");

        let result: (i32,) = sqlx::query_as("SELECT cleanup_deleted_address_book_entries()")
            .fetch_one(&self.pool)
            .await?;

        let deleted_count = result.0;

        if deleted_count > 0 {
            info!("Permanently deleted {} address book entries", deleted_count);
        }

        Ok(())
    }

    /// Re-verify stale entries
    async fn re_verify_stale_entries(&self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Running re-verification of stale address book entries");

        // Get stale entries
        let stale_entries = self
            .repository
            .get_stale_entries(self.stale_threshold_hours)
            .await?;

        if stale_entries.is_empty() {
            info!("No stale entries to re-verify");
            return Ok(());
        }

        info!("Found {} stale entries to re-verify", stale_entries.len());

        let mut verified_count = 0;
        let mut failed_count = 0;

        for entry in stale_entries.iter().take(self.re_verification_batch_size) {
            match entry.entry_type {
                AddressEntryType::StellarWallet => {
                    if let Some(stellar) = self.repository.get_stellar_wallet_entry(entry.id).await? {
                        match self
                            .stellar_verifier
                            .verify_account(&stellar.stellar_public_key)
                            .await
                        {
                            Ok(result) => {
                                if result.success {
                                    let account_exists = result.verification_status == VerificationStatus::Verified
                                        || result.verification_status == VerificationStatus::Pending;
                                    let trustline_active = result.verification_status == VerificationStatus::Verified;

                                    self.repository
                                        .update_stellar_verification(entry.id, account_exists, trustline_active)
                                        .await?;

                                    self.repository
                                        .update_verification_status(entry.id, result.verification_status)
                                        .await?;

                                    verified_count += 1;
                                } else {
                                    self.repository
                                        .update_verification_status(entry.id, VerificationStatus::Failed)
                                        .await?;
                                    failed_count += 1;
                                }
                            }
                            Err(e) => {
                                warn!("Failed to verify entry {}: {}", entry.id, e);
                                failed_count += 1;
                            }
                        }
                    }
                }
                AddressEntryType::MobileMoney | AddressEntryType::BankAccount => {
                    // Mobile money and bank account re-verification would require
                    // integration with provider APIs - for now, just mark as stale
                    self.repository
                        .update_verification_status(entry.id, VerificationStatus::Stale)
                        .await?;
                }
            }
        }

        info!(
            "Re-verification complete: {} verified, {} failed",
            verified_count, failed_count
        );

        Ok(())
    }

    /// Check stale verification count and alert if threshold exceeded
    async fn check_stale_verification_alert(&self) -> Result<(), Box<dyn std::error::Error>> {
        let stale_count = self
            .repository
            .count_stale_verifications(self.stale_threshold_hours)
            .await?;

        if stale_count > self.stale_alert_threshold {
            warn!(
                "ALERT: Stale verification count ({}) exceeds threshold ({})",
                stale_count, self.stale_alert_threshold
            );
            // TODO: Send alert to monitoring system
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires database
    async fn test_worker_initialization() {
        let pool = PgPool::connect("postgres://localhost/test").await.unwrap();
        let worker = AddressBookMaintenanceWorker::new(
            pool,
            "https://horizon-testnet.stellar.org".to_string(),
            "GXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX".to_string(),
            168, // 7 days
            1000,
        );

        assert_eq!(worker.stale_threshold_hours, 168);
        assert_eq!(worker.stale_alert_threshold, 1000);
    }
}
