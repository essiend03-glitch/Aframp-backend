//! Profiling Admin Endpoints
//! Management interfaces for performance profiling

use crate::profiling::models::{
    ProfilingConfig, ProfilingStatusResponse, SlowEndpointAlert, SlowEndpointsResponse,
    UpdateProfilingRequest,
};
use crate::profiling::service::ProfilingService;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

/// Profiling Admin Service
pub struct ProfilingAdminService {
    profiling_service: Arc<ProfilingService>,
}

impl ProfilingAdminService {
    pub fn new(profiling_service: Arc<ProfilingService>) -> Self {
        Self { profiling_service }
    }

    /// Get slow endpoints exceeding P95/P99 thresholds
    /// GET /api/v1/admin/infra/profile/slow-endpoints
    pub async fn get_slow_endpoints(&self) -> SlowEndpointsResponse {
        let endpoints = self.profiling_service.get_slow_endpoints().await;

        SlowEndpointsResponse {
            endpoints,
            generated_at: Utc::now(),
        }
    }

    /// Update profiling configuration
    /// POST /api/v1/admin/infra/profile/capture
    pub async fn update_profiling(
        &self,
        request: UpdateProfilingRequest,
    ) -> Result<ProfilingStatusResponse, ProfilingError> {
        let mut config = self.profiling_service.config().clone();

        // Apply updates
        if let Some(sample_rate) = request.sample_rate {
            config.sample_rate = sample_rate.clamp(0.0, 1.0);
        }
        if let Some(threshold) = request.slow_request_threshold_ms {
            config.slow_request_threshold_ms = threshold;
        }
        if let Some(p95) = request.p95_threshold_ms {
            config.p95_threshold_ms = p95;
        }
        if let Some(p99) = request.p99_threshold_ms {
            config.p99_threshold_ms = p99;
        }
        if let Some(enable_mem) = request.enable_memory_profiling {
            config.enable_memory_profiling = enable_mem;
        }
        if let Some(enable_trace) = request.enable_trace_collection {
            config.enable_trace_collection = enable_trace;
        }
        if let Some(max_traces) = request.max_traces_per_minute {
            config.max_traces_per_minute = max_traces;
        }
        if let Some(is_active) = request.is_active {
            config.enable_trace_collection = is_active;
        }

        // Update service
        self.profiling_service.update_config(config).await;

        Ok(ProfilingStatusResponse {
            is_active: self.profiling_service.config().enable_trace_collection,
            sample_rate: self.profiling_service.config().sample_rate,
            traces_per_minute: self.profiling_service.config().max_traces_per_minute,
            memory_profiling_enabled: self.profiling_service.config().enable_memory_profiling,
            trace_collection_enabled: self.profiling_service.config().enable_trace_collection,
            p95_threshold_ms: self.profiling_service.config().p95_threshold_ms,
            p99_threshold_ms: self.profiling_service.config().p99_threshold_ms,
        })
    }

    /// Get current profiling status
    pub async fn get_status(&self) -> ProfilingStatusResponse {
        let config = self.profiling_service.config();

        ProfilingStatusResponse {
            is_active: config.enable_trace_collection,
            sample_rate: config.sample_rate,
            traces_per_minute: config.max_traces_per_minute,
            memory_profiling_enabled: config.enable_memory_profiling,
            trace_collection_enabled: config.enable_trace_collection,
            p95_threshold_ms: config.p95_threshold_ms,
            p99_threshold_ms: config.p99_threshold_ms,
        }
    }

    /// Acknowledge a slow endpoint alert
    pub async fn acknowledge_alert(
        &self,
        alert_id: Uuid,
        acknowledged_by: Uuid,
    ) -> Result<(), ProfilingError> {
        // In production, would update database
        info!(alert_id = %alert_id, user_id = %acknowledged_by, "Alert acknowledged");
        Ok(())
    }
}

/// Admin endpoint request/response types
#[derive(Debug, Deserialize)]
pub struct AcknowledgeAlertRequest {
    pub alert_id: Uuid,
    pub acknowledged_by: Uuid,
}

#[derive(Debug, Serialize)]
pub struct AlertAcknowledgeResponse {
    pub ok: bool,
    pub acknowledged_at: chrono::DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum ProfilingError {
    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Not found: {0}")]
    NotFound(String),
}