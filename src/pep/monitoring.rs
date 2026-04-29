//! Continuous PEP monitoring service — nightly re-screening of the customer base

use super::models::{PepMatchStatus, PepScreeningRequest};
use super::repository::PepRepository;
use super::screening::PepScreeningService;
use std::sync::Arc;
use tracing::{error, info, warn};
use uuid::Uuid;

pub struct PepMonitoringService {
    screening: Arc<PepScreeningService>,
    repo: Arc<PepRepository>,
}

impl PepMonitoringService {
    pub fn new(screening: Arc<PepScreeningService>, repo: Arc<PepRepository>) -> Self {
        Self { screening, repo }
    }

    /// Re-screen all active consumers who have not been screened in the last 24 hours.
    /// Called by the nightly worker.
    pub async fn run_nightly_rescreening(&self) -> Result<RescreeningSummary, anyhow::Error> {
        info!("Starting nightly PEP re-screening cycle");

        let consumers = self.repo.fetch_consumers_for_rescreening().await?;
        let total = consumers.len();
        let mut screened = 0;
        let mut new_matches = 0;
        let mut status_changes = 0;

        for consumer in consumers {
            match self.rescreen_consumer(&consumer).await {
                Ok(result) => {
                    screened += 1;
                    if result.new_matches > 0 {
                        new_matches += result.new_matches;
                        warn!(
                            consumer_id = %consumer.consumer_id,
                            new_matches = result.new_matches,
                            "New PEP matches detected during re-screening"
                        );
                    }
                    if result.status_changed {
                        status_changes += 1;
                    }
                }
                Err(e) => {
                    error!(
                        consumer_id = %consumer.consumer_id,
                        error = %e,
                        "Failed to re-screen consumer"
                    );
                }
            }
        }

        info!(
            total,
            screened,
            new_matches,
            status_changes,
            "Nightly PEP re-screening cycle complete"
        );

        Ok(RescreeningSummary {
            total_consumers: total,
            screened_count: screened,
            new_matches_count: new_matches,
            status_changes_count: status_changes,
        })
    }

    async fn rescreen_consumer(
        &self,
        consumer: &ConsumerForRescreening,
    ) -> Result<RescreeningResult, anyhow::Error> {
        let req = PepScreeningRequest {
            consumer_id: consumer.consumer_id,
            full_name: consumer.full_name.clone(),
            date_of_birth: consumer.date_of_birth,
            nationality: consumer.nationality.clone(),
            country_of_residence: consumer.country_of_residence.clone(),
            is_rescreening: true,
        };

        let result = self.screening.screen(&req).await;

        // Compare with previous screening results
        let previous_matches = self.repo.fetch_matches_for_consumer(consumer.consumer_id).await?;
        let new_matches = result
            .matches
            .iter()
            .filter(|m| m.status == PepMatchStatus::PendingReview)
            .filter(|m| {
                !previous_matches
                    .iter()
                    .any(|pm| pm.matched_name == m.matched_name)
            })
            .count();

        let status_changed = result.edd_triggered;

        Ok(RescreeningResult {
            new_matches,
            status_changed,
        })
    }
}

pub struct ConsumerForRescreening {
    pub consumer_id: Uuid,
    pub full_name: String,
    pub date_of_birth: Option<chrono::NaiveDate>,
    pub nationality: Option<String>,
    pub country_of_residence: Option<String>,
}

struct RescreeningResult {
    new_matches: usize,
    status_changed: bool,
}

#[derive(Debug)]
pub struct RescreeningSummary {
    pub total_consumers: usize,
    pub screened_count: usize,
    pub new_matches_count: usize,
    pub status_changes_count: usize,
}
