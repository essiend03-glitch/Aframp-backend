//! CTR Batch Filing Service
//!
//! Handles batch filing of multiple CTRs with per-CTR status tracking,
//! deadline monitoring, reminders, and overdue alerts.

use super::ctr_filing::{CtrFilingService, FilingResult, FilingStatus};
use super::ctr_logging;
use super::ctr_metrics;
use super::models::{Ctr, CtrStatus};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use tracing::{error, info, warn};
use uuid::Uuid;

/// Configuration for batch filing and deadline monitoring
#[derive(Debug, Clone)]
pub struct BatchFilingConfig {
    /// Email address for compliance director
    pub compliance_director_email: String,
    /// Days before deadline for first reminder
    pub first_reminder_days: i64,
    /// Days before deadline for second reminder
    pub second_reminder_days: i64,
    /// Days before deadline for final reminder
    pub final_reminder_days: i64,
}

impl Default for BatchFilingConfig {
    fn default() -> Self {
        Self {
            compliance_director_email: "compliance-director@example.com".to_string(),
            first_reminder_days: 3,
            second_reminder_days: 1,
            final_reminder_days: 0, // On deadline day
        }
    }
}

/// Batch filing request
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BatchFilingRequest {
    pub ctr_ids: Vec<Uuid>,
}

/// Per-CTR filing status in batch
#[derive(Debug, Clone, Serialize)]
pub struct CtrFilingStatus {
    pub ctr_id: Uuid,
    pub subject_name: String,
    pub total_amount: String,
    pub status: String,
    pub submission_reference: Option<String>,
    pub error: Option<String>,
    pub retry_count: u32,
}

/// Batch filing summary report
#[derive(Debug, Clone, Serialize)]
pub struct BatchFilingSummary {
    pub batch_id: Uuid,
    pub total_ctrs: usize,
    pub successful: usize,
    pub failed: usize,
    pub skipped: usize,
    pub ctr_statuses: Vec<CtrFilingStatus>,
    pub started_at: DateTime<Utc>,
    pub completed_at: DateTime<Utc>,
    pub duration_seconds: i64,
}

/// Deadline status for a CTR
#[derive(Debug, Clone, Serialize)]
pub struct CtrDeadlineStatus {
    pub ctr_id: Uuid,
    pub subject_name: String,
    pub total_amount: String,
    pub filing_deadline: DateTime<Utc>,
    pub days_until_deadline: i64,
    pub status: String,
    pub is_overdue: bool,
    pub reminder_sent: bool,
}

/// Deadline status report
#[derive(Debug, Clone, Serialize)]
pub struct DeadlineStatusReport {
    pub total_ctrs: usize,
    pub overdue: usize,
    pub due_today: usize,
    pub due_within_3_days: usize,
    pub ctrs: Vec<CtrDeadlineStatus>,
    pub generated_at: DateTime<Utc>,
}

/// Reminder notification
#[derive(Debug, Clone, Serialize)]
pub struct ReminderNotification {
    pub ctr_id: Uuid,
    pub subject_name: String,
    pub filing_deadline: DateTime<Utc>,
    pub days_until_deadline: i64,
    pub reminder_type: ReminderType,
}

/// Reminder type
#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum ReminderType {
    FirstReminder,  // 3 days before
    SecondReminder, // 1 day before
    FinalReminder,  // On deadline day
    OverdueAlert,   // Past deadline
}

/// CTR Batch Filing Service
pub struct CtrBatchFilingService {
    pool: PgPool,
    config: BatchFilingConfig,
    filing_service: Arc<CtrFilingService>,
}

impl CtrBatchFilingService {
    pub fn new(
        pool: PgPool,
        config: BatchFilingConfig,
        filing_service: Arc<CtrFilingService>,
    ) -> Self {
        Self {
            pool,
            config,
            filing_service,
        }
    }

    /// Batch file multiple CTRs
    pub async fn batch_file(
        &self,
        request: BatchFilingRequest,
    ) -> Result<BatchFilingSummary, anyhow::Error> {
        let batch_id = Uuid::new_v4();
        let started_at = Utc::now();

        info!(
            batch_id = %batch_id,
            ctr_count = request.ctr_ids.len(),
            "Starting batch CTR filing"
        );

        let mut ctr_statuses = Vec::new();
        let mut successful = 0;
        let mut failed = 0;
        let mut skipped = 0;

        for ctr_id in &request.ctr_ids {
            let status = self.file_single_ctr(*ctr_id).await;

            match &status.status.as_str() {
                &"Submitted" | &"Acknowledged" => successful += 1,
                &"Failed" => failed += 1,
                &"Skipped" => skipped += 1,
                _ => {}
            }

            ctr_statuses.push(status);
        }

        let completed_at = Utc::now();
        let duration_seconds = (completed_at - started_at).num_seconds();

        info!(
            batch_id = %batch_id,
            total = request.ctr_ids.len(),
            successful = successful,
            failed = failed,
            skipped = skipped,
            duration_seconds = duration_seconds,
            "Batch CTR filing completed"
        );

        // Record metrics
        ctr_metrics::record_batch_filing("successful", successful);
        ctr_metrics::record_batch_filing("failed", failed);
        ctr_metrics::record_batch_filing("skipped", skipped);

        // Determine batch size range for metrics
        let batch_size_range = if request.ctr_ids.len() <= 10 {
            "1-10"
        } else if request.ctr_ids.len() <= 50 {
            "11-50"
        } else {
            "51+"
        };
        ctr_metrics::record_batch_filing_duration(batch_size_range, duration_seconds as f64);

        // Log structured event
        ctr_logging::log_batch_filing(
            batch_id,
            request.ctr_ids.len(),
            successful,
            failed,
            skipped,
        );

        Ok(BatchFilingSummary {
            batch_id,
            total_ctrs: request.ctr_ids.len(),
            successful,
            failed,
            skipped,
            ctr_statuses,
            started_at,
            completed_at,
            duration_seconds,
        })
    }

    /// File a single CTR in batch context
    async fn file_single_ctr(&self, ctr_id: Uuid) -> CtrFilingStatus {
        // Get CTR details
        let ctr = match self.get_ctr(ctr_id).await {
            Ok(ctr) => ctr,
            Err(e) => {
                error!(ctr_id = %ctr_id, error = %e, "Failed to get CTR");
                return CtrFilingStatus {
                    ctr_id,
                    subject_name: "Unknown".to_string(),
                    total_amount: "0".to_string(),
                    status: "Skipped".to_string(),
                    submission_reference: None,
                    error: Some(format!("Failed to get CTR: {}", e)),
                    retry_count: 0,
                };
            }
        };

        // Check if already filed
        if ctr.status == CtrStatus::Filed
            || ctr.status == CtrStatus::Acknowledged
            || ctr.status == CtrStatus::Rejected
        {
            info!(
                ctr_id = %ctr_id,
                status = ?ctr.status,
                "CTR already filed, skipping"
            );
            return CtrFilingStatus {
                ctr_id,
                subject_name: ctr.subject_full_name,
                total_amount: ctr.total_transaction_amount.to_string(),
                status: "Skipped".to_string(),
                submission_reference: ctr.regulatory_reference_number,
                error: Some(format!("Already in {:?} status", ctr.status)),
                retry_count: 0,
            };
        }

        // Check if approved
        if ctr.status != CtrStatus::Approved {
            warn!(
                ctr_id = %ctr_id,
                status = ?ctr.status,
                "CTR not approved, skipping"
            );
            return CtrFilingStatus {
                ctr_id,
                subject_name: ctr.subject_full_name,
                total_amount: ctr.total_transaction_amount.to_string(),
                status: "Skipped".to_string(),
                submission_reference: None,
                error: Some(format!("Not approved (status: {:?})", ctr.status)),
                retry_count: 0,
            };
        }

        // Attempt to file
        match self.filing_service.file_ctr(ctr_id).await {
            Ok(result) => CtrFilingStatus {
                ctr_id,
                subject_name: ctr.subject_full_name,
                total_amount: ctr.total_transaction_amount.to_string(),
                status: format!("{:?}", result.status),
                submission_reference: Some(result.submission_reference),
                error: None,
                retry_count: result.retry_count,
            },
            Err(e) => {
                error!(ctr_id = %ctr_id, error = %e, "Failed to file CTR");
                CtrFilingStatus {
                    ctr_id,
                    subject_name: ctr.subject_full_name,
                    total_amount: ctr.total_transaction_amount.to_string(),
                    status: "Failed".to_string(),
                    submission_reference: None,
                    error: Some(e.to_string()),
                    retry_count: 0,
                }
            }
        }
    }

    /// Get deadline status for all pending CTRs
    pub async fn get_deadline_status(&self) -> Result<DeadlineStatusReport, anyhow::Error> {
        info!("Generating deadline status report");

        // Get all CTRs that are not yet filed
        let ctrs = sqlx::query_as::<_, Ctr>(
            r#"
            SELECT ctr_id, reporting_period, ctr_type, subject_kyc_id, subject_full_name,
                   subject_identification, subject_address, total_transaction_amount,
                   transaction_count, transaction_references, detection_method, status,
                   assigned_compliance_officer, filing_timestamp, regulatory_reference_number
            FROM ctrs
            WHERE status IN ('draft', 'under_review', 'approved')
              AND filing_timestamp IS NOT NULL
            ORDER BY filing_timestamp ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let now = Utc::now();
        let mut overdue = 0;
        let mut due_today = 0;
        let mut due_within_3_days = 0;

        let mut ctr_statuses = Vec::new();

        for ctr in ctrs {
            if let Some(deadline) = ctr.filing_timestamp {
                let days_until = (deadline - now).num_days();
                let is_overdue = deadline < now;

                if is_overdue {
                    overdue += 1;
                } else if days_until == 0 {
                    due_today += 1;
                } else if days_until <= 3 {
                    due_within_3_days += 1;
                }

                // Check if reminder was sent
                let reminder_sent = self.check_reminder_sent(ctr.ctr_id).await?;

                ctr_statuses.push(CtrDeadlineStatus {
                    ctr_id: ctr.ctr_id,
                    subject_name: ctr.subject_full_name,
                    total_amount: ctr.total_transaction_amount.to_string(),
                    filing_deadline: deadline,
                    days_until_deadline: days_until,
                    status: format!("{:?}", ctr.status),
                    is_overdue,
                    reminder_sent,
                });
            }
        }

        Ok(DeadlineStatusReport {
            total_ctrs: ctr_statuses.len(),
            overdue,
            due_today,
            due_within_3_days,
            ctrs: ctr_statuses,
            generated_at: Utc::now(),
        })
    }

    /// Process deadline reminders and alerts
    pub async fn process_deadline_reminders(&self) -> Result<Vec<ReminderNotification>, anyhow::Error> {
        info!("Processing deadline reminders");

        let status_report = self.get_deadline_status().await?;
        let mut notifications = Vec::new();

        for ctr_status in &status_report.ctrs {
            // Skip if reminder already sent
            if ctr_status.reminder_sent {
                continue;
            }

            let reminder_type = if ctr_status.is_overdue {
                Some(ReminderType::OverdueAlert)
            } else if ctr_status.days_until_deadline == self.config.final_reminder_days {
                Some(ReminderType::FinalReminder)
            } else if ctr_status.days_until_deadline == self.config.second_reminder_days {
                Some(ReminderType::SecondReminder)
            } else if ctr_status.days_until_deadline == self.config.first_reminder_days {
                Some(ReminderType::FirstReminder)
            } else {
                None
            };

            if let Some(reminder_type) = reminder_type {
                let notification = ReminderNotification {
                    ctr_id: ctr_status.ctr_id,
                    subject_name: ctr_status.subject_name.clone(),
                    filing_deadline: ctr_status.filing_deadline,
                    days_until_deadline: ctr_status.days_until_deadline,
                    reminder_type: reminder_type.clone(),
                };

                // Send notification
                self.send_reminder_notification(&notification).await?;

                // Record reminder sent
                self.record_reminder_sent(ctr_status.ctr_id, &reminder_type)
                    .await?;

                notifications.push(notification);
            }
        }

        info!(
            reminders_sent = notifications.len(),
            "Deadline reminders processed"
        );

        Ok(notifications)
    }

    /// Send reminder notification
    async fn send_reminder_notification(
        &self,
        notification: &ReminderNotification,
    ) -> Result<(), anyhow::Error> {
        let message = match notification.reminder_type {
            ReminderType::FirstReminder => {
                format!(
                    "REMINDER: CTR {} for {} (Amount: {}) is due in {} days (Deadline: {})",
                    notification.ctr_id,
                    notification.subject_name,
                    "amount",
                    notification.days_until_deadline,
                    notification.filing_deadline.format("%Y-%m-%d")
                )
            }
            ReminderType::SecondReminder => {
                format!(
                    "URGENT: CTR {} for {} is due TOMORROW (Deadline: {})",
                    notification.ctr_id,
                    notification.subject_name,
                    notification.filing_deadline.format("%Y-%m-%d")
                )
            }
            ReminderType::FinalReminder => {
                format!(
                    "FINAL REMINDER: CTR {} for {} is due TODAY (Deadline: {})",
                    notification.ctr_id,
                    notification.subject_name,
                    notification.filing_deadline.format("%Y-%m-%d")
                )
            }
            ReminderType::OverdueAlert => {
                format!(
                    "OVERDUE ALERT: CTR {} for {} is OVERDUE by {} days (Deadline was: {})",
                    notification.ctr_id,
                    notification.subject_name,
                    notification.days_until_deadline.abs(),
                    notification.filing_deadline.format("%Y-%m-%d")
                )
            }
        };

        // Log the notification
        match notification.reminder_type {
            ReminderType::OverdueAlert => {
                error!(
                    ctr_id = %notification.ctr_id,
                    subject = %notification.subject_name,
                    days_overdue = notification.days_until_deadline.abs(),
                    "CTR OVERDUE - Alerting compliance director"
                );

                // Record metrics
                ctr_metrics::record_overdue_alert("compliance_director");
                ctr_logging::log_overdue_alert(
                    notification.ctr_id,
                    notification.subject_name.clone(),
                    notification.days_until_deadline.abs(),
                );

                // Send alert to compliance director
                self.alert_compliance_director(notification).await?;
            }
            _ => {
                warn!(
                    ctr_id = %notification.ctr_id,
                    subject = %notification.subject_name,
                    days_until = notification.days_until_deadline,
                    reminder_type = ?notification.reminder_type,
                    "Deadline reminder sent"
                );

                // Record metrics
                let reminder_type_str = match notification.reminder_type {
                    ReminderType::FirstReminder => "3_days",
                    ReminderType::SecondReminder => "1_day",
                    ReminderType::FinalReminder => "deadline_day",
                    ReminderType::OverdueAlert => "overdue",
                };
                ctr_metrics::record_deadline_reminder(reminder_type_str);
                ctr_logging::log_deadline_reminder(
                    notification.ctr_id,
                    reminder_type_str.to_string(),
                    notification.days_until_deadline,
                );
            }
        }

        // Store notification in database
        sqlx::query(
            r#"
            INSERT INTO ctr_deadline_notifications
                (id, ctr_id, notification_type, message, sent_at)
            VALUES ($1, $2, $3, $4, NOW())
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(notification.ctr_id)
        .bind(format!("{:?}", notification.reminder_type))
        .bind(&message)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Alert compliance director about overdue CTR
    async fn alert_compliance_director(
        &self,
        notification: &ReminderNotification,
    ) -> Result<(), anyhow::Error> {
        // In production, would send email via email service
        // For now, log the alert
        error!(
            ctr_id = %notification.ctr_id,
            subject = %notification.subject_name,
            days_overdue = notification.days_until_deadline.abs(),
            director_email = %self.config.compliance_director_email,
            "IMMEDIATE ALERT: Overdue CTR - Compliance Director notified"
        );

        // Record alert in database
        sqlx::query(
            r#"
            INSERT INTO compliance_director_alerts
                (id, ctr_id, alert_type, message, recipient_email, sent_at)
            VALUES ($1, $2, $3, $4, $5, NOW())
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(notification.ctr_id)
        .bind("overdue_ctr")
        .bind(format!(
            "OVERDUE: CTR {} for {} is {} days overdue",
            notification.ctr_id,
            notification.subject_name,
            notification.days_until_deadline.abs()
        ))
        .bind(&self.config.compliance_director_email)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Check if reminder was sent for a CTR
    async fn check_reminder_sent(&self, ctr_id: Uuid) -> Result<bool, anyhow::Error> {
        let count = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM ctr_deadline_notifications
            WHERE ctr_id = $1
              AND sent_at > NOW() - INTERVAL '24 hours'
            "#,
        )
        .bind(ctr_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(count > 0)
    }

    /// Record that a reminder was sent
    async fn record_reminder_sent(
        &self,
        ctr_id: Uuid,
        reminder_type: &ReminderType,
    ) -> Result<(), anyhow::Error> {
        info!(
            ctr_id = %ctr_id,
            reminder_type = ?reminder_type,
            "Recording reminder sent"
        );

        // Already recorded in send_reminder_notification
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = BatchFilingConfig::default();
        assert_eq!(config.first_reminder_days, 3);
        assert_eq!(config.second_reminder_days, 1);
        assert_eq!(config.final_reminder_days, 0);
    }
}
