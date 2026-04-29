//! CTR Structured Logging
//!
//! Provides structured log events for every CTR state change and lifecycle event.

use chrono::{DateTime, Utc};
use serde::Serialize;
use tracing::{event, Level};
use uuid::Uuid;

/// CTR lifecycle event
#[derive(Debug, Clone, Serialize)]
pub struct CtrLifecycleEvent {
    pub event_type: String,
    pub ctr_id: Uuid,
    pub subject_id: Option<Uuid>,
    pub subject_name: Option<String>,
    pub from_status: Option<String>,
    pub to_status: Option<String>,
    pub amount_ngn: Option<String>,
    pub transaction_count: Option<i32>,
    pub actor_id: Option<Uuid>,
    pub reason: Option<String>,
    pub timestamp: DateTime<Utc>,
    pub metadata: serde_json::Value,
}

impl CtrLifecycleEvent {
    pub fn new(event_type: impl Into<String>, ctr_id: Uuid) -> Self {
        Self {
            event_type: event_type.into(),
            ctr_id,
            subject_id: None,
            subject_name: None,
            from_status: None,
            to_status: None,
            amount_ngn: None,
            transaction_count: None,
            actor_id: None,
            reason: None,
            timestamp: Utc::now(),
            metadata: serde_json::json!({}),
        }
    }

    pub fn with_subject(mut self, subject_id: Uuid, subject_name: String) -> Self {
        self.subject_id = Some(subject_id);
        self.subject_name = Some(subject_name);
        self
    }

    pub fn with_status_change(mut self, from: String, to: String) -> Self {
        self.from_status = Some(from);
        self.to_status = Some(to);
        self
    }

    pub fn with_amount(mut self, amount: String, count: i32) -> Self {
        self.amount_ngn = Some(amount);
        self.transaction_count = Some(count);
        self
    }

    pub fn with_actor(mut self, actor_id: Uuid) -> Self {
        self.actor_id = Some(actor_id);
        self
    }

    pub fn with_reason(mut self, reason: String) -> Self {
        self.reason = Some(reason);
        self
    }

    pub fn with_metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }

    pub fn log(&self) {
        let json = serde_json::to_string(self).unwrap_or_default();
        event!(Level::INFO, ctr_lifecycle_event = %json);
    }
}

/// Log CTR generation
pub fn log_ctr_generated(
    ctr_id: Uuid,
    subject_id: Uuid,
    subject_name: String,
    amount: String,
    count: i32,
    detection_method: String,
) {
    CtrLifecycleEvent::new("ctr_generated", ctr_id)
        .with_subject(subject_id, subject_name)
        .with_amount(amount, count)
        .with_metadata(serde_json::json!({
            "detection_method": detection_method
        }))
        .log();
}

/// Log CTR status change
pub fn log_status_change(
    ctr_id: Uuid,
    from_status: String,
    to_status: String,
    actor_id: Option<Uuid>,
    reason: Option<String>,
) {
    let mut event = CtrLifecycleEvent::new("status_change", ctr_id)
        .with_status_change(from_status, to_status);

    if let Some(actor) = actor_id {
        event = event.with_actor(actor);
    }

    if let Some(r) = reason {
        event = event.with_reason(r);
    }

    event.log();
}

/// Log CTR review
pub fn log_ctr_reviewed(
    ctr_id: Uuid,
    reviewer_id: Uuid,
    checklist_complete: bool,
    notes: Option<String>,
) {
    CtrLifecycleEvent::new("ctr_reviewed", ctr_id)
        .with_actor(reviewer_id)
        .with_metadata(serde_json::json!({
            "checklist_complete": checklist_complete,
            "notes": notes
        }))
        .log();
}

/// Log CTR approval
pub fn log_ctr_approved(
    ctr_id: Uuid,
    approver_id: Uuid,
    approval_level: String,
    notes: Option<String>,
) {
    CtrLifecycleEvent::new("ctr_approved", ctr_id)
        .with_actor(approver_id)
        .with_metadata(serde_json::json!({
            "approval_level": approval_level,
            "notes": notes
        }))
        .log();
}

/// Log CTR filed
pub fn log_ctr_filed(
    ctr_id: Uuid,
    submission_reference: String,
    retry_count: u32,
) {
    CtrLifecycleEvent::new("ctr_filed", ctr_id)
        .with_metadata(serde_json::json!({
            "submission_reference": submission_reference,
            "retry_count": retry_count
        }))
        .log();
}

/// Log exemption applied
pub fn log_exemption_applied(
    ctr_id: Uuid,
    subject_id: Uuid,
    exemption_category: String,
) {
    CtrLifecycleEvent::new("exemption_applied", ctr_id)
        .with_metadata(serde_json::json!({
            "subject_id": subject_id,
            "exemption_category": exemption_category
        }))
        .log();
}

/// Log threshold breach
pub fn log_threshold_breach(
    subject_id: Uuid,
    subject_name: String,
    amount: String,
    threshold: String,
    subject_type: String,
) {
    CtrLifecycleEvent::new("threshold_breach", Uuid::nil())
        .with_subject(subject_id, subject_name)
        .with_metadata(serde_json::json!({
            "amount": amount,
            "threshold": threshold,
            "subject_type": subject_type
        }))
        .log();
}

/// Log deadline reminder
pub fn log_deadline_reminder(
    ctr_id: Uuid,
    reminder_type: String,
    days_until_deadline: i64,
) {
    CtrLifecycleEvent::new("deadline_reminder", ctr_id)
        .with_metadata(serde_json::json!({
            "reminder_type": reminder_type,
            "days_until_deadline": days_until_deadline
        }))
        .log();
}

/// Log overdue alert
pub fn log_overdue_alert(
    ctr_id: Uuid,
    subject_name: String,
    days_overdue: i64,
) {
    CtrLifecycleEvent::new("overdue_alert", ctr_id)
        .with_metadata(serde_json::json!({
            "subject_name": subject_name,
            "days_overdue": days_overdue
        }))
        .log();
}

/// Log batch filing
pub fn log_batch_filing(
    batch_id: Uuid,
    total_ctrs: usize,
    successful: usize,
    failed: usize,
    skipped: usize,
) {
    CtrLifecycleEvent::new("batch_filing", batch_id)
        .with_metadata(serde_json::json!({
            "total_ctrs": total_ctrs,
            "successful": successful,
            "failed": failed,
            "skipped": skipped
        }))
        .log();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lifecycle_event() {
        let event = CtrLifecycleEvent::new("test_event", Uuid::new_v4())
            .with_subject(Uuid::new_v4(), "Test Subject".to_string())
            .with_status_change("draft".to_string(), "approved".to_string());

        assert_eq!(event.event_type, "test_event");
        assert!(event.subject_name.is_some());
    }
}
