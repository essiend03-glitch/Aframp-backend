//! AML case management — compliance officer workflow
//!
//! Flagged transactions are moved to PENDING_COMPLIANCE_REVIEW.
//! Compliance officers can Clear or Permanently Block them.

use super::models::{AmlCaseStatus, AmlFlag, AmlFlagLevel, AmlScreeningResult};
use super::repository::AmlRepository;
use crate::services::notification::NotificationService;
// REMOVED: use crate::sar::SarService;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use tracing::{error, info, warn};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AmlCase {
    pub id: Uuid,
    pub transaction_id: Uuid,
    pub wallet_address: String,
    pub risk_score: f64,
    pub flag_level: String,
    pub flags_json: serde_json::Value,
    pub status: String,
    pub reviewed_by: Option<String>,
    pub review_notes: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct AmlCaseManager {
    repo: AmlRepository,
    notifications: Arc<NotificationService>,
    sar_svc: Option<Arc<SarService>>,
}

impl AmlCaseManager {
    pub fn new(pool: PgPool, notifications: Arc<NotificationService>) -> Self {
        Self {
            repo: AmlRepository::new(pool),
            notifications,
            sar_svc: None,
        }
    }

    /// Attach a SAR service so Critical/Medium cases auto-generate a SAR draft.
    pub fn with_sar(mut self, sar_svc: Arc<SarService>) -> Self {
        self.sar_svc = Some(sar_svc);
        self
    }

    /// Open a new compliance case for a flagged transaction.
    /// Sends instant alert to AML Officer for Critical flags.
    pub async fn open_case(
        &self,
        result: &AmlScreeningResult,
        wallet_address: &str,
    ) -> Result<AmlCase, anyhow::Error> {
        let flags_json = serde_json::to_value(&result.flags)?;
        let flag_level = result
            .flag_level
            .as_ref()
            .map(|l| l.to_string())
            .unwrap_or_else(|| "LOW".into());

        let case = self
            .repo
            .create_case(
                result.transaction_id,
                wallet_address,
                result.risk_score,
                &flag_level,
                flags_json,
            )
            .await?;

        info!(
            case_id = %case.id,
            transaction_id = %result.transaction_id,
            flag_level = %flag_level,
            "AML compliance case opened"
        );

        // Instant alert for Critical (Level 3) flags
        if result.flag_level == Some(AmlFlagLevel::Critical) {
            self.notifications
                .send_system_alert(
                    &case.id.to_string(),
                    &format!(
                        "CRITICAL AML FLAG — transaction {} requires immediate review. Risk score: {:.2}",
                        result.transaction_id, result.risk_score
                    ),
                )
                .await;
        }

        // Auto-generate SAR draft for Critical or Medium flags
        let should_draft = matches!(
            result.flag_level,
            Some(AmlFlagLevel::Critical) | Some(AmlFlagLevel::Medium)
        );
        if should_draft {
            if let Some(ref sar_svc) = self.sar_svc {
                let svc = Arc::clone(sar_svc);
                let case_id = case.id;
                let wallet = wallet_address.to_owned();
                let risk_score = result.risk_score;
                let tx_id = result.transaction_id;
                let flags_json_clone = serde_json::to_value(&result.flags).unwrap_or_default();
                let detection_method = if result.flags.iter().any(|f| matches!(f, AmlFlag::SanctionsHit { .. })) {
                    crate::sar::DetectionMethod::SanctionsMatch
                } else {
                    crate::sar::DetectionMethod::AmlRuleTrigger
                };
                let today = chrono::Utc::now().date_naive();
                tokio::spawn(async move {
                    match svc.auto_initiate(
                        case_id,
                        detection_method,
                        None,
                        vec![wallet],
                        format!("Automated SAR from AML engine. Risk score: {risk_score:.2}"),
                        today - chrono::Duration::days(1),
                        today,
                        rust_decimal::Decimal::ZERO,
                        0,
                        vec![tx_id],
                        flags_json_clone,
                        Some(risk_score),
                        None,
                    ).await {
                        Ok(sar) => {
                            tracing::info!(sar_id = %sar.id, aml_case_id = %case_id, "SAR auto-initiated from AML case");
                        }
                        Err(e) => error!(aml_case_id = %case_id, error = %e, "SAR auto-initiation failed"),
                    }
                });
            }
        }

        Ok(case)
    }

    /// Compliance officer clears a case — transaction may proceed
    pub async fn clear_case(
        &self,
        case_id: Uuid,
        officer_id: &str,
        notes: &str,
    ) -> Result<AmlCase, anyhow::Error> {
        let case = self
            .repo
            .update_case_status(case_id, AmlCaseStatus::Cleared, officer_id, notes)
            .await?;

        info!(
            case_id = %case_id,
            officer = %officer_id,
            "AML case cleared by compliance officer"
        );

        Ok(case)
    }

    /// Compliance officer permanently blocks a transaction
    pub async fn block_case(
        &self,
        case_id: Uuid,
        officer_id: &str,
        notes: &str,
    ) -> Result<AmlCase, anyhow::Error> {
        let case = self
            .repo
            .update_case_status(case_id, AmlCaseStatus::PermanentlyBlocked, officer_id, notes)
            .await?;

        warn!(
            case_id = %case_id,
            officer = %officer_id,
            "Transaction permanently blocked by compliance officer"
        );

        Ok(case)
    }

    /// Check whether a transaction has been cleared for processing
    pub async fn is_cleared(&self, transaction_id: Uuid) -> Result<bool, anyhow::Error> {
        self.repo.is_transaction_cleared(transaction_id).await
    }
}
