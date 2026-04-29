//! Business logic for Disaster Recovery & Business Continuity Planning (Issue #DR-BCP).

use crate::dr_bcp::{
    models::*,
    repository::DrBcpRepository,
};
use chrono::Utc;
use tracing::{error, info, warn};
use uuid::Uuid;

/// RPO target: 0 seconds (zero data loss).
pub const RPO_TARGET_SECONDS: i64 = 0;
/// RTO target: 15 minutes.
pub const RTO_TARGET_SECONDS: i64 = 900;

pub struct DrBcpService {
    repo: DrBcpRepository,
}

impl DrBcpService {
    pub fn new(repo: DrBcpRepository) -> Self {
        Self { repo }
    }

    // -----------------------------------------------------------------------
    // DR status overview
    // -----------------------------------------------------------------------

    pub async fn get_status(&self) -> Result<DrStatusResponse, String> {
        let (active_incidents, last_backup, last_restore_test, bia_entries) = tokio::try_join!(
            self.repo.active_incidents(),
            self.repo.latest_backup(),
            self.repo.latest_restore_test(),
            self.repo.list_bia_entries(),
        )
        .map_err(|e| e.to_string())?;

        Ok(DrStatusResponse {
            active_incidents,
            last_backup,
            last_restore_test,
            bia_entries,
        })
    }

    // -----------------------------------------------------------------------
    // Incident management
    // -----------------------------------------------------------------------

    pub async fn declare_incident(
        &self,
        req: DeclareDrIncidentRequest,
    ) -> Result<DrIncident, String> {
        let now = Utc::now();
        let incident = DrIncident {
            id: Uuid::new_v4(),
            title: req.title,
            description: req.description,
            status: DrIncidentStatus::Declared,
            commander_id: req.commander_id,
            affected_services: serde_json::json!(req.affected_services),
            rpo_achieved_seconds: None,
            rto_achieved_seconds: None,
            declared_at: now,
            resolved_at: None,
            created_at: now,
            updated_at: now,
        };

        self.repo
            .create_incident(&incident)
            .await
            .map_err(|e| e.to_string())?;

        info!(incident_id = %incident.id, "DR incident declared");
        Ok(incident)
    }

    pub async fn update_incident_status(
        &self,
        id: Uuid,
        req: UpdateIncidentStatusRequest,
    ) -> Result<(), String> {
        // Warn if RTO target was breached.
        if let Some(rto) = req.rto_achieved_seconds {
            if rto > RTO_TARGET_SECONDS {
                warn!(
                    incident_id = %id,
                    rto_achieved = rto,
                    rto_target = RTO_TARGET_SECONDS,
                    "RTO target breached"
                );
            }
        }
        self.repo
            .update_incident_status(id, &req)
            .await
            .map_err(|e| e.to_string())
    }

    // -----------------------------------------------------------------------
    // Regulatory notifications
    // -----------------------------------------------------------------------

    pub async fn send_regulatory_notification(
        &self,
        incident_id: Uuid,
        req: SendRegulatoryNotificationRequest,
    ) -> Result<RegulatoryNotification, String> {
        let template = regulatory_template(req.body, incident_id);
        let notif = RegulatoryNotification {
            id: Uuid::new_v4(),
            incident_id,
            body: req.body,
            template_used: template.clone(),
            sent_at: Utc::now(),
            acknowledged_at: None,
        };

        self.repo
            .record_regulatory_notification(&notif)
            .await
            .map_err(|e| e.to_string())?;

        // In production this would dispatch via PagerDuty / email / SMS.
        info!(
            incident_id = %incident_id,
            regulatory_body = ?req.body,
            "Regulatory notification dispatched"
        );
        Ok(notif)
    }

    // -----------------------------------------------------------------------
    // Backup & restore
    // -----------------------------------------------------------------------

    pub async fn record_restore_test(
        &self,
        backup_id: Uuid,
        result: RestoreTestResult,
        restore_duration_seconds: i64,
        rpo_achieved_seconds: Option<i64>,
        rto_achieved_seconds: Option<i64>,
        error_message: Option<String>,
    ) -> Result<RestoreTestRun, String> {
        let run = RestoreTestRun {
            id: Uuid::new_v4(),
            backup_id,
            result,
            restore_duration_seconds,
            rpo_achieved_seconds,
            rto_achieved_seconds,
            error_message,
            run_at: Utc::now(),
        };

        self.repo
            .record_restore_test(&run)
            .await
            .map_err(|e| e.to_string())?;

        if matches!(result, RestoreTestResult::Passed) {
            self.repo
                .mark_backup_verified(backup_id)
                .await
                .map_err(|e| e.to_string())?;
        } else {
            error!(backup_id = %backup_id, "Backup restore test FAILED");
        }

        Ok(run)
    }
}

// ---------------------------------------------------------------------------
// Regulatory communication templates
// ---------------------------------------------------------------------------

fn regulatory_template(body: RegulatoryBody, incident_id: Uuid) -> String {
    match body {
        RegulatoryBody::Cbn => format!(
            "INCIDENT NOTIFICATION — CBN\n\
             Incident ID: {incident_id}\n\
             Aframp hereby notifies the Central Bank of Nigeria of a service disruption \
             affecting payment processing operations. Our Emergency Response Team has been \
             activated. We will provide updates every 30 minutes until resolution. \
             Full post-incident report will be submitted within 24 hours of resolution."
        ),
        RegulatoryBody::Sec => format!(
            "INCIDENT NOTIFICATION — SEC\n\
             Incident ID: {incident_id}\n\
             Aframp notifies the Securities and Exchange Commission of a technology incident. \
             No customer funds are at risk. Recovery operations are underway."
        ),
        RegulatoryBody::PartnerFi => format!(
            "PARTNER FINANCIAL INSTITUTION NOTICE\n\
             Incident ID: {incident_id}\n\
             Aframp is experiencing a service disruption. Settlement operations may be delayed. \
             We will notify you immediately upon restoration. ETA: < 15 minutes."
        ),
        RegulatoryBody::Internal => format!(
            "INTERNAL ERT ACTIVATION\n\
             Incident ID: {incident_id}\n\
             Emergency Response Team activated. All ERT members report to incident bridge."
        ),
    }
}
