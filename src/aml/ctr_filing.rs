//! CTR Filing Service
//!
//! Handles CTR document generation (NFIU-compliant XML and PDF), validation,
//! filing with regulatory authority, and retry logic with exponential backoff.

use super::models::{Ctr, CtrFiling, CtrStatus, CtrTransaction};
use super::ctr_logging;
use super::ctr_metrics;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::time::Duration;
use tracing::{error, info, warn};
use uuid::Uuid;

/// Configuration for CTR filing
#[derive(Debug, Clone)]
pub struct CtrFilingConfig {
    /// NFIU API endpoint
    pub nfiu_api_endpoint: String,
    /// API key for NFIU submission
    pub nfiu_api_key: String,
    /// Maximum retry attempts
    pub max_retry_attempts: u32,
    /// Initial retry delay in seconds
    pub initial_retry_delay_secs: u64,
    /// Maximum retry delay in seconds
    pub max_retry_delay_secs: u64,
    /// Request timeout in seconds
    pub request_timeout_secs: u64,
}

impl Default for CtrFilingConfig {
    fn default() -> Self {
        Self {
            nfiu_api_endpoint: "https://api.nfiu.gov.ng/ctr/submit".to_string(),
            nfiu_api_key: String::new(),
            max_retry_attempts: 5,
            initial_retry_delay_secs: 2,
            max_retry_delay_secs: 300, // 5 minutes
            request_timeout_secs: 30,
        }
    }
}

/// NFIU-compliant CTR XML document
#[derive(Debug, Clone, Serialize)]
pub struct NfiuCtrXml {
    pub ctr_id: String,
    pub reporting_period: String,
    pub subject_type: String,
    pub subject_identification: SubjectIdentification,
    pub transactions: Vec<TransactionXml>,
    pub total_amount: String,
    pub transaction_count: u32,
    pub detection_method: String,
    pub filing_institution: FilingInstitution,
    pub submission_timestamp: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct SubjectIdentification {
    pub full_name: String,
    pub identification_number: String,
    pub address: String,
    pub kyc_id: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TransactionXml {
    pub transaction_id: String,
    pub timestamp: String,
    pub transaction_type: String,
    pub amount_ngn: String,
    pub direction: String,
    pub counterparty: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct FilingInstitution {
    pub name: String,
    pub registration_number: String,
    pub contact_email: String,
}

/// CTR document generation result
#[derive(Debug, Clone, Serialize)]
pub struct CtrDocuments {
    pub ctr_id: Uuid,
    pub xml_content: String,
    pub pdf_url: Option<String>,
    pub generated_at: DateTime<Utc>,
}

/// CTR filing result
#[derive(Debug, Clone, Serialize)]
pub struct FilingResult {
    pub ctr_id: Uuid,
    pub filing_id: Uuid,
    pub submission_reference: String,
    pub submission_timestamp: DateTime<Utc>,
    pub status: FilingStatus,
    pub retry_count: u32,
}

/// Filing status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum FilingStatus {
    Pending,
    Submitted,
    Acknowledged,
    Rejected,
    Failed,
}

/// Validation error
#[derive(Debug, Clone, Serialize)]
pub struct ValidationError {
    pub field: String,
    pub message: String,
}

/// CTR Filing Service
pub struct CtrFilingService {
    pool: PgPool,
    config: CtrFilingConfig,
    http_client: reqwest::Client,
}

impl CtrFilingService {
    pub fn new(pool: PgPool, config: CtrFilingConfig) -> Self {
        let http_client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.request_timeout_secs))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            pool,
            config,
            http_client,
        }
    }

    /// Generate CTR documents (XML and PDF)
    pub async fn generate_documents(
        &self,
        ctr_id: Uuid,
    ) -> Result<CtrDocuments, anyhow::Error> {
        info!(ctr_id = %ctr_id, "Generating CTR documents");

        // Get CTR
        let ctr = self.get_ctr(ctr_id).await?;

        // Verify CTR is approved
        if ctr.status != CtrStatus::Approved {
            return Err(anyhow::anyhow!(
                "CTR must be in Approved status to generate documents. Current status: {:?}",
                ctr.status
            ));
        }

        // Get transactions
        let transactions = self.get_ctr_transactions(ctr_id).await?;

        // Generate NFIU-compliant XML
        let xml_content = self.generate_nfiu_xml(&ctr, &transactions)?;

        // Generate PDF (placeholder - would use actual PDF library)
        let pdf_url = self.generate_pdf(&ctr, &transactions).await?;

        info!(
            ctr_id = %ctr_id,
            xml_size = xml_content.len(),
            "CTR documents generated successfully"
        );

        Ok(CtrDocuments {
            ctr_id,
            xml_content,
            pdf_url: Some(pdf_url),
            generated_at: Utc::now(),
        })
    }

    /// Get CTR document (retrieve previously generated)
    pub async fn get_document(
        &self,
        ctr_id: Uuid,
    ) -> Result<Option<CtrDocuments>, anyhow::Error> {
        // Check if documents exist in storage
        let document = sqlx::query!(
            r#"
            SELECT ctr_id, xml_content, pdf_url, generated_at
            FROM ctr_documents
            WHERE ctr_id = $1
            ORDER BY generated_at DESC
            LIMIT 1
            "#,
            ctr_id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(document.map(|doc| CtrDocuments {
            ctr_id: doc.ctr_id,
            xml_content: doc.xml_content,
            pdf_url: doc.pdf_url,
            generated_at: doc.generated_at,
        }))
    }

    /// File CTR with NFIU
    pub async fn file_ctr(&self, ctr_id: Uuid) -> Result<FilingResult, anyhow::Error> {
        info!(ctr_id = %ctr_id, "Filing CTR with NFIU");

        // Get CTR
        let ctr = self.get_ctr(ctr_id).await?;

        // Verify CTR is approved
        if ctr.status != CtrStatus::Approved {
            return Err(anyhow::anyhow!(
                "CTR must be in Approved status to file. Current status: {:?}",
                ctr.status
            ));
        }

        // Validate CTR before filing
        let validation_errors = self.validate_ctr(&ctr).await?;
        if !validation_errors.is_empty() {
            error!(
                ctr_id = %ctr_id,
                errors = ?validation_errors,
                "CTR validation failed"
            );
            return Err(anyhow::anyhow!(
                "CTR validation failed: {} errors",
                validation_errors.len()
            ));
        }

        // Get or generate documents
        let documents = match self.get_document(ctr_id).await? {
            Some(docs) => docs,
            None => self.generate_documents(ctr_id).await?,
        };

        // Store documents if not already stored
        self.store_documents(&documents).await?;

        // Submit to NFIU with retry logic
        let filing_result = self
            .submit_with_retry(ctr_id, &documents.xml_content)
            .await?;

        // Update CTR status
        if filing_result.status == FilingStatus::Submitted
            || filing_result.status == FilingStatus::Acknowledged
        {
            self.update_ctr_status(ctr_id, CtrStatus::Filed).await?;
        }

        info!(
            ctr_id = %ctr_id,
            submission_reference = %filing_result.submission_reference,
            status = ?filing_result.status,
            "CTR filed successfully"
        );

        // Record metrics
        let ctr_type_str = match ctr.ctr_type {
            super::models::CtrType::Individual => "individual",
            super::models::CtrType::Corporate => "corporate",
        };
        ctr_metrics::record_ctr_filed(ctr_type_str, "electronic");
        ctr_metrics::record_filing_retry_count(
            &format!("{:?}", filing_result.status),
            filing_result.retry_count as f64,
        );

        // Log structured event
        ctr_logging::log_ctr_filed(
            ctr_id,
            filing_result.submission_reference.clone(),
            filing_result.retry_count,
        );

        Ok(filing_result)
    }

    /// Validate CTR before filing
    async fn validate_ctr(&self, ctr: &Ctr) -> Result<Vec<ValidationError>, anyhow::Error> {
        let mut errors = Vec::new();

        // Validate subject information
        if ctr.subject_full_name.trim().is_empty() {
            errors.push(ValidationError {
                field: "subject_full_name".to_string(),
                message: "Subject full name is required".to_string(),
            });
        }

        if ctr.subject_identification.trim().is_empty() {
            errors.push(ValidationError {
                field: "subject_identification".to_string(),
                message: "Subject identification is required".to_string(),
            });
        }

        if ctr.subject_address.trim().is_empty() {
            errors.push(ValidationError {
                field: "subject_address".to_string(),
                message: "Subject address is required".to_string(),
            });
        }

        // Validate transaction data
        if ctr.transaction_count == 0 {
            errors.push(ValidationError {
                field: "transaction_count".to_string(),
                message: "At least one transaction is required".to_string(),
            });
        }

        if ctr.total_transaction_amount <= Decimal::ZERO {
            errors.push(ValidationError {
                field: "total_transaction_amount".to_string(),
                message: "Total transaction amount must be greater than zero".to_string(),
            });
        }

        // Validate filing deadline
        if let Some(deadline) = ctr.filing_timestamp {
            if deadline < Utc::now() {
                warn!(
                    ctr_id = %ctr.ctr_id,
                    deadline = %deadline,
                    "Filing deadline has passed"
                );
            }
        }

        Ok(errors)
    }

    /// Submit CTR to NFIU with retry logic and exponential backoff
    async fn submit_with_retry(
        &self,
        ctr_id: Uuid,
        xml_content: &str,
    ) -> Result<FilingResult, anyhow::Error> {
        let mut retry_count = 0;
        let mut delay_secs = self.config.initial_retry_delay_secs;

        loop {
            match self.submit_to_nfiu(ctr_id, xml_content).await {
                Ok(result) => {
                    // Record successful filing
                    self.record_filing(&result).await?;
                    return Ok(result);
                }
                Err(e) => {
                    retry_count += 1;

                    if retry_count >= self.config.max_retry_attempts {
                        error!(
                            ctr_id = %ctr_id,
                            retry_count = retry_count,
                            error = %e,
                            "Max retry attempts reached, filing failed"
                        );

                        // Record failed filing
                        let failed_result = FilingResult {
                            ctr_id,
                            filing_id: Uuid::new_v4(),
                            submission_reference: format!("FAILED-{}", ctr_id),
                            submission_timestamp: Utc::now(),
                            status: FilingStatus::Failed,
                            retry_count,
                        };
                        self.record_filing(&failed_result).await?;

                        return Err(anyhow::anyhow!(
                            "Failed to file CTR after {} attempts: {}",
                            retry_count,
                            e
                        ));
                    }

                    warn!(
                        ctr_id = %ctr_id,
                        retry_count = retry_count,
                        delay_secs = delay_secs,
                        error = %e,
                        "Filing attempt failed, retrying"
                    );

                    // Wait before retry
                    tokio::time::sleep(Duration::from_secs(delay_secs)).await;

                    // Exponential backoff with cap
                    delay_secs = (delay_secs * 2).min(self.config.max_retry_delay_secs);
                }
            }
        }
    }

    /// Submit CTR to NFIU API
    async fn submit_to_nfiu(
        &self,
        ctr_id: Uuid,
        xml_content: &str,
    ) -> Result<FilingResult, anyhow::Error> {
        // If API key is not configured, simulate submission for testing
        if self.config.nfiu_api_key.is_empty() {
            warn!(
                ctr_id = %ctr_id,
                "NFIU API key not configured, simulating submission"
            );

            return Ok(FilingResult {
                ctr_id,
                filing_id: Uuid::new_v4(),
                submission_reference: format!("SIM-{}", Uuid::new_v4()),
                submission_timestamp: Utc::now(),
                status: FilingStatus::Submitted,
                retry_count: 0,
            });
        }

        // Prepare submission payload
        let payload = serde_json::json!({
            "ctr_id": ctr_id.to_string(),
            "xml_content": xml_content,
            "submission_timestamp": Utc::now().to_rfc3339(),
        });

        // Submit to NFIU
        let response = self
            .http_client
            .post(&self.config.nfiu_api_endpoint)
            .header("Authorization", format!("Bearer {}", self.config.nfiu_api_key))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await?;

        if response.status().is_success() {
            let response_data: NfiuSubmissionResponse = response.json().await?;

            Ok(FilingResult {
                ctr_id,
                filing_id: Uuid::new_v4(),
                submission_reference: response_data.reference_number,
                submission_timestamp: Utc::now(),
                status: FilingStatus::Submitted,
                retry_count: 0,
            })
        } else {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();

            Err(anyhow::anyhow!(
                "NFIU submission failed with status {}: {}",
                status,
                error_text
            ))
        }
    }

    /// Generate NFIU-compliant XML
    fn generate_nfiu_xml(
        &self,
        ctr: &Ctr,
        transactions: &[CtrTransaction],
    ) -> Result<String, anyhow::Error> {
        let nfiu_ctr = NfiuCtrXml {
            ctr_id: ctr.ctr_id.to_string(),
            reporting_period: ctr.reporting_period.to_rfc3339(),
            subject_type: match ctr.ctr_type {
                super::models::CtrType::Individual => "individual".to_string(),
                super::models::CtrType::Corporate => "corporate".to_string(),
            },
            subject_identification: SubjectIdentification {
                full_name: ctr.subject_full_name.clone(),
                identification_number: ctr.subject_identification.clone(),
                address: ctr.subject_address.clone(),
                kyc_id: ctr.subject_kyc_id.to_string(),
            },
            transactions: transactions
                .iter()
                .map(|t| TransactionXml {
                    transaction_id: t.transaction_id.to_string(),
                    timestamp: t.transaction_timestamp.to_rfc3339(),
                    transaction_type: t.transaction_type.clone(),
                    amount_ngn: t.transaction_amount_ngn.to_string(),
                    direction: match t.direction {
                        super::models::TransactionDirection::Credit => "credit".to_string(),
                        super::models::TransactionDirection::Debit => "debit".to_string(),
                    },
                    counterparty: t.counterparty_details.clone(),
                })
                .collect(),
            total_amount: ctr.total_transaction_amount.to_string(),
            transaction_count: ctr.transaction_count as u32,
            detection_method: match ctr.detection_method {
                super::models::DetectionMethod::Automatic => "automatic".to_string(),
                super::models::DetectionMethod::Manual => "manual".to_string(),
            },
            filing_institution: FilingInstitution {
                name: "Bitmesh Financial Services".to_string(),
                registration_number: "RC-123456".to_string(),
                contact_email: "compliance@bitmesh.com".to_string(),
            },
            submission_timestamp: Utc::now().to_rfc3339(),
        };

        // Generate XML (simplified - would use proper XML library in production)
        let xml = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<CTR xmlns="http://nfiu.gov.ng/ctr/v1">
  <CtrId>{}</CtrId>
  <ReportingPeriod>{}</ReportingPeriod>
  <SubjectType>{}</SubjectType>
  <Subject>
    <FullName>{}</FullName>
    <IdentificationNumber>{}</IdentificationNumber>
    <Address>{}</Address>
    <KycId>{}</KycId>
  </Subject>
  <Transactions>
{}
  </Transactions>
  <TotalAmount>{}</TotalAmount>
  <TransactionCount>{}</TransactionCount>
  <DetectionMethod>{}</DetectionMethod>
  <FilingInstitution>
    <Name>{}</Name>
    <RegistrationNumber>{}</RegistrationNumber>
    <ContactEmail>{}</ContactEmail>
  </FilingInstitution>
  <SubmissionTimestamp>{}</SubmissionTimestamp>
</CTR>"#,
            nfiu_ctr.ctr_id,
            nfiu_ctr.reporting_period,
            nfiu_ctr.subject_type,
            escape_xml(&nfiu_ctr.subject_identification.full_name),
            escape_xml(&nfiu_ctr.subject_identification.identification_number),
            escape_xml(&nfiu_ctr.subject_identification.address),
            nfiu_ctr.subject_identification.kyc_id,
            nfiu_ctr
                .transactions
                .iter()
                .map(|t| format!(
                    r#"    <Transaction>
      <TransactionId>{}</TransactionId>
      <Timestamp>{}</Timestamp>
      <Type>{}</Type>
      <AmountNGN>{}</AmountNGN>
      <Direction>{}</Direction>
      <Counterparty>{}</Counterparty>
    </Transaction>"#,
                    t.transaction_id,
                    t.timestamp,
                    escape_xml(&t.transaction_type),
                    t.amount_ngn,
                    t.direction,
                    escape_xml(&t.counterparty)
                ))
                .collect::<Vec<_>>()
                .join("\n"),
            nfiu_ctr.total_amount,
            nfiu_ctr.transaction_count,
            nfiu_ctr.detection_method,
            escape_xml(&nfiu_ctr.filing_institution.name),
            nfiu_ctr.filing_institution.registration_number,
            nfiu_ctr.filing_institution.contact_email,
            nfiu_ctr.submission_timestamp
        );

        Ok(xml)
    }

    /// Generate PDF document (placeholder)
    async fn generate_pdf(
        &self,
        ctr: &Ctr,
        transactions: &[CtrTransaction],
    ) -> Result<String, anyhow::Error> {
        // In production, would use a PDF library like printpdf or wkhtmltopdf
        // For now, return a placeholder URL
        let pdf_filename = format!("ctr_{}.pdf", ctr.ctr_id);
        let pdf_url = format!("/documents/ctrs/{}", pdf_filename);

        info!(
            ctr_id = %ctr.ctr_id,
            transaction_count = transactions.len(),
            "PDF generation placeholder - would generate actual PDF in production"
        );

        Ok(pdf_url)
    }

    /// Store generated documents
    async fn store_documents(&self, documents: &CtrDocuments) -> Result<(), anyhow::Error> {
        sqlx::query(
            r#"
            INSERT INTO ctr_documents
                (ctr_id, xml_content, pdf_url, generated_at)
            VALUES ($1, $2, $3, $4)
            ON CONFLICT (ctr_id) DO UPDATE
            SET xml_content = $2, pdf_url = $3, generated_at = $4
            "#,
        )
        .bind(documents.ctr_id)
        .bind(&documents.xml_content)
        .bind(&documents.pdf_url)
        .bind(documents.generated_at)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Record filing details
    async fn record_filing(&self, result: &FilingResult) -> Result<(), anyhow::Error> {
        let filing_method = "electronic";
        let status_str = match result.status {
            FilingStatus::Pending => "pending",
            FilingStatus::Submitted => "submitted",
            FilingStatus::Acknowledged => "acknowledged",
            FilingStatus::Rejected => "rejected",
            FilingStatus::Failed => "failed",
        };

        sqlx::query(
            r#"
            INSERT INTO ctr_filings
                (ctr_id, filing_method, submission_timestamp, regulatory_submission_reference,
                 acknowledgement_timestamp, acknowledgement_reference, rejection_details)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(result.ctr_id)
        .bind(filing_method)
        .bind(result.submission_timestamp)
        .bind(&result.submission_reference)
        .bind(None::<DateTime<Utc>>)
        .bind(None::<String>)
        .bind(if result.status == FilingStatus::Failed {
            Some(format!("Filing failed after {} retries", result.retry_count))
        } else {
            None
        })
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get CTR
    async fn get_ctr(&self, ctr_id: Uuid) -> Result<Ctr, anyhow::Error> {
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
        .fetch_one(&self.pool)
        .await?;

        Ok(ctr)
    }

    /// Get CTR transactions
    async fn get_ctr_transactions(
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
    async fn update_ctr_status(
        &self,
        ctr_id: Uuid,
        status: CtrStatus,
    ) -> Result<(), anyhow::Error> {
        sqlx::query(
            r#"
            UPDATE ctrs
            SET status = $2
            WHERE ctr_id = $1
            "#,
        )
        .bind(ctr_id)
        .bind(&status)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

/// NFIU submission response
#[derive(Debug, Deserialize)]
struct NfiuSubmissionResponse {
    reference_number: String,
}

/// Escape XML special characters
fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_xml() {
        assert_eq!(escape_xml("Test & Co."), "Test &amp; Co.");
        assert_eq!(escape_xml("<tag>"), "&lt;tag&gt;");
    }

    #[test]
    fn test_default_config() {
        let config = CtrFilingConfig::default();
        assert_eq!(config.max_retry_attempts, 5);
        assert_eq!(config.initial_retry_delay_secs, 2);
    }
}
