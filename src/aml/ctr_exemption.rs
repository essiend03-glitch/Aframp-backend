//! CTR Exemption Management Service
//!
//! Manages CTR exemptions for subjects that qualify for reporting exemptions.
//! Checks for active exemptions before CTR generation and alerts on expiring exemptions.

use super::models::CtrExemption;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::{info, warn};
use uuid::Uuid;

/// Configuration for exemption expiry alerts
#[derive(Debug, Clone)]
pub struct CtrExemptionConfig {
    /// Days before expiry to trigger alert
    pub expiry_alert_days: i64,
}

impl Default for CtrExemptionConfig {
    fn default() -> Self {
        Self {
            expiry_alert_days: 30, // Alert 30 days before expiry
        }
    }
}

/// Request to create a new exemption
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CreateExemptionRequest {
    pub subject_id: Uuid,
    pub exemption_category: String,
    pub exemption_basis: String,
    pub expiry_date: Option<DateTime<Utc>>,
}

/// Exemption with additional metadata
#[derive(Debug, Clone, Serialize)]
pub struct ExemptionWithStatus {
    pub subject_id: Uuid,
    pub exemption_category: String,
    pub exemption_basis: String,
    pub expiry_date: Option<DateTime<Utc>>,
    pub is_active: bool,
    pub days_until_expiry: Option<i64>,
    pub created_at: DateTime<Utc>,
}

/// Result of an exemption check
#[derive(Debug, Clone)]
pub struct ExemptionCheckResult {
    pub is_exempt: bool,
    pub exemption: Option<CtrExemption>,
    pub expiring_soon: bool,
    pub days_until_expiry: Option<i64>,
}

/// CTR Exemption Service
pub struct CtrExemptionService {
    pool: PgPool,
    config: CtrExemptionConfig,
}

impl CtrExemptionService {
    pub fn new(pool: PgPool, config: CtrExemptionConfig) -> Self {
        Self { pool, config }
    }

    /// Create a new exemption
    pub async fn create_exemption(
        &self,
        request: CreateExemptionRequest,
    ) -> Result<CtrExemption, anyhow::Error> {
        // Check if exemption already exists for this subject
        let existing = self.get_active_exemption(request.subject_id).await?;
        if existing.is_some() {
            return Err(anyhow::anyhow!(
                "Active exemption already exists for subject {}",
                request.subject_id
            ));
        }

        let exemption = sqlx::query_as::<_, CtrExemption>(
            r#"
            INSERT INTO ctr_exemptions
                (subject_id, exemption_category, exemption_basis, expiry_date, created_at)
            VALUES ($1, $2, $3, $4, NOW())
            RETURNING subject_id, exemption_category, exemption_basis, expiry_date
            "#,
        )
        .bind(request.subject_id)
        .bind(&request.exemption_category)
        .bind(&request.exemption_basis)
        .bind(request.expiry_date)
        .fetch_one(&self.pool)
        .await?;

        info!(
            subject_id = %request.subject_id,
            category = %request.exemption_category,
            expiry_date = ?request.expiry_date,
            "CTR exemption created"
        );

        Ok(exemption)
    }

    /// Get all exemptions with status information
    pub async fn get_all_exemptions(&self) -> Result<Vec<ExemptionWithStatus>, anyhow::Error> {
        let exemptions = sqlx::query!(
            r#"
            SELECT subject_id, exemption_category, exemption_basis, expiry_date, created_at
            FROM ctr_exemptions
            ORDER BY created_at DESC
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        let now = Utc::now();
        let mut result = Vec::new();

        for exemption in exemptions {
            let (is_active, days_until_expiry) = if let Some(expiry) = exemption.expiry_date {
                let days = (expiry - now).num_days();
                (days >= 0, Some(days))
            } else {
                (true, None) // No expiry date means perpetual exemption
            };

            result.push(ExemptionWithStatus {
                subject_id: exemption.subject_id,
                exemption_category: exemption.exemption_category,
                exemption_basis: exemption.exemption_basis,
                expiry_date: exemption.expiry_date,
                is_active,
                days_until_expiry,
                created_at: exemption.created_at,
            });
        }

        Ok(result)
    }

    /// Get active exemption for a subject
    pub async fn get_active_exemption(
        &self,
        subject_id: Uuid,
    ) -> Result<Option<CtrExemption>, anyhow::Error> {
        let exemption = sqlx::query_as::<_, CtrExemption>(
            r#"
            SELECT subject_id, exemption_category, exemption_basis, expiry_date
            FROM ctr_exemptions
            WHERE subject_id = $1
              AND (expiry_date IS NULL OR expiry_date > NOW())
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(subject_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(exemption)
    }

    /// Delete an exemption
    pub async fn delete_exemption(&self, subject_id: Uuid) -> Result<bool, anyhow::Error> {
        let result = sqlx::query(
            r#"
            DELETE FROM ctr_exemptions
            WHERE subject_id = $1
            "#,
        )
        .bind(subject_id)
        .execute(&self.pool)
        .await?;

        let deleted = result.rows_affected() > 0;

        if deleted {
            info!(
                subject_id = %subject_id,
                "CTR exemption deleted"
            );
        }

        Ok(deleted)
    }

    /// Check if a subject is exempt from CTR reporting
    ///
    /// This method:
    /// 1. Checks for active exemption
    /// 2. Logs the exemption check
    /// 3. Alerts if exemption is expiring soon
    pub async fn check_exemption(
        &self,
        subject_id: Uuid,
    ) -> Result<ExemptionCheckResult, anyhow::Error> {
        let exemption = self.get_active_exemption(subject_id).await?;

        let (is_exempt, expiring_soon, days_until_expiry) = if let Some(ref ex) = exemption {
            let (expiring, days) = if let Some(expiry) = ex.expiry_date {
                let days = (expiry - Utc::now()).num_days();
                let expiring = days <= self.config.expiry_alert_days && days >= 0;
                (expiring, Some(days))
            } else {
                (false, None)
            };

            (true, expiring, days)
        } else {
            (false, false, None)
        };

        // Log exemption check
        info!(
            subject_id = %subject_id,
            is_exempt = is_exempt,
            exemption_category = ?exemption.as_ref().map(|e| &e.exemption_category),
            expiry_date = ?exemption.as_ref().and_then(|e| e.expiry_date),
            "CTR exemption check performed"
        );

        // Alert if exemption is expiring soon
        if expiring_soon {
            if let Some(ref ex) = exemption {
                warn!(
                    subject_id = %subject_id,
                    exemption_category = %ex.exemption_category,
                    days_until_expiry = ?days_until_expiry,
                    expiry_date = ?ex.expiry_date,
                    "CTR exemption approaching expiry"
                );
            }
        }

        Ok(ExemptionCheckResult {
            is_exempt,
            exemption,
            expiring_soon,
            days_until_expiry,
        })
    }

    /// Get all exemptions expiring within the configured alert window
    pub async fn get_expiring_exemptions(&self) -> Result<Vec<ExemptionWithStatus>, anyhow::Error> {
        let alert_date = Utc::now() + Duration::days(self.config.expiry_alert_days);

        let exemptions = sqlx::query!(
            r#"
            SELECT subject_id, exemption_category, exemption_basis, expiry_date, created_at
            FROM ctr_exemptions
            WHERE expiry_date IS NOT NULL
              AND expiry_date > NOW()
              AND expiry_date <= $1
            ORDER BY expiry_date ASC
            "#,
            alert_date
        )
        .fetch_all(&self.pool)
        .await?;

        let now = Utc::now();
        let mut result = Vec::new();

        for exemption in exemptions {
            let days_until_expiry = exemption
                .expiry_date
                .map(|expiry| (expiry - now).num_days());

            result.push(ExemptionWithStatus {
                subject_id: exemption.subject_id,
                exemption_category: exemption.exemption_category,
                exemption_basis: exemption.exemption_basis,
                expiry_date: exemption.expiry_date,
                is_active: true,
                days_until_expiry,
                created_at: exemption.created_at,
            });
        }

        Ok(result)
    }

    /// Clean up expired exemptions (for maintenance tasks)
    pub async fn cleanup_expired_exemptions(&self) -> Result<u64, anyhow::Error> {
        let result = sqlx::query(
            r#"
            DELETE FROM ctr_exemptions
            WHERE expiry_date IS NOT NULL
              AND expiry_date < NOW()
            "#,
        )
        .execute(&self.pool)
        .await?;

        let deleted = result.rows_affected();

        if deleted > 0 {
            info!(
                deleted_count = deleted,
                "Expired CTR exemptions cleaned up"
            );
        }

        Ok(deleted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = CtrExemptionConfig::default();
        assert_eq!(config.expiry_alert_days, 30);
    }
}
