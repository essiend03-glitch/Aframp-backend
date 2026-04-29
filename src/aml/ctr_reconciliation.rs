//! CTR Reconciliation and Reporting Service
//!
//! Handles CTR reconciliation with transaction data and generates monthly activity reports.

use super::models::{Ctr, CtrStatus, CtrTransaction};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tracing::{info, warn};
use uuid::Uuid;

/// Reconciliation request
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ReconciliationRequest {
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
}

/// Reconciliation discrepancy
#[derive(Debug, Clone, Serialize)]
pub struct ReconciliationDiscrepancy {
    pub ctr_id: Uuid,
    pub subject_name: String,
    pub discrepancy_type: String,
    pub expected_value: String,
    pub actual_value: String,
    pub details: String,
}

/// Reconciliation result
#[derive(Debug, Clone, Serialize)]
pub struct ReconciliationResult {
    pub reconciliation_id: Uuid,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub total_ctrs_checked: usize,
    pub ctrs_with_discrepancies: usize,
    pub discrepancies: Vec<ReconciliationDiscrepancy>,
    pub reconciled_at: DateTime<Utc>,
}

/// Monthly CTR activity report
#[derive(Debug, Clone, Serialize)]
pub struct MonthlyActivityReport {
    pub report_id: Uuid,
    pub year: i32,
    pub month: u32,
    pub total_ctrs_generated: usize,
    pub total_ctrs_filed: usize,
    pub total_ctrs_overdue: usize,
    pub total_amount_reported: Decimal,
    pub ctrs_by_status: StatusBreakdown,
    pub ctrs_by_type: TypeBreakdown,
    pub top_subjects: Vec<SubjectSummary>,
    pub filing_performance: FilingPerformance,
    pub generated_at: DateTime<Utc>,
}

/// CTR status breakdown
#[derive(Debug, Clone, Serialize)]
pub struct StatusBreakdown {
    pub draft: usize,
    pub under_review: usize,
    pub approved: usize,
    pub filed: usize,
    pub acknowledged: usize,
    pub rejected: usize,
}

/// CTR type breakdown
#[derive(Debug, Clone, Serialize)]
pub struct TypeBreakdown {
    pub individual: usize,
    pub corporate: usize,
}

/// Subject summary for top subjects
#[derive(Debug, Clone, Serialize)]
pub struct SubjectSummary {
    pub subject_name: String,
    pub ctr_count: usize,
    pub total_amount: Decimal,
}

/// Filing performance metrics
#[derive(Debug, Clone, Serialize)]
pub struct FilingPerformance {
    pub filed_on_time: usize,
    pub filed_late: usize,
    pub average_days_to_file: f64,
}

/// CTR Reconciliation Service
pub struct CtrReconciliationService {
    pool: PgPool,
}

impl CtrReconciliationService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Reconcile CTRs with transaction data
    pub async fn reconcile(
        &self,
        request: ReconciliationRequest,
    ) -> Result<ReconciliationResult, anyhow::Error> {
        let reconciliation_id = Uuid::new_v4();

        info!(
            reconciliation_id = %reconciliation_id,
            start_date = %request.start_date,
            end_date = %request.end_date,
            "Starting CTR reconciliation"
        );

        // Get all CTRs in the date range
        let ctrs = self.get_ctrs_in_range(request.start_date, request.end_date).await?;

        let mut discrepancies = Vec::new();

        for ctr in &ctrs {
            // Get transactions for this CTR
            let transactions = self.get_ctr_transactions(ctr.ctr_id).await?;

            // Check transaction count
            if transactions.len() != ctr.transaction_count as usize {
                discrepancies.push(ReconciliationDiscrepancy {
                    ctr_id: ctr.ctr_id,
                    subject_name: ctr.subject_full_name.clone(),
                    discrepancy_type: "transaction_count_mismatch".to_string(),
                    expected_value: ctr.transaction_count.to_string(),
                    actual_value: transactions.len().to_string(),
                    details: format!(
                        "CTR reports {} transactions but {} found in database",
                        ctr.transaction_count,
                        transactions.len()
                    ),
                });
            }

            // Check total amount
            let actual_total: Decimal = transactions.iter().map(|t| t.transaction_amount_ngn).sum();
            if actual_total != ctr.total_transaction_amount {
                discrepancies.push(ReconciliationDiscrepancy {
                    ctr_id: ctr.ctr_id,
                    subject_name: ctr.subject_full_name.clone(),
                    discrepancy_type: "amount_mismatch".to_string(),
                    expected_value: ctr.total_transaction_amount.to_string(),
                    actual_value: actual_total.to_string(),
                    details: format!(
                        "CTR reports {} NGN but transactions sum to {} NGN",
                        ctr.total_transaction_amount, actual_total
                    ),
                });
            }

            // Check transaction references
            let actual_refs: Vec<String> = transactions
                .iter()
                .map(|t| t.transaction_id.to_string())
                .collect();
            let expected_refs = &ctr.transaction_references;

            if actual_refs.len() != expected_refs.len() {
                discrepancies.push(ReconciliationDiscrepancy {
                    ctr_id: ctr.ctr_id,
                    subject_name: ctr.subject_full_name.clone(),
                    discrepancy_type: "reference_count_mismatch".to_string(),
                    expected_value: expected_refs.len().to_string(),
                    actual_value: actual_refs.len().to_string(),
                    details: "Transaction reference count does not match".to_string(),
                });
            }
        }

        let ctrs_with_discrepancies = discrepancies
            .iter()
            .map(|d| d.ctr_id)
            .collect::<std::collections::HashSet<_>>()
            .len();

        info!(
            reconciliation_id = %reconciliation_id,
            total_ctrs = ctrs.len(),
            ctrs_with_discrepancies = ctrs_with_discrepancies,
            total_discrepancies = discrepancies.len(),
            "CTR reconciliation completed"
        );

        Ok(ReconciliationResult {
            reconciliation_id,
            start_date: request.start_date,
            end_date: request.end_date,
            total_ctrs_checked: ctrs.len(),
            ctrs_with_discrepancies,
            discrepancies,
            reconciled_at: Utc::now(),
        })
    }

    /// Generate monthly activity report
    pub async fn generate_monthly_report(
        &self,
        year: i32,
        month: u32,
    ) -> Result<MonthlyActivityReport, anyhow::Error> {
        let report_id = Uuid::new_v4();

        info!(
            report_id = %report_id,
            year = year,
            month = month,
            "Generating monthly CTR activity report"
        );

        // Get date range for the month
        let start_date = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
        let end_date = if month == 12 {
            NaiveDate::from_ymd_opt(year + 1, 1, 1).unwrap()
        } else {
            NaiveDate::from_ymd_opt(year, month + 1, 1).unwrap()
        };

        // Get all CTRs created in this month
        let ctrs = self.get_ctrs_in_range(start_date, end_date).await?;

        // Calculate metrics
        let total_ctrs_generated = ctrs.len();
        let total_ctrs_filed = ctrs.iter().filter(|c| c.status == CtrStatus::Filed || c.status == CtrStatus::Acknowledged).count();
        let total_ctrs_overdue = ctrs.iter().filter(|c| {
            if let Some(deadline) = c.filing_timestamp {
                deadline < Utc::now() && c.status != CtrStatus::Filed && c.status != CtrStatus::Acknowledged
            } else {
                false
            }
        }).count();

        let total_amount_reported: Decimal = ctrs.iter().map(|c| c.total_transaction_amount).sum();

        // Status breakdown
        let ctrs_by_status = StatusBreakdown {
            draft: ctrs.iter().filter(|c| c.status == CtrStatus::Draft).count(),
            under_review: ctrs.iter().filter(|c| c.status == CtrStatus::UnderReview).count(),
            approved: ctrs.iter().filter(|c| c.status == CtrStatus::Approved).count(),
            filed: ctrs.iter().filter(|c| c.status == CtrStatus::Filed).count(),
            acknowledged: ctrs.iter().filter(|c| c.status == CtrStatus::Acknowledged).count(),
            rejected: ctrs.iter().filter(|c| c.status == CtrStatus::Rejected).count(),
        };

        // Type breakdown
        let ctrs_by_type = TypeBreakdown {
            individual: ctrs.iter().filter(|c| c.ctr_type == super::models::CtrType::Individual).count(),
            corporate: ctrs.iter().filter(|c| c.ctr_type == super::models::CtrType::Corporate).count(),
        };

        // Top subjects
        let mut subject_map: std::collections::HashMap<String, (usize, Decimal)> = std::collections::HashMap::new();
        for ctr in &ctrs {
            let entry = subject_map.entry(ctr.subject_full_name.clone()).or_insert((0, Decimal::ZERO));
            entry.0 += 1;
            entry.1 += ctr.total_transaction_amount;
        }

        let mut top_subjects: Vec<SubjectSummary> = subject_map
            .into_iter()
            .map(|(name, (count, amount))| SubjectSummary {
                subject_name: name,
                ctr_count: count,
                total_amount: amount,
            })
            .collect();
        top_subjects.sort_by(|a, b| b.total_amount.cmp(&a.total_amount));
        top_subjects.truncate(10);

        // Filing performance
        let filed_ctrs: Vec<&Ctr> = ctrs.iter().filter(|c| c.status == CtrStatus::Filed || c.status == CtrStatus::Acknowledged).collect();
        let filed_on_time = filed_ctrs.iter().filter(|c| {
            // Simplified: assume filed on time if filed
            true
        }).count();
        let filed_late = filed_ctrs.len() - filed_on_time;
        let average_days_to_file = 7.5; // Placeholder

        let filing_performance = FilingPerformance {
            filed_on_time,
            filed_late,
            average_days_to_file,
        };

        info!(
            report_id = %report_id,
            total_ctrs = total_ctrs_generated,
            filed = total_ctrs_filed,
            overdue = total_ctrs_overdue,
            "Monthly report generated"
        );

        Ok(MonthlyActivityReport {
            report_id,
            year,
            month,
            total_ctrs_generated,
            total_ctrs_filed,
            total_ctrs_overdue,
            total_amount_reported,
            ctrs_by_status,
            ctrs_by_type,
            top_subjects,
            filing_performance,
            generated_at: Utc::now(),
        })
    }

    /// Get CTRs in date range
    async fn get_ctrs_in_range(
        &self,
        start_date: NaiveDate,
        end_date: NaiveDate,
    ) -> Result<Vec<Ctr>, anyhow::Error> {
        let start_datetime = start_date.and_hms_opt(0, 0, 0).unwrap().and_utc();
        let end_datetime = end_date.and_hms_opt(0, 0, 0).unwrap().and_utc();

        let ctrs = sqlx::query_as::<_, Ctr>(
            r#"
            SELECT ctr_id, reporting_period, ctr_type, subject_kyc_id, subject_full_name,
                   subject_identification, subject_address, total_transaction_amount,
                   transaction_count, transaction_references, detection_method, status,
                   assigned_compliance_officer, filing_timestamp, regulatory_reference_number
            FROM ctrs
            WHERE reporting_period >= $1 AND reporting_period < $2
            ORDER BY reporting_period ASC
            "#,
        )
        .bind(start_datetime)
        .bind(end_datetime)
        .fetch_all(&self.pool)
        .await?;

        Ok(ctrs)
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reconciliation_request() {
        let request = ReconciliationRequest {
            start_date: NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2024, 1, 31).unwrap(),
        };
        assert_eq!(request.start_date.month(), 1);
    }
}
