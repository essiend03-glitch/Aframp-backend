//! Admin & Management Endpoints for Banking Partners

use crate::banking::integrations::{
    BankIntegration, BankPartnerStatusResponse, ReconcileRequest,
};
use crate::banking::metrics::BankingMetricsService;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

/// Banking Admin Service
pub struct BankingAdminService {
    pub metrics: Arc<BankingMetricsService>,
}

impl BankingAdminService {
    pub fn new(metrics: Arc<BankingMetricsService>) -> Self {
        Self { metrics }
    }

    /// Get status of all banking partners
    /// GET /api/v1/admin/partners/banking/status
    pub async fn get_partners_status(&self) -> Vec<BankPartnerStatusResponse> {
        // In production, would query database for all integrations
        // For now, return mock data
        vec![
            BankPartnerStatusResponse {
                partner_code: "044".to_string(),
                partner_name: "Access Bank".to_string(),
                status: "active".to_string(),
                connection_health: "healthy".to_string(),
                settlement_pool_balance: Some(rust_decimal::Decimal::from(50_000_000)),
                webhook_backlog_count: 12,
                last_health_check: Some(Utc::now()),
                api_latency_ms: Some(245),
            },
            BankPartnerStatusResponse {
                partner_code: "058".to_string(),
                partner_name: "Guaranty Trust Bank".to_string(),
                status: "active".to_string(),
                connection_health: "healthy".to_string(),
                settlement_pool_balance: Some(rust_decimal::Decimal::from(75_000_000)),
                webhook_backlog_count: 5,
                last_health_check: Some(Utc::now()),
                api_latency_ms: Some(189),
            },
            BankPartnerStatusResponse {
                partner_code: "011".to_string(),
                partner_name: "First Bank of Nigeria".to_string(),
                status: "active".to_string(),
                connection_health: "degraded".to_string(),
                settlement_pool_balance: Some(rust_decimal::Decimal::from(25_000_000)),
                webhook_backlog_count: 45,
                last_health_check: Some(Utc::now()),
                api_latency_ms: Some(1250),
            },
        ]
    }

    /// Get status of specific banking partner
    pub async fn get_partner_status(&self, partner_code: &str) -> Option<BankPartnerStatusResponse> {
        self.get_partners_status()
            .await
            .into_iter()
            .find(|p| p.partner_code == partner_code)
    }

    /// Trigger manual reconciliation
    /// POST /api/v1/admin/partners/banking/reconcile
    pub async fn trigger_reconciliation(
        &self,
        request: ReconcileRequest,
        triggered_by: Uuid,
    ) -> Result<ReconciliationResult, ReconciliationError> {
        info!(
            bank_integration_id = %request.bank_integration_id,
            triggered_by = %triggered_by,
            "Manual reconciliation triggered"
        );

        // In production, would:
        // 1. Create reconciliation job record
        // 2. Fetch bank statements via API
        // 3. Compare with internal records
        // 4. Flag discrepancies

        let job_id = Uuid::new_v4();

        Ok(ReconciliationResult {
            job_id,
            bank_integration_id: request.bank_integration_id,
            status: "completed".to_string(),
            records_checked: 1250,
            discrepancies_found: 3,
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
        })
    }

    /// Get reconciliation job status
    pub async fn get_reconciliation_status(&self, job_id: Uuid) -> Option<ReconciliationResult> {
        // Would query database
        None
    }
}

/// Reconciliation Result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReconciliationResult {
    pub job_id: Uuid,
    pub bank_integration_id: Uuid,
    pub status: String,
    pub records_checked: i32,
    pub discrepancies_found: i32,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Reconciliation Errors
#[derive(Debug, thiserror::Error)]
pub enum ReconciliationError {
    #[error("Bank integration not found")]
    NotFound,

    #[error("Reconciliation already in progress")]
    AlreadyRunning,

    #[error("Bank API error: {0}")]
    BankApiError(String),

    #[error("Internal error: {0}")]
    Internal(String),
}