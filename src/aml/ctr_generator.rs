//! CTR Auto-Generation Service
//!
//! Automatically generates draft CTRs on threshold breach, pre-populated with
//! subject KYC data and all transactions in the reporting window.
//! Checks for active exemptions before generating CTRs.

use super::ctr_exemption::CtrExemptionService;
use super::ctr_logging;
use super::ctr_metrics;
use super::models::{Ctr, CtrStatus, CtrTransaction, CtrType, DetectionMethod, TransactionDirection};
use chrono::{DateTime, Duration, Utc};
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{error, info, warn};
use uuid::Uuid;

/// Configuration for CTR generation
#[derive(Debug, Clone)]
pub struct CtrGeneratorConfig {
    /// Filing deadline in days from CTR creation
    pub filing_deadline_days: i64,
    /// Default compliance officer to assign (if not specified)
    pub default_compliance_officer: Option<Uuid>,
}

impl Default for CtrGeneratorConfig {
    fn default() -> Self {
        Self {
            filing_deadline_days: 15, // Standard 15-day filing deadline
            default_compliance_officer: None,
        }
    }
}

/// Subject information for CTR generation
#[derive(Debug, Clone)]
pub struct SubjectInfo {
    pub kyc_id: Uuid,
    pub consumer_id: Uuid,
    pub full_name: String,
    pub identification: String,
    pub address: String,
    pub subject_type: CtrType,
}

/// Transaction information for CTR
#[derive(Debug, Clone)]
pub struct TransactionInfo {
    pub transaction_id: Uuid,
    pub transaction_timestamp: DateTime<Utc>,
    pub transaction_type: String,
    pub amount_ngn: Decimal,
    pub counterparty_details: String,
    pub direction: TransactionDirection,
}

/// Result of CTR generation
#[derive(Debug, Clone)]
pub struct CtrGenerationResult {
    pub ctr_id: Uuid,
    pub subject_id: Uuid,
    pub total_amount: Decimal,
    pub transaction_count: i32,
    pub filing_deadline: DateTime<Utc>,
    pub already_existed: bool,
    pub exemption_applied: bool,
}

/// CTR Auto-Generation Service
pub struct CtrGeneratorService {
    pool: PgPool,
    config: CtrGeneratorConfig,
    exemption_service: Option<Arc<CtrExemptionService>>,
}

impl CtrGeneratorService {
    pub fn new(pool: PgPool, config: CtrGeneratorConfig) -> Self {
        Self {
            pool,
            config,
            exemption_service: None,
        }
    }

    /// Create a new service with exemption checking enabled
    pub fn with_exemption_service(
        pool: PgPool,
        config: CtrGeneratorConfig,
        exemption_service: Arc<CtrExemptionService>,
    ) -> Self {
        Self {
            pool,
            config,
            exemption_service: Some(exemption_service),
        }
    }

    /// Auto-generate a draft CTR on threshold breach
    ///
    /// This method:
    /// 1. Checks for active exemptions (if exemption service is configured)
    /// 2. Checks for existing CTR to prevent duplicates
    /// 3. Fetches subject KYC data
    /// 4. Fetches all transactions in the reporting window
    /// 5. Creates a draft CTR with all data pre-populated
    /// 6. Creates CTR transaction records
    /// 7. Notifies the assigned compliance officer
    ///
    /// # Arguments
    /// * `subject_id` - The KYC ID of the subject
    /// * `reporting_window_start` - Start of the reporting window
    /// * `reporting_window_end` - End of the reporting window
    /// * `total_amount` - Total transaction amount in the window
    /// * `transaction_count` - Number of transactions in the window
    /// * `assigned_officer` - Optional specific compliance officer to assign
    pub async fn generate_ctr_on_breach(
        &self,
        subject_id: Uuid,
        reporting_window_start: DateTime<Utc>,
        reporting_window_end: DateTime<Utc>,
        total_amount: Decimal,
        transaction_count: i32,
        assigned_officer: Option<Uuid>,
    ) -> Result<CtrGenerationResult, anyhow::Error> {
        info!(
            subject_id = %subject_id,
            window_start = %reporting_window_start,
            window_end = %reporting_window_end,
            total_amount = %total_amount,
            "Starting CTR auto-generation on threshold breach"
        );

        // Check for active exemption
        if let Some(exemption_service) = &self.exemption_service {
            let exemption_check = exemption_service.check_exemption(subject_id).await?;

            if exemption_check.is_exempt {
                info!(
                    subject_id = %subject_id,
                    exemption_category = ?exemption_check.exemption.as_ref().map(|e| &e.exemption_category),
                    "Subject is exempt from CTR reporting, skipping CTR generation"
                );

                // Record exemption metrics
                if let Some(exemption) = &exemption_check.exemption {
                    ctr_metrics::record_exemption_applied(&exemption.exemption_category);
                    ctr_logging::log_exemption_applied(
                        Uuid::nil(),
                        subject_id,
                        exemption.exemption_category.clone(),
                    );
                }

                // Return a result indicating exemption was applied
                return Ok(CtrGenerationResult {
                    ctr_id: Uuid::nil(), // No CTR created
                    subject_id,
                    total_amount,
                    transaction_count,
                    filing_deadline: Utc::now(),
                    already_existed: false,
                    exemption_applied: true,
                });
            }
        }

        // Check for existing CTR to prevent duplicates
        if let Some(existing_ctr) = self
            .check_existing_ctr(subject_id, reporting_window_start, reporting_window_end)
            .await?
        {
            info!(
                ctr_id = %existing_ctr.ctr_id,
                subject_id = %subject_id,
                "CTR already exists for this subject and reporting window"
            );
            return Ok(CtrGenerationResult {
                ctr_id: existing_ctr.ctr_id,
                subject_id,
                total_amount: existing_ctr.total_transaction_amount,
                transaction_count: existing_ctr.transaction_count,
                filing_deadline: existing_ctr.filing_timestamp.unwrap_or_else(|| {
                    Utc::now() + Duration::days(self.config.filing_deadline_days)
                }),
                already_existed: true,
                exemption_applied: false,
            });
        }

        // Fetch subject information
        let subject_info = self.fetch_subject_info(subject_id).await?;

        // Fetch all transactions in the reporting window
        let transactions = self
            .fetch_transactions_in_window(
                subject_info.consumer_id,
                reporting_window_start,
                reporting_window_end,
            )
            .await?;

        // Generate transaction references
        let transaction_references: Vec<String> = transactions
            .iter()
            .map(|t| t.transaction_id.to_string())
            .collect();

        // Calculate filing deadline
        let filing_deadline = Utc::now() + Duration::days(self.config.filing_deadline_days);

        // Determine assigned compliance officer
        let compliance_officer = assigned_officer.or(self.config.default_compliance_officer);

        // Create the CTR
        let ctr_id = Uuid::new_v4();
        let ctr = Ctr {
            ctr_id,
            reporting_period: reporting_window_start,
            ctr_type: subject_info.subject_type.clone(),
            subject_kyc_id: subject_info.kyc_id,
            subject_full_name: subject_info.full_name.clone(),
            subject_identification: subject_info.identification.clone(),
            subject_address: subject_info.address.clone(),
            total_transaction_amount: total_amount,
            transaction_count,
            transaction_references: transaction_references.clone(),
            detection_method: DetectionMethod::Automatic,
            status: CtrStatus::Draft,
            assigned_compliance_officer: compliance_officer,
            filing_timestamp: Some(filing_deadline),
            regulatory_reference_number: None,
        };

        // Insert CTR into database
        self.insert_ctr(&ctr).await?;

        info!(
            ctr_id = %ctr_id,
            subject_id = %subject_id,
            subject_name = %subject_info.full_name,
            transaction_count = transaction_count,
            total_amount = %total_amount,
            filing_deadline = %filing_deadline,
            "Draft CTR created successfully"
        );

        // Record metrics
        let ctr_type_str = match subject_info.subject_type {
            CtrType::Individual => "individual",
            CtrType::Corporate => "corporate",
        };
        ctr_metrics::record_ctr_generated(ctr_type_str, "automatic");

        // Log structured event
        ctr_logging::log_ctr_generated(
            ctr_id,
            subject_id,
            subject_info.full_name.clone(),
            total_amount.to_string(),
            transaction_count,
            "automatic".to_string(),
        );

        // Create CTR transaction records
        for transaction in &transactions {
            self.insert_ctr_transaction(ctr_id, transaction).await?;
        }

        info!(
            ctr_id = %ctr_id,
            transaction_records_created = transactions.len(),
            "CTR transaction records created"
        );

        // Notify compliance officer
        if let Some(officer_id) = compliance_officer {
            self.notify_compliance_officer(officer_id, &ctr, &subject_info)
                .await?;
        } else {
            warn!(
                ctr_id = %ctr_id,
                "No compliance officer assigned, notification skipped"
            );
        }

        Ok(CtrGenerationResult {
            ctr_id,
            subject_id,
            total_amount,
            transaction_count,
            filing_deadline,
            already_existed: false,
            exemption_applied: false,
        })
    }

    /// Check if a CTR already exists for the subject and reporting window
    async fn check_existing_ctr(
        &self,
        subject_id: Uuid,
        window_start: DateTime<Utc>,
        window_end: DateTime<Utc>,
    ) -> Result<Option<Ctr>, anyhow::Error> {
        let ctr = sqlx::query_as::<_, Ctr>(
            r#"
            SELECT ctr_id, reporting_period, ctr_type, subject_kyc_id, subject_full_name,
                   subject_identification, subject_address, total_transaction_amount,
                   transaction_count, transaction_references, detection_method, status,
                   assigned_compliance_officer, filing_timestamp, regulatory_reference_number
            FROM ctrs
            WHERE subject_kyc_id = $1
              AND reporting_period >= $2
              AND reporting_period <= $3
            ORDER BY reporting_period DESC
            LIMIT 1
            "#,
        )
        .bind(subject_id)
        .bind(window_start)
        .bind(window_end)
        .fetch_optional(&self.pool)
        .await?;

        Ok(ctr)
    }

    /// Fetch subject information from KYC records and consumers table
    async fn fetch_subject_info(&self, kyc_id: Uuid) -> Result<SubjectInfo, anyhow::Error> {
        // Fetch KYC record
        let kyc_record = sqlx::query!(
            r#"
            SELECT id, consumer_id, tier, status
            FROM kyc_records
            WHERE id = $1
            "#,
            kyc_id
        )
        .fetch_one(&self.pool)
        .await?;

        // Fetch consumer information
        let consumer = sqlx::query!(
            r#"
            SELECT id, name, consumer_type
            FROM consumers
            WHERE id = $1
            "#,
            kyc_record.consumer_id
        )
        .fetch_one(&self.pool)
        .await?;

        // Determine subject type based on consumer_type
        let subject_type = if consumer.consumer_type.to_lowercase().contains("corporate")
            || consumer.consumer_type.to_lowercase().contains("business")
        {
            CtrType::Corporate
        } else {
            CtrType::Individual
        };

        // For now, use placeholder data for identification and address
        // In production, these should come from KYC documents or a separate profile table
        let identification = format!("KYC-{}", kyc_id);
        let address = "Address on file".to_string();

        Ok(SubjectInfo {
            kyc_id,
            consumer_id: kyc_record.consumer_id,
            full_name: consumer.name,
            identification,
            address,
            subject_type,
        })
    }

    /// Fetch all transactions for a consumer in the reporting window
    async fn fetch_transactions_in_window(
        &self,
        consumer_id: Uuid,
        window_start: DateTime<Utc>,
        window_end: DateTime<Utc>,
    ) -> Result<Vec<TransactionInfo>, anyhow::Error> {
        // First, get wallet addresses for this consumer
        let wallet_addresses = sqlx::query!(
            r#"
            SELECT wallet_address
            FROM wallets
            WHERE consumer_id = $1
            "#,
            consumer_id
        )
        .fetch_all(&self.pool)
        .await?;

        if wallet_addresses.is_empty() {
            warn!(
                consumer_id = %consumer_id,
                "No wallet addresses found for consumer"
            );
            return Ok(Vec::new());
        }

        let addresses: Vec<String> = wallet_addresses
            .into_iter()
            .map(|r| r.wallet_address)
            .collect();

        // Fetch transactions for these wallet addresses in the window
        let transactions = sqlx::query!(
            r#"
            SELECT transaction_id, type, cngn_amount, created_at, from_currency, to_currency,
                   payment_provider, status
            FROM transactions
            WHERE wallet_address = ANY($1)
              AND created_at >= $2
              AND created_at <= $3
              AND status IN ('completed', 'confirmed', 'success')
            ORDER BY created_at ASC
            "#,
            &addresses,
            window_start,
            window_end
        )
        .fetch_all(&self.pool)
        .await?;

        let transaction_infos: Vec<TransactionInfo> = transactions
            .into_iter()
            .map(|t| {
                // Determine direction based on transaction type
                let direction = if t.r#type == "onramp" || t.r#type == "deposit" {
                    TransactionDirection::Credit
                } else {
                    TransactionDirection::Debit
                };

                // Build counterparty details
                let counterparty = format!(
                    "Provider: {}, From: {}, To: {}",
                    t.payment_provider.unwrap_or_else(|| "N/A".to_string()),
                    t.from_currency,
                    t.to_currency
                );

                TransactionInfo {
                    transaction_id: t.transaction_id,
                    transaction_timestamp: t.created_at,
                    transaction_type: t.r#type,
                    amount_ngn: t.cngn_amount,
                    counterparty_details: counterparty,
                    direction,
                }
            })
            .collect();

        Ok(transaction_infos)
    }

    /// Insert CTR into database
    async fn insert_ctr(&self, ctr: &Ctr) -> Result<(), anyhow::Error> {
        sqlx::query(
            r#"
            INSERT INTO ctrs
                (ctr_id, reporting_period, ctr_type, subject_kyc_id, subject_full_name,
                 subject_identification, subject_address, total_transaction_amount,
                 transaction_count, transaction_references, detection_method, status,
                 assigned_compliance_officer, filing_timestamp, regulatory_reference_number)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15)
            "#,
        )
        .bind(ctr.ctr_id)
        .bind(ctr.reporting_period)
        .bind(&ctr.ctr_type)
        .bind(ctr.subject_kyc_id)
        .bind(&ctr.subject_full_name)
        .bind(&ctr.subject_identification)
        .bind(&ctr.subject_address)
        .bind(ctr.total_transaction_amount)
        .bind(ctr.transaction_count)
        .bind(&ctr.transaction_references)
        .bind(&ctr.detection_method)
        .bind(&ctr.status)
        .bind(ctr.assigned_compliance_officer)
        .bind(ctr.filing_timestamp)
        .bind(&ctr.regulatory_reference_number)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Insert CTR transaction record
    async fn insert_ctr_transaction(
        &self,
        ctr_id: Uuid,
        transaction: &TransactionInfo,
    ) -> Result<(), anyhow::Error> {
        sqlx::query(
            r#"
            INSERT INTO ctr_transactions
                (ctr_id, transaction_id, transaction_timestamp, transaction_type,
                 transaction_amount_ngn, counterparty_details, direction)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(ctr_id)
        .bind(transaction.transaction_id)
        .bind(transaction.transaction_timestamp)
        .bind(&transaction.transaction_type)
        .bind(transaction.amount_ngn)
        .bind(&transaction.counterparty_details)
        .bind(&transaction.direction)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Notify compliance officer about new CTR
    async fn notify_compliance_officer(
        &self,
        officer_id: Uuid,
        ctr: &Ctr,
        subject_info: &SubjectInfo,
    ) -> Result<(), anyhow::Error> {
        // Insert notification into compliance_notifications table
        sqlx::query(
            r#"
            INSERT INTO compliance_notifications
                (id, officer_id, notification_type, subject_id, subject_name, ctr_id,
                 total_amount, transaction_count, filing_deadline, created_at, read_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(officer_id)
        .bind("ctr_threshold_breach")
        .bind(subject_info.kyc_id)
        .bind(&subject_info.full_name)
        .bind(ctr.ctr_id)
        .bind(ctr.total_transaction_amount)
        .bind(ctr.transaction_count)
        .bind(ctr.filing_timestamp)
        .bind(Utc::now())
        .bind(None::<DateTime<Utc>>)
        .execute(&self.pool)
        .await?;

        info!(
            officer_id = %officer_id,
            ctr_id = %ctr.ctr_id,
            subject_name = %subject_info.full_name,
            "Compliance officer notified of new CTR"
        );

        Ok(())
    }

    /// Get all pending CTRs assigned to a compliance officer
    pub async fn get_pending_ctrs_for_officer(
        &self,
        officer_id: Uuid,
    ) -> Result<Vec<Ctr>, anyhow::Error> {
        let ctrs = sqlx::query_as::<_, Ctr>(
            r#"
            SELECT ctr_id, reporting_period, ctr_type, subject_kyc_id, subject_full_name,
                   subject_identification, subject_address, total_transaction_amount,
                   transaction_count, transaction_references, detection_method, status,
                   assigned_compliance_officer, filing_timestamp, regulatory_reference_number
            FROM ctrs
            WHERE assigned_compliance_officer = $1
              AND status IN ('draft', 'under_review')
            ORDER BY filing_timestamp ASC
            "#,
        )
        .bind(officer_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(ctrs)
    }

    /// Get CTR by ID
    pub async fn get_ctr_by_id(&self, ctr_id: Uuid) -> Result<Option<Ctr>, anyhow::Error> {
        let ctr = sqlx::query_as::<_, Ctr>(
            r#"
            SELECT ctr_id, reporting_period, ctr_type, subject_kyc_id, subject_full_name,
                   subject_identification, subject_address, total_transaction_amount,
                   transaction_count, transaction_references, detection_method, status,
                   assigned_compliance_officer, filing_timestamp, regulatory_reference_number
            FROM ctrs
            WHERE ctr_id = $1
            "#,
        )
        .bind(ctr_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(ctr)
    }

    /// Get all transactions for a CTR
    pub async fn get_ctr_transactions(
        &self,
        ctr_id: Uuid,
    ) -> Result<Vec<CtrTransaction>, anyhow::Error> {
        let transactions = sqlx::query_as::<_, CtrTransaction>(
            r#"
            SELECT ctr_id, transaction_id, transaction_timestamp, transaction_type,
                   transaction_amount_ngn, counterparty_details, direction
            FROM ctr_transactions
            WHERE ctr_id = $1
            ORDER BY transaction_timestamp ASC
            "#,
        )
        .bind(ctr_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(transactions)
    }

    /// Update CTR status
    pub async fn update_ctr_status(
        &self,
        ctr_id: Uuid,
        new_status: CtrStatus,
    ) -> Result<(), anyhow::Error> {
        // Get current status first
        let current_ctr = self.get_ctr_by_id(ctr_id).await?;
        let old_status = current_ctr.as_ref().map(|c| format!("{:?}", c.status));

        sqlx::query(
            r#"
            UPDATE ctrs
            SET status = $2
            WHERE ctr_id = $1
            "#,
        )
        .bind(ctr_id)
        .bind(&new_status)
        .execute(&self.pool)
        .await?;

        info!(
            ctr_id = %ctr_id,
            new_status = ?new_status,
            "CTR status updated"
        );

        // Record metrics and logging
        if let Some(old) = old_status {
            let new = format!("{:?}", new_status);
            ctr_metrics::record_status_change(&old, &new);
            ctr_logging::log_status_change(ctr_id, old, new, None, None);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = CtrGeneratorConfig::default();
        assert_eq!(config.filing_deadline_days, 15);
        assert_eq!(config.default_compliance_officer, None);
    }
}
