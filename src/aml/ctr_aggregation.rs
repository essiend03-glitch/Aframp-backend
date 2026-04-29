//! CTR Transaction Aggregation Service
//!
//! Maintains rolling daily aggregation windows per subject (00:00–23:59 WAT),
//! computes NGN equivalents at confirmation time, updates subject's daily total
//! on every confirmed transaction, and triggers threshold breach flags.

use super::ctr_generator::{CtrGeneratorService, CtrGenerationResult};
use super::ctr_logging;
use super::ctr_metrics;
use super::models::{CtrAggregation, CtrType};
use chrono::{DateTime, Duration, NaiveDate, TimeZone, Utc};
use chrono_tz::Africa::Lagos as WAT;
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::str::FromStr;
use std::sync::Arc;
use tracing::{error, info, warn};
use uuid::Uuid;

/// Configuration for CTR aggregation thresholds
#[derive(Debug, Clone)]
pub struct CtrAggregationConfig {
    /// Threshold for individual subjects (NGN)
    pub individual_threshold: Decimal,
    /// Threshold for corporate subjects (NGN)
    pub corporate_threshold: Decimal,
    /// Proximity percentage to threshold for early warning (0.0-1.0)
    pub proximity_threshold: Decimal,
}

impl Default for CtrAggregationConfig {
    fn default() -> Self {
        Self {
            individual_threshold: Decimal::from_str("5000000").unwrap(), // NGN 5M
            corporate_threshold: Decimal::from_str("10000000").unwrap(), // NGN 10M
            proximity_threshold: Decimal::from_str("0.9").unwrap(),      // 90%
        }
    }
}

/// Result of an aggregation update
#[derive(Debug, Clone)]
pub struct AggregationUpdateResult {
    pub subject_id: Uuid,
    pub new_running_total: Decimal,
    pub transaction_count: i32,
    pub threshold_breached: bool,
    pub proximity_warning: bool,
    pub applicable_threshold: Decimal,
    pub ctr_generated: Option<CtrGenerationResult>,
}

/// CTR Transaction Aggregation Service
pub struct CtrAggregationService {
    pool: PgPool,
    config: CtrAggregationConfig,
    ctr_generator: Option<Arc<CtrGeneratorService>>,
}

impl CtrAggregationService {
    pub fn new(pool: PgPool, config: CtrAggregationConfig) -> Self {
        Self {
            pool,
            config,
            ctr_generator: None,
        }
    }

    /// Create a new service with CTR auto-generation enabled
    pub fn with_ctr_generator(
        pool: PgPool,
        config: CtrAggregationConfig,
        ctr_generator: Arc<CtrGeneratorService>,
    ) -> Self {
        Self {
            pool,
            config,
            ctr_generator: Some(ctr_generator),
        }
    }

    /// Process a confirmed transaction and update the subject's daily aggregation
    ///
    /// # Arguments
    /// * `subject_id` - The KYC ID of the subject
    /// * `subject_type` - Whether the subject is Individual or Corporate
    /// * `transaction_id` - The transaction ID
    /// * `transaction_amount_ngn` - The transaction amount in NGN
    /// * `transaction_timestamp` - When the transaction was confirmed
    pub async fn process_transaction(
        &self,
        subject_id: Uuid,
        subject_type: CtrType,
        transaction_id: Uuid,
        transaction_amount_ngn: Decimal,
        transaction_timestamp: DateTime<Utc>,
    ) -> Result<AggregationUpdateResult, anyhow::Error> {
        // Get the WAT day boundaries for this transaction
        let (window_start, window_end) = self.get_wat_day_boundaries(transaction_timestamp);

        info!(
            subject_id = %subject_id,
            transaction_id = %transaction_id,
            amount_ngn = %transaction_amount_ngn,
            window_start = %window_start,
            window_end = %window_end,
            "Processing transaction for CTR aggregation"
        );

        // Get or create aggregation record for this subject and day
        let mut aggregation = self
            .get_or_create_aggregation(subject_id, window_start, window_end)
            .await?;

        // Update aggregation with new transaction
        aggregation.running_total_amount += transaction_amount_ngn;
        aggregation.transaction_count += 1;
        aggregation.transaction_amounts.push(transaction_amount_ngn);
        aggregation.transaction_timestamps.push(transaction_timestamp);

        // Determine applicable threshold based on subject type
        let applicable_threshold = match subject_type {
            CtrType::Individual => self.config.individual_threshold,
            CtrType::Corporate => self.config.corporate_threshold,
        };

        // Check for threshold breach
        let threshold_breached = aggregation.running_total_amount >= applicable_threshold;
        aggregation.threshold_breach_flag = threshold_breached;

        // Check for proximity warning
        let proximity_amount = applicable_threshold * self.config.proximity_threshold;
        let proximity_warning = aggregation.running_total_amount >= proximity_amount
            && !threshold_breached;

        // Persist updated aggregation
        self.update_aggregation(&aggregation).await?;

        // Log the update
        info!(
            subject_id = %subject_id,
            transaction_id = %transaction_id,
            running_total = %aggregation.running_total_amount,
            transaction_count = aggregation.transaction_count,
            threshold = %applicable_threshold,
            threshold_breached = threshold_breached,
            proximity_warning = proximity_warning,
            "CTR aggregation updated"
        );

        // Log threshold breach
        if threshold_breached {
            warn!(
                subject_id = %subject_id,
                subject_type = ?subject_type,
                running_total = %aggregation.running_total_amount,
                threshold = %applicable_threshold,
                transaction_count = aggregation.transaction_count,
                "CTR threshold breached"
            );

            // Record metrics
            let subject_type_str = match subject_type {
                CtrType::Individual => "individual",
                CtrType::Corporate => "corporate",
            };
            ctr_metrics::record_threshold_breach(subject_type_str);

            // Log structured event
            ctr_logging::log_threshold_breach(
                subject_id,
                "Subject Name".to_string(), // Would fetch from KYC in production
                aggregation.running_total_amount.to_string(),
                applicable_threshold.to_string(),
                subject_type_str.to_string(),
            );
        }

        // Log proximity warning
        if proximity_warning {
            warn!(
                subject_id = %subject_id,
                subject_type = ?subject_type,
                running_total = %aggregation.running_total_amount,
                threshold = %applicable_threshold,
                proximity_percentage = %(self.config.proximity_threshold * Decimal::from(100)),
                "Subject approaching CTR threshold"
            );
        }

        // Auto-generate CTR if threshold breached and generator is configured
        let ctr_generated = if threshold_breached {
            if let Some(generator) = &self.ctr_generator {
                match generator
                    .generate_ctr_on_breach(
                        subject_id,
                        window_start,
                        window_end,
                        aggregation.running_total_amount,
                        aggregation.transaction_count,
                        None, // Use default compliance officer from config
                    )
                    .await
                {
                    Ok(result) => {
                        info!(
                            ctr_id = %result.ctr_id,
                            subject_id = %subject_id,
                            already_existed = result.already_existed,
                            "CTR auto-generation completed"
                        );
                        Some(result)
                    }
                    Err(e) => {
                        error!(
                            error = %e,
                            subject_id = %subject_id,
                            "Failed to auto-generate CTR"
                        );
                        None
                    }
                }
            } else {
                warn!(
                    subject_id = %subject_id,
                    "Threshold breached but CTR generator not configured"
                );
                None
            }
        } else {
            None
        };

        Ok(AggregationUpdateResult {
            subject_id,
            new_running_total: aggregation.running_total_amount,
            transaction_count: aggregation.transaction_count,
            threshold_breached,
            proximity_warning,
            applicable_threshold,
            ctr_generated,
        })
    }

    /// Get the WAT (West Africa Time) day boundaries for a given timestamp
    ///
    /// Returns (start, end) where start is 00:00:00 WAT and end is 23:59:59.999 WAT
    fn get_wat_day_boundaries(&self, timestamp: DateTime<Utc>) -> (DateTime<Utc>, DateTime<Utc>) {
        // Convert UTC timestamp to WAT
        let wat_time = timestamp.with_timezone(&WAT);
        
        // Get the date in WAT
        let wat_date = wat_time.date_naive();
        
        // Create start of day in WAT (00:00:00)
        let start_wat = WAT
            .from_local_datetime(&wat_date.and_hms_opt(0, 0, 0).unwrap())
            .single()
            .unwrap();
        
        // Create end of day in WAT (23:59:59.999)
        let end_wat = WAT
            .from_local_datetime(&wat_date.and_hms_milli_opt(23, 59, 59, 999).unwrap())
            .single()
            .unwrap();
        
        // Convert back to UTC
        (start_wat.with_timezone(&Utc), end_wat.with_timezone(&Utc))
    }

    /// Get existing aggregation or create a new one
    async fn get_or_create_aggregation(
        &self,
        subject_id: Uuid,
        window_start: DateTime<Utc>,
        window_end: DateTime<Utc>,
    ) -> Result<CtrAggregation, anyhow::Error> {
        // Try to fetch existing aggregation
        let existing = sqlx::query_as::<_, CtrAggregation>(
            r#"
            SELECT subject_id, aggregation_window_start, aggregation_window_end,
                   running_total_amount, transaction_count, transaction_amounts,
                   transaction_timestamps, threshold_breach_flag
            FROM ctr_aggregations
            WHERE subject_id = $1
              AND aggregation_window_start = $2
              AND aggregation_window_end = $3
            "#,
        )
        .bind(subject_id)
        .bind(window_start)
        .bind(window_end)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(agg) = existing {
            Ok(agg)
        } else {
            // Create new aggregation record
            let new_agg = CtrAggregation {
                subject_id,
                aggregation_window_start: window_start,
                aggregation_window_end: window_end,
                running_total_amount: Decimal::ZERO,
                transaction_count: 0,
                transaction_amounts: Vec::new(),
                transaction_timestamps: Vec::new(),
                threshold_breach_flag: false,
            };

            // Insert into database
            sqlx::query(
                r#"
                INSERT INTO ctr_aggregations
                    (subject_id, aggregation_window_start, aggregation_window_end,
                     running_total_amount, transaction_count, transaction_amounts,
                     transaction_timestamps, threshold_breach_flag)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
                "#,
            )
            .bind(subject_id)
            .bind(window_start)
            .bind(window_end)
            .bind(Decimal::ZERO)
            .bind(0)
            .bind(&new_agg.transaction_amounts)
            .bind(&new_agg.transaction_timestamps)
            .bind(false)
            .execute(&self.pool)
            .await?;

            info!(
                subject_id = %subject_id,
                window_start = %window_start,
                window_end = %window_end,
                "Created new CTR aggregation record"
            );

            Ok(new_agg)
        }
    }

    /// Update an existing aggregation record
    async fn update_aggregation(&self, aggregation: &CtrAggregation) -> Result<(), anyhow::Error> {
        sqlx::query(
            r#"
            UPDATE ctr_aggregations
            SET running_total_amount = $4,
                transaction_count = $5,
                transaction_amounts = $6,
                transaction_timestamps = $7,
                threshold_breach_flag = $8
            WHERE subject_id = $1
              AND aggregation_window_start = $2
              AND aggregation_window_end = $3
            "#,
        )
        .bind(aggregation.subject_id)
        .bind(aggregation.aggregation_window_start)
        .bind(aggregation.aggregation_window_end)
        .bind(aggregation.running_total_amount)
        .bind(aggregation.transaction_count)
        .bind(&aggregation.transaction_amounts)
        .bind(&aggregation.transaction_timestamps)
        .bind(aggregation.threshold_breach_flag)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    /// Get current aggregation for a subject on a specific date
    pub async fn get_aggregation_for_date(
        &self,
        subject_id: Uuid,
        date: NaiveDate,
    ) -> Result<Option<CtrAggregation>, anyhow::Error> {
        // Convert date to WAT day boundaries
        let start_wat = WAT
            .from_local_datetime(&date.and_hms_opt(0, 0, 0).unwrap())
            .single()
            .unwrap();
        let end_wat = WAT
            .from_local_datetime(&date.and_hms_milli_opt(23, 59, 59, 999).unwrap())
            .single()
            .unwrap();

        let window_start = start_wat.with_timezone(&Utc);
        let window_end = end_wat.with_timezone(&Utc);

        let aggregation = sqlx::query_as::<_, CtrAggregation>(
            r#"
            SELECT subject_id, aggregation_window_start, aggregation_window_end,
                   running_total_amount, transaction_count, transaction_amounts,
                   transaction_timestamps, threshold_breach_flag
            FROM ctr_aggregations
            WHERE subject_id = $1
              AND aggregation_window_start = $2
              AND aggregation_window_end = $3
            "#,
        )
        .bind(subject_id)
        .bind(window_start)
        .bind(window_end)
        .fetch_optional(&self.pool)
        .await?;

        Ok(aggregation)
    }

    /// Get all subjects that have breached the threshold on a specific date
    pub async fn get_threshold_breaches_for_date(
        &self,
        date: NaiveDate,
    ) -> Result<Vec<CtrAggregation>, anyhow::Error> {
        // Convert date to WAT day boundaries
        let start_wat = WAT
            .from_local_datetime(&date.and_hms_opt(0, 0, 0).unwrap())
            .single()
            .unwrap();
        let end_wat = WAT
            .from_local_datetime(&date.and_hms_milli_opt(23, 59, 59, 999).unwrap())
            .single()
            .unwrap();

        let window_start = start_wat.with_timezone(&Utc);
        let window_end = end_wat.with_timezone(&Utc);

        let breaches = sqlx::query_as::<_, CtrAggregation>(
            r#"
            SELECT subject_id, aggregation_window_start, aggregation_window_end,
                   running_total_amount, transaction_count, transaction_amounts,
                   transaction_timestamps, threshold_breach_flag
            FROM ctr_aggregations
            WHERE aggregation_window_start = $1
              AND aggregation_window_end = $2
              AND threshold_breach_flag = true
            ORDER BY running_total_amount DESC
            "#,
        )
        .bind(window_start)
        .bind(window_end)
        .fetch_all(&self.pool)
        .await?;

        Ok(breaches)
    }

    /// Get subjects within proximity of threshold for a specific date
    pub async fn get_proximity_warnings_for_date(
        &self,
        date: NaiveDate,
        subject_type: CtrType,
    ) -> Result<Vec<CtrAggregation>, anyhow::Error> {
        // Convert date to WAT day boundaries
        let start_wat = WAT
            .from_local_datetime(&date.and_hms_opt(0, 0, 0).unwrap())
            .single()
            .unwrap();
        let end_wat = WAT
            .from_local_datetime(&date.and_hms_milli_opt(23, 59, 59, 999).unwrap())
            .single()
            .unwrap();

        let window_start = start_wat.with_timezone(&Utc);
        let window_end = end_wat.with_timezone(&Utc);

        // Determine threshold based on subject type
        let threshold = match subject_type {
            CtrType::Individual => self.config.individual_threshold,
            CtrType::Corporate => self.config.corporate_threshold,
        };

        let proximity_amount = threshold * self.config.proximity_threshold;

        let warnings = sqlx::query_as::<_, CtrAggregation>(
            r#"
            SELECT subject_id, aggregation_window_start, aggregation_window_end,
                   running_total_amount, transaction_count, transaction_amounts,
                   transaction_timestamps, threshold_breach_flag
            FROM ctr_aggregations
            WHERE aggregation_window_start = $1
              AND aggregation_window_end = $2
              AND running_total_amount >= $3
              AND threshold_breach_flag = false
            ORDER BY running_total_amount DESC
            "#,
        )
        .bind(window_start)
        .bind(window_end)
        .bind(proximity_amount)
        .fetch_all(&self.pool)
        .await?;

        Ok(warnings)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDateTime;

    #[test]
    fn test_wat_day_boundaries() {
        let service = CtrAggregationService::new(
            PgPool::connect_lazy("postgresql://localhost/test").unwrap(),
            CtrAggregationConfig::default(),
        );

        // Test with a UTC timestamp that falls on 2024-01-15 in WAT
        // WAT is UTC+1, so 2024-01-15 00:30 UTC is 2024-01-15 01:30 WAT
        let timestamp = Utc.with_ymd_and_hms(2024, 1, 15, 0, 30, 0).unwrap();
        let (start, end) = service.get_wat_day_boundaries(timestamp);

        // Start should be 2024-01-14 23:00:00 UTC (2024-01-15 00:00:00 WAT)
        assert_eq!(start, Utc.with_ymd_and_hms(2024, 1, 14, 23, 0, 0).unwrap());
        
        // End should be 2024-01-15 22:59:59.999 UTC (2024-01-15 23:59:59.999 WAT)
        assert_eq!(end, Utc.with_ymd_and_hms(2024, 1, 15, 22, 59, 59).unwrap().with_nanosecond(999_000_000).unwrap());
    }

    #[test]
    fn test_default_config() {
        let config = CtrAggregationConfig::default();
        assert_eq!(config.individual_threshold, Decimal::from_str("5000000").unwrap());
        assert_eq!(config.corporate_threshold, Decimal::from_str("10000000").unwrap());
        assert_eq!(config.proximity_threshold, Decimal::from_str("0.9").unwrap());
    }
}
