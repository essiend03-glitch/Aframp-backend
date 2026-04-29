//! CTR Management Service
//!
//! Manages CTR review workflow, approval process, and compliance checks.
//! Enforces mandatory review checklist and senior officer approval for high-value CTRs.

use super::models::{Ctr, CtrStatus, CtrTransaction};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::str::FromStr;
use tracing::{info, warn};
use uuid::Uuid;

/// Configuration for CTR management
#[derive(Debug, Clone)]
pub struct CtrManagementConfig {
    /// Threshold for requiring senior officer approval (NGN)
    pub senior_approval_threshold: Decimal,
    /// Whether to enforce mandatory review checklist
    pub enforce_checklist: bool,
}

impl Default for CtrManagementConfig {
    fn default() -> Self {
        Self {
            senior_approval_threshold: Decimal::from_str("50000000").unwrap(), // NGN 50M
            enforce_checklist: true,
        }
    }
}

/// Mandatory review checklist items
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewChecklist {
    pub subject_identity_verified: bool,
    pub transaction_details_accurate: bool,
    pub amounts_reconciled: bool,
    pub supporting_documents_attached: bool,
    pub suspicious_activity_noted: bool,
    pub regulatory_requirements_met: bool,
}

impl ReviewChecklist {
    /// Check if all mandatory items are completed
    pub fn is_complete(&self) -> bool {
        self.subject_identity_verified
            && self.transaction_details_accurate
            && self.amounts_reconciled
            && self.supporting_documents_attached
            && self.regulatory_requirements_met
    }

    /// Get list of incomplete items
    pub fn incomplete_items(&self) -> Vec<String> {
        let mut items = Vec::new();
        if !self.subject_identity_verified {
            items.push("Subject identity verification".to_string());
        }
        if !self.transaction_details_accurate {
            items.push("Transaction details accuracy check".to_string());
        }
        if !self.amounts_reconciled {
            items.push("Amount reconciliation".to_string());
        }
        if !self.supporting_documents_attached {
            items.push("Supporting documents attachment".to_string());
        }
        if !self.regulatory_requirements_met {
            items.push("Regulatory requirements verification".to_string());
        }
        items
    }
}

/// CTR review record
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CtrReview {
    pub id: Uuid,
    pub ctr_id: Uuid,
    pub reviewer_id: Uuid,
    pub checklist: sqlx::types::Json<ReviewChecklist>,
    pub review_notes: Option<String>,
    pub reviewed_at: DateTime<Utc>,
}

/// CTR approval record
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CtrApproval {
    pub id: Uuid,
    pub ctr_id: Uuid,
    pub approver_id: Uuid,
    pub approval_level: String,
    pub approval_notes: Option<String>,
    pub approved_at: DateTime<Utc>,
}

/// CTR with full details including reviews and approvals
#[derive(Debug, Clone, Serialize)]
pub struct CtrWithDetails {
    pub ctr: Ctr,
    pub transactions: Vec<CtrTransaction>,
    pub reviews: Vec<CtrReview>,
    pub approvals: Vec<CtrApproval>,
    pub requires_senior_approval: bool,
}

/// Request to review a CTR
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReviewCtrRequest {
    pub reviewer_id: Uuid,
    pub checklist: ReviewChecklist,
    pub review_notes: Option<String>,
}

/// Request to approve a CTR
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ApproveCtrRequest {
    pub approver_id: Uuid,
    pub approval_level: String,
    pub approval_notes: Option<String>,
}

/// Request to return CTR for correction
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReturnForCorrectionRequest {
    pub reviewer_id: Uuid,
    pub correction_notes: String,
    pub issues: Vec<String>,
}

/// Result of CTR review
#[derive(Debug, Clone, Serialize)]
pub struct ReviewResult {
    pub ctr_id: Uuid,
    pub review_id: Uuid,
    pub checklist_complete: bool,
    pub incomplete_items: Vec<String>,
    pub can_proceed_to_approval: bool,
}

/// Result of CTR approval
#[derive(Debug, Clone, Serialize)]
pub struct ApprovalResult {
    pub ctr_id: Uuid,
    pub approval_id: Uuid,
    pub requires_senior_approval: bool,
    pub senior_approval_received: bool,
    pub can_proceed_to_filing: bool,
}

/// CTR Management Service
pub struct CtrManagementService {
    pool: PgPool,
    config: CtrManagementConfig,
}

impl CtrManagementService {
    pub fn new(pool: PgPool, config: CtrManagementConfig) -> Self {
        Self { pool, config }
    }

    /// Get all CTRs with optional status filter
    pub async fn get_all_ctrs(
        &self,
        status_filter: Option<CtrStatus>,
    ) -> Result<Vec<Ctr>, anyhow::Error> {
        let ctrs = if let Some(status) = status_filter {
            sqlx::query_as::<_, Ctr>(
                r#"
                SELECT ctr_id, reporting_period, ctr_type, subject_kyc_id, subject_full_name,
                       subject_identification, subject_address, total_transaction_amount,
                       transaction_count, transaction_references, detection_method, status,
                       assigned_compliance_officer, filing_timestamp, regulatory_reference_number
                FROM ctrs
                WHERE status = $1
                ORDER BY reporting_period DESC
                "#,
            )
            .bind(&status)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, Ctr>(
                r#"
                SELECT ctr_id, reporting_period, ctr_type, subject_kyc_id, subject_full_name,
                       subject_identification, subject_address, total_transaction_amount,
                       transaction_count, transaction_references, detection_method, status,
                       assigned_compliance_officer, filing_timestamp, regulatory_reference_number
                FROM ctrs
                ORDER BY reporting_period DESC
                "#,
            )
            .fetch_all(&self.pool)
            .await?
        };

        Ok(ctrs)
    }

    /// Get CTR by ID with full details
    pub async fn get_ctr_with_details(
        &self,
        ctr_id: Uuid,
    ) -> Result<Option<CtrWithDetails>, anyhow::Error> {
        // Get CTR
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

        if let Some(ctr) = ctr {
            // Get transactions
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

            // Get reviews
            let reviews = sqlx::query_as::<_, CtrReview>(
                r#"
                SELECT id, ctr_id, reviewer_id, checklist, review_notes, reviewed_at
                FROM ctr_reviews
                WHERE ctr_id = $1
                ORDER BY reviewed_at DESC
                "#,
            )
            .bind(ctr_id)
            .fetch_all(&self.pool)
            .await?;

            // Get approvals
            let approvals = sqlx::query_as::<_, CtrApproval>(
                r#"
                SELECT id, ctr_id, approver_id, approval_level, approval_notes, approved_at
                FROM ctr_approvals
                WHERE ctr_id = $1
                ORDER BY approved_at DESC
                "#,
            )
            .bind(ctr_id)
            .fetch_all(&self.pool)
            .await?;

            // Check if requires senior approval
            let requires_senior_approval =
                ctr.total_transaction_amount >= self.config.senior_approval_threshold;

            Ok(Some(CtrWithDetails {
                ctr,
                transactions,
                reviews,
                approvals,
                requires_senior_approval,
            }))
        } else {
            Ok(None)
        }
    }

    /// Review a CTR with mandatory checklist
    pub async fn review_ctr(
        &self,
        ctr_id: Uuid,
        request: ReviewCtrRequest,
    ) -> Result<ReviewResult, anyhow::Error> {
        // Get CTR
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

        // Verify CTR is in correct status for review
        if ctr.status != CtrStatus::Draft && ctr.status != CtrStatus::UnderReview {
            return Err(anyhow::anyhow!(
                "CTR must be in Draft or UnderReview status to be reviewed"
            ));
        }

        // Check if checklist is complete (if enforcement is enabled)
        let checklist_complete = request.checklist.is_complete();
        let incomplete_items = request.checklist.incomplete_items();

        if self.config.enforce_checklist && !checklist_complete {
            warn!(
                ctr_id = %ctr_id,
                incomplete_items = ?incomplete_items,
                "Review checklist incomplete"
            );
        }

        // Create review record
        let review_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO ctr_reviews
                (id, ctr_id, reviewer_id, checklist, review_notes, reviewed_at)
            VALUES ($1, $2, $3, $4, $5, NOW())
            "#,
        )
        .bind(review_id)
        .bind(ctr_id)
        .bind(request.reviewer_id)
        .bind(sqlx::types::Json(&request.checklist))
        .bind(&request.review_notes)
        .execute(&self.pool)
        .await?;

        // Update CTR status to UnderReview if it was Draft
        if ctr.status == CtrStatus::Draft {
            sqlx::query(
                r#"
                UPDATE ctrs
                SET status = $2
                WHERE ctr_id = $1
                "#,
            )
            .bind(ctr_id)
            .bind(&CtrStatus::UnderReview)
            .execute(&self.pool)
            .await?;
        }

        info!(
            ctr_id = %ctr_id,
            reviewer_id = %request.reviewer_id,
            checklist_complete = checklist_complete,
            "CTR reviewed"
        );

        let can_proceed_to_approval = if self.config.enforce_checklist {
            checklist_complete
        } else {
            true
        };

        Ok(ReviewResult {
            ctr_id,
            review_id,
            checklist_complete,
            incomplete_items,
            can_proceed_to_approval,
        })
    }

    /// Approve a CTR
    pub async fn approve_ctr(
        &self,
        ctr_id: Uuid,
        request: ApproveCtrRequest,
    ) -> Result<ApprovalResult, anyhow::Error> {
        // Get CTR
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

        // Verify CTR is in correct status for approval
        if ctr.status != CtrStatus::UnderReview {
            return Err(anyhow::anyhow!(
                "CTR must be in UnderReview status to be approved"
            ));
        }

        // Check if review checklist is complete (if enforcement is enabled)
        if self.config.enforce_checklist {
            let latest_review = sqlx::query_as::<_, CtrReview>(
                r#"
                SELECT id, ctr_id, reviewer_id, checklist, review_notes, reviewed_at
                FROM ctr_reviews
                WHERE ctr_id = $1
                ORDER BY reviewed_at DESC
                LIMIT 1
                "#,
            )
            .bind(ctr_id)
            .fetch_optional(&self.pool)
            .await?;

            if let Some(review) = latest_review {
                if !review.checklist.0.is_complete() {
                    return Err(anyhow::anyhow!(
                        "Cannot approve CTR: review checklist is incomplete"
                    ));
                }
            } else {
                return Err(anyhow::anyhow!(
                    "Cannot approve CTR: no review record found"
                ));
            }
        }

        // Check if requires senior approval
        let requires_senior_approval =
            ctr.total_transaction_amount >= self.config.senior_approval_threshold;

        // If requires senior approval, verify approval level
        if requires_senior_approval && request.approval_level != "senior" {
            return Err(anyhow::anyhow!(
                "CTR amount {} exceeds threshold {}. Senior officer approval required.",
                ctr.total_transaction_amount,
                self.config.senior_approval_threshold
            ));
        }

        // Create approval record
        let approval_id = Uuid::new_v4();
        sqlx::query(
            r#"
            INSERT INTO ctr_approvals
                (id, ctr_id, approver_id, approval_level, approval_notes, approved_at)
            VALUES ($1, $2, $3, $4, $5, NOW())
            "#,
        )
        .bind(approval_id)
        .bind(ctr_id)
        .bind(request.approver_id)
        .bind(&request.approval_level)
        .bind(&request.approval_notes)
        .execute(&self.pool)
        .await?;

        // Check if we have senior approval (if required)
        let senior_approval_received = if requires_senior_approval {
            let senior_approvals = sqlx::query_scalar::<_, i64>(
                r#"
                SELECT COUNT(*)
                FROM ctr_approvals
                WHERE ctr_id = $1 AND approval_level = 'senior'
                "#,
            )
            .bind(ctr_id)
            .fetch_one(&self.pool)
            .await?;

            senior_approvals > 0
        } else {
            false // Not required, so N/A
        };

        // Update CTR status to Approved if all requirements met
        let can_proceed_to_filing = if requires_senior_approval {
            senior_approval_received
        } else {
            true
        };

        if can_proceed_to_filing {
            sqlx::query(
                r#"
                UPDATE ctrs
                SET status = $2
                WHERE ctr_id = $1
                "#,
            )
            .bind(ctr_id)
            .bind(&CtrStatus::Approved)
            .execute(&self.pool)
            .await?;

            info!(
                ctr_id = %ctr_id,
                approver_id = %request.approver_id,
                approval_level = %request.approval_level,
                "CTR approved and ready for filing"
            );
        } else {
            info!(
                ctr_id = %ctr_id,
                approver_id = %request.approver_id,
                approval_level = %request.approval_level,
                "CTR approval recorded, awaiting senior approval"
            );
        }

        Ok(ApprovalResult {
            ctr_id,
            approval_id,
            requires_senior_approval,
            senior_approval_received,
            can_proceed_to_filing,
        })
    }

    /// Return CTR for correction
    pub async fn return_for_correction(
        &self,
        ctr_id: Uuid,
        request: ReturnForCorrectionRequest,
    ) -> Result<(), anyhow::Error> {
        // Get CTR
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

        // Verify CTR is in correct status
        if ctr.status == CtrStatus::Filed
            || ctr.status == CtrStatus::Acknowledged
            || ctr.status == CtrStatus::Rejected
        {
            return Err(anyhow::anyhow!(
                "Cannot return CTR in {} status for correction",
                match ctr.status {
                    CtrStatus::Filed => "Filed",
                    CtrStatus::Acknowledged => "Acknowledged",
                    CtrStatus::Rejected => "Rejected",
                    _ => "unknown",
                }
            ));
        }

        // Create correction record
        sqlx::query(
            r#"
            INSERT INTO ctr_corrections
                (id, ctr_id, reviewer_id, correction_notes, issues, created_at)
            VALUES ($1, $2, $3, $4, $5, NOW())
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(ctr_id)
        .bind(request.reviewer_id)
        .bind(&request.correction_notes)
        .bind(&request.issues)
        .execute(&self.pool)
        .await?;

        // Update CTR status back to Draft
        sqlx::query(
            r#"
            UPDATE ctrs
            SET status = $2
            WHERE ctr_id = $1
            "#,
        )
        .bind(ctr_id)
        .bind(&CtrStatus::Draft)
        .execute(&self.pool)
        .await?;

        warn!(
            ctr_id = %ctr_id,
            reviewer_id = %request.reviewer_id,
            issues = ?request.issues,
            "CTR returned for correction"
        );

        Ok(())
    }

    /// Get CTRs requiring senior approval
    pub async fn get_ctrs_requiring_senior_approval(&self) -> Result<Vec<Ctr>, anyhow::Error> {
        let ctrs = sqlx::query_as::<_, Ctr>(
            r#"
            SELECT ctr_id, reporting_period, ctr_type, subject_kyc_id, subject_full_name,
                   subject_identification, subject_address, total_transaction_amount,
                   transaction_count, transaction_references, detection_method, status,
                   assigned_compliance_officer, filing_timestamp, regulatory_reference_number
            FROM ctrs
            WHERE status = 'under_review'
              AND total_transaction_amount >= $1
              AND NOT EXISTS (
                  SELECT 1 FROM ctr_approvals
                  WHERE ctr_approvals.ctr_id = ctrs.ctr_id
                    AND ctr_approvals.approval_level = 'senior'
              )
            ORDER BY total_transaction_amount DESC
            "#,
        )
        .bind(self.config.senior_approval_threshold)
        .fetch_all(&self.pool)
        .await?;

        Ok(ctrs)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checklist_complete() {
        let complete = ReviewChecklist {
            subject_identity_verified: true,
            transaction_details_accurate: true,
            amounts_reconciled: true,
            supporting_documents_attached: true,
            suspicious_activity_noted: false,
            regulatory_requirements_met: true,
        };
        assert!(complete.is_complete());

        let incomplete = ReviewChecklist {
            subject_identity_verified: true,
            transaction_details_accurate: false,
            amounts_reconciled: true,
            supporting_documents_attached: true,
            suspicious_activity_noted: false,
            regulatory_requirements_met: true,
        };
        assert!(!incomplete.is_complete());
        assert_eq!(incomplete.incomplete_items().len(), 1);
    }

    #[test]
    fn test_default_config() {
        let config = CtrManagementConfig::default();
        assert_eq!(
            config.senior_approval_threshold,
            Decimal::from_str("50000000").unwrap()
        );
        assert!(config.enforce_checklist);
    }
}
