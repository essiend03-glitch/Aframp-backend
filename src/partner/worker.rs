//! Deprecation notification worker.
//!
//! Runs on a daily cadence. For each API version scheduled for sunset within
//! the next 30 days that has not yet been notified, it:
//!   1. Fetches all active partners using that version.
//!   2. Logs a structured notification event per partner.
//!   3. Marks the deprecation record as notified.
//!
//! Plug in an email/webhook service by replacing the `notify_partner` stub.

use sqlx::PgPool;
use tokio::time::{interval, Duration};
use tracing::{error, info};
use uuid::Uuid;

use super::{error::PartnerError, repository::PartnerRepository};

const NOTIFY_DAYS_AHEAD: i64 = 30;
const POLL_INTERVAL_SECS: u64 = 86_400; // 24 h

pub struct DeprecationNotificationWorker {
    repo: PartnerRepository,
}

impl DeprecationNotificationWorker {
    pub fn new(pool: PgPool) -> Self {
        Self {
            repo: PartnerRepository::new(pool),
        }
    }

    pub async fn run(self) {
        let mut ticker = interval(Duration::from_secs(POLL_INTERVAL_SECS));
        loop {
            ticker.tick().await;
            if let Err(e) = self.notify_pending().await {
                error!(error=%e, "Deprecation notification worker error");
            }
        }
    }

    async fn notify_pending(&self) -> Result<(), PartnerError> {
        let deprecations = self.repo.deprecations_due_for_notification(NOTIFY_DAYS_AHEAD).await
            .map_err(PartnerError::Database)?;

        for dep in deprecations {
            // Find all active partners on this API version
            let partners = self.repo.find_partners_by_api_version(&dep.api_version).await
                .map_err(PartnerError::Database)?;

            for partner in &partners {
                notify_partner(
                    partner.id,
                    &partner.contact_email,
                    &dep.api_version,
                    dep.sunset_at,
                    dep.migration_guide_url.as_deref(),
                );
            }

            info!(
                api_version=%dep.api_version,
                sunset_at=%dep.sunset_at,
                partners_notified=partners.len(),
                "Deprecation notifications dispatched"
            );

            self.repo.mark_deprecation_notified(dep.id).await?;
        }

        Ok(())
    }
}

/// Stub notification — replace with email/webhook delivery as needed.
fn notify_partner(
    partner_id: Uuid,
    contact_email: &str,
    api_version: &str,
    sunset_at: chrono::DateTime<chrono::Utc>,
    migration_guide_url: Option<&str>,
) {
    let days_left = (sunset_at - chrono::Utc::now()).num_days().max(0);
    info!(
        partner_id=%partner_id,
        contact_email=%contact_email,
        api_version=%api_version,
        sunset_at=%sunset_at,
        days_until_sunset=days_left,
        migration_guide_url=migration_guide_url.unwrap_or("N/A"),
        "PARTNER_DEPRECATION_NOTICE: API version sunset approaching"
    );
}
