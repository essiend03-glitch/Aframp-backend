//! PEP Database Integration Service
//! Handles ingestion from external PEP database providers

use crate::cache::RedisCache;
use crate::pep::extended_models::{PepDatabaseStatus, PepDatabaseVersion, PepDatabaseStatusResponse};
use chrono::Utc;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, warn};
use uuid::Uuid;

/// PEP Database Configuration
#[derive(Debug, Clone)]
pub struct PepDatabaseConfig {
    /// Nigerian/African PEP database provider
    pub african_provider_url: String,
    pub african_provider_key: String,
    /// International PEP database provider
    pub international_provider_url: String,
    pub international_provider_key: String,
    /// Database update interval in hours
    pub update_interval_hours: i64,
    /// Maximum staleness threshold in hours before alert
    pub max_staleness_hours: i64,
    /// Enable/disable specific sources
    pub enable_african: bool,
    pub enable_international: bool,
}

impl Default for PepDatabaseConfig {
    fn default() -> Self {
        Self {
            african_provider_url: "https://api.africanpepdb.com/v1".to_string(),
            african_provider_key: String::new(),
            international_provider_url: "https://api.dowjones.com/risk/v2".to_string(),
            international_provider_key: String::new(),
            update_interval_hours: 24,
            max_staleness_hours: 48,
            enable_african: true,
            enable_international: true,
        }
    }
}

/// Database entry from provider
#[derive(Debug, Deserialize)]
struct ProviderEntry {
    id: String,
    name: String,
    aliases: Option<Vec<String>>,
    #[serde(rename = "dateOfBirth")]
    date_of_birth: Option<String>,
    nationality: Option<String>,
    country: Option<String>,
    #[serde(rename = "positionTitle")]
    position_title: Option<String>,
    #[serde(rename = "organizationName")]
    organization_name: Option<String>,
    #[serde(rename = "entityType")]
    entity_type: Option<String>,
    #[serde(rename = "positionStartDate")]
    position_start_date: Option<String>,
    #[serde(rename = "positionEndDate")]
    position_end_date: Option<String>,
}

/// Provider response wrapper
#[derive(Debug, Deserialize)]
struct ProviderResponse {
    entries: Vec<ProviderEntry>,
    #[serde(rename = "totalCount")]
    total_count: Option<i32>,
    #[serde(rename = "nextCursor")]
    next_cursor: Option<String>,
}

/// PEP Database Service
pub struct PepDatabaseService {
    config: PepDatabaseConfig,
    http: Client,
    cache: Arc<RedisCache>,
}

impl PepDatabaseService {
    pub fn new(config: PepDatabaseConfig, cache: Arc<RedisCache>) -> Self {
        Self {
            config,
            http: Client::new(),
            cache,
        }
    }

    /// Fetch and ingest PEP data from all configured sources
    pub async fn ingest_databases(&self) -> Result<IngestionSummary, anyhow::Error> {
        info!("Starting PEP database ingestion");
        let mut summary = IngestionSummary::default();

        if self.config.enable_african {
            match self.ingest_african_database().await {
                Ok(count) => {
                    summary.african_entries = count;
                    summary.african_success = true;
                }
                Err(e) => {
                    error!(error = %e, "African PEP database ingestion failed");
                    summary.african_success = false;
                    summary.errors.push(format!("African DB: {}", e));
                }
            }
        }

        if self.config.enable_international {
            match self.ingest_international_database().await {
                Ok(count) => {
                    summary.international_entries = count;
                    summary.international_success = true;
                }
                Err(e) => {
                    error!(error = %e, "International PEP database ingestion failed");
                    summary.international_success = false;
                    summary.errors.push(format!("International DB: {}", e));
                }
            }
        }

        summary.total_entries = summary.african_entries + summary.international_entries;
        summary.completed_at = Some(Utc::now());

        info!(
            total = summary.total_entries,
            african = summary.african_entries,
            international = summary.international_entries,
            "PEP database ingestion complete"
        );

        Ok(summary)
    }

    /// Ingest from African PEP database (Nigerian and other African political figures)
    async fn ingest_african_database(&self) -> Result<i32, anyhow::Error> {
        if self.config.african_provider_key.is_empty() {
            warn!("African PEP provider not configured");
            return Ok(0);
        }

        info!("Ingesting African PEP database");

        // In production, this would paginate through the full dataset
        let mut total_entries = 0;
        let mut cursor: Option<String> = None;

        loop {
            let url = if let Some(ref c) = cursor {
                format!(
                    "{}/entities?cursor={}&limit=1000",
                    self.config.african_provider_url, c
                )
            } else {
                format!("{}/entities?limit=1000", self.config.african_provider_url)
            };

            let response = self
                .http
                .get(&url)
                .bearer_auth(&self.config.african_provider_key)
                .send()
                .await?;

            if !response.status().is_success() {
                return Err(anyhow::anyhow!(
                    "African provider returned status: {}",
                    response.status()
                ));
            }

            let data: ProviderResponse = response.json().await?;
            total_entries += data.entries.len();

            // Index entries (in production, store in DB and/or search index)
            for entry in data.entries {
                self.index_pep_entry("african", &entry).await?;
            }

            if data.next_cursor.is_none() {
                break;
            }
            cursor = data.next_cursor;
        }

        // Record version history
        self.record_version("african", total_entries).await?;

        info!(count = total_entries, "African PEP database ingestion complete");
        Ok(total_entries)
    }

    /// Ingest from international PEP database (foreign PEPs and international org officials)
    async fn ingest_international_database(&self) -> Result<i32, anyhow::Error> {
        if self.config.international_provider_key.is_empty() {
            warn!("International PEP provider not configured");
            return Ok(0);
        }

        info!("Ingesting international PEP database");

        let mut total_entries = 0;
        let mut cursor: Option<String> = None;

        loop {
            let url = if let Some(ref c) = cursor {
                format!(
                    "{}/pep/search?cursor={}&limit=1000",
                    self.config.international_provider_url, c
                )
            } else {
                format!("{}/pep/search?limit=1000", self.config.international_provider_url)
            };

            let response = self
                .http
                .get(&url)
                .bearer_auth(&self.config.international_provider_key)
                .send()
                .await?;

            if !response.status().is_success() {
                return Err(anyhow::anyhow!(
                    "International provider returned status: {}",
                    response.status()
                ));
            }

            let data: ProviderResponse = response.json().await?;
            total_entries += data.entries.len();

            for entry in data.entries {
                self.index_pep_entry("international", &entry).await?;
            }

            if data.next_cursor.is_none() {
                break;
            }
            cursor = data.next_cursor;
        }

        self.record_version("international", total_entries).await?;

        info!(count = total_entries, "International PEP database ingestion complete");
        Ok(total_entries)
    }

    /// Index a PEP entry for fast name matching
    async fn index_pep_entry(&self, source: &str, entry: &ProviderEntry) -> Result<(), anyhow::Error> {
        // Build searchable name variants
        let mut name_variants = vec![entry.name.clone()];

        if let Some(aliases) = &entry.aliases {
            name_variants.extend(aliases.clone());
        }

        // Add transliterations if available (in production, use transliteration service)

        // Cache the entry for fast lookup
        let cache_key = format!("pep:index:{}:{}", source, entry.id);
        let entry_data = serde_json::json!({
            "id": entry.id,
            "name": entry.name,
            "aliases": entry.aliases,
            "date_of_birth": entry.date_of_birth,
            "nationality": entry.nationality,
            "country": entry.country,
            "position_title": entry.position_title,
            "organization": entry.organization_name,
            "entity_type": entry.entity_type,
            "position_start_date": entry.position_start_date,
            "position_end_date": entry.position_end_date,
        });

        // Store in cache/index (in production, also store in database search index)
        let _ = self.cache.set(&cache_key, &entry_data, Some(std::time::Duration::from_secs(86400 * 30))).await;

        Ok(())
    }

    /// Record version history for audit
    async fn record_version(&self, source: &str, entry_count: i32) -> Result<(), anyhow::Error> {
        let version_id = Uuid::new_v4();
        let version_hash = format!("{}_{}", source, Utc::now().timestamp());

        // In production, store in pep_database_versions table
        info!(
            version_id = %version_id,
            source = source,
            entries = entry_count,
            "Recorded PEP database version"
        );

        // Update cached status
        let status_key = format!("pep:db:status:{}", source);
        let status = PepDatabaseStatus {
            id: Uuid::new_v4(),
            source_name: source.to_string(),
            last_update: Some(Utc::now()),
            total_entries: entry_count,
            index_health: "healthy".to_string(),
            config: serde_json::json!({}),
        };
        let _ = self.cache.set(&status_key, &status, None).await;

        Ok(())
    }

    /// Get database status for API endpoint
    pub async fn get_database_status(&self) -> PepDatabaseStatusResponse {
        let sources = vec!["african", "international"];
        let mut source_statuses = Vec::new();
        let mut last_global_update: Option<chrono::DateTime<Utc>> = None;
        let mut total_entries = 0;

        for source in sources {
            let status_key = format!("pep:db:status:{}", source);
            let status: Option<PepDatabaseStatus> = self.cache.get(&status_key).await.unwrap_or(None);

            if let Some(s) = status {
                if last_global_update.is_none() || s.last_update > last_global_update {
                    last_global_update = s.last_update;
                }
                total_entries += s.total_entries;
                source_statuses.push(s);
            } else {
                // Return default status if not in cache
                source_statuses.push(PepDatabaseStatus::new(source.to_string()));
            }
        }

        // Determine overall health
        let overall_health = if let Some(last_update) = last_global_update {
            let hours_since = (Utc::now() - last_update).num_hours();
            if hours_since > self.config.max_staleness_hours {
                "stale".to_string()
            } else if hours_since > self.config.max_staleness_hours / 2 {
                "warning".to_string()
            } else {
                "healthy".to_string()
            }
        } else {
            "unknown".to_string()
        };

        PepDatabaseStatusResponse {
            sources: source_statuses,
            overall_health,
            last_global_update,
            total_indexed_entries: total_entries,
        }
    }

    /// Check if databases need update
    pub async fn needs_update(&self) -> bool {
        let status = self.get_database_status().await;

        if let Some(last_update) = status.last_global_update {
            let hours_since = (Utc::now() - last_update).num_hours();
            hours_since >= self.config.update_interval_hours
        } else {
            true // Never updated
        }
    }
}

/// Summary of database ingestion
#[derive(Debug, Default, Serialize)]
pub struct IngestionSummary {
    pub african_entries: i32,
    pub international_entries: i32,
    pub total_entries: i32,
    pub african_success: bool,
    pub international_success: bool,
    pub completed_at: Option<chrono::DateTime<Utc>>,
    pub errors: Vec<String>,
}

impl IngestionSummary {
    pub fn is_success(&self) -> bool {
        self.african_success && self.international_success
    }
}