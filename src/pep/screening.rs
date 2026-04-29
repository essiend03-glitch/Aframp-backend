//! PEP screening service — real-time screening against external PEP databases
//!
//! Implements fuzzy name matching with alias/transliteration support and
//! contextual false-positive filtering.

use super::models::{
    PepInfluenceLevel, PepMatch, PepMatchStatus, PepRelationshipType, PepRiskTier,
    PepScreeningConfig, PepScreeningRequest, PepScreeningResult,
};
use super::repository::PepRepository;
use super::risk_scoring::PepRiskScorer;
use crate::cache::RedisCache;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, warn};
use uuid::Uuid;

// ---------------------------------------------------------------------------
// Provider wire types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct ProviderSearchRequest {
    name: String,
    fuzziness: f64,
    filters: ProviderFilters,
    #[serde(skip_serializing_if = "Option::is_none")]
    date_of_birth: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    nationality: Option<String>,
}

#[derive(Debug, Serialize)]
struct ProviderFilters {
    types: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ProviderSearchResponse {
    hits: Vec<ProviderHit>,
}

#[derive(Debug, Deserialize)]
struct ProviderHit {
    /// 0–100 match score
    score: f64,
    entity: ProviderEntity,
}

#[derive(Debug, Deserialize)]
struct ProviderEntity {
    id: String,
    name: String,
    aliases: Option<Vec<String>>,
    entity_type: String,
    relationship_type: Option<String>,
    countries: Option<Vec<String>>,
    positions: Option<Vec<ProviderPosition>>,
}

#[derive(Debug, Deserialize)]
struct ProviderPosition {
    title: String,
    #[allow(dead_code)]
    country: Option<String>,
}

// ---------------------------------------------------------------------------
// Service
// ---------------------------------------------------------------------------

pub struct PepScreeningService {
    config: PepScreeningConfig,
    http: Client,
    cache: Arc<RedisCache>,
    repo: Arc<PepRepository>,
    scorer: PepRiskScorer,
}

impl PepScreeningService {
    pub fn new(
        config: PepScreeningConfig,
        cache: Arc<RedisCache>,
        repo: Arc<PepRepository>,
    ) -> Self {
        let scorer = PepRiskScorer::new();
        Self {
            config,
            http: Client::new(),
            cache,
            repo,
            scorer,
        }
    }

    /// Screen a consumer against global PEP databases.
    /// Called during KYC onboarding and periodic re-screening.
    pub async fn screen(&self, req: &PepScreeningRequest) -> PepScreeningResult {
        info!(
            consumer_id = %req.consumer_id,
            name = %req.full_name,
            is_rescreening = req.is_rescreening,
            "Starting PEP screening"
        );

        let raw_hits = self.fetch_provider_hits(req).await;
        let mut matches: Vec<PepMatch> = Vec::new();

        for hit in raw_hits {
            let score = hit.score as u8;

            // Below minimum threshold — skip entirely
            if score < self.config.match_threshold {
                continue;
            }

            let influence = infer_influence_level(&hit.entity);
            let relationship = infer_relationship_type(&hit.entity);
            let jurisdiction = hit
                .entity
                .countries
                .as_ref()
                .and_then(|c| c.first())
                .cloned()
                .unwrap_or_default();

            let cpi_score = self.scorer.cpi_for_country(&jurisdiction);
            let risk_score = self.scorer.compute_risk_score(&influence, &relationship, cpi_score);
            let risk_tier = PepRiskTier::from_score(risk_score);

            // Contextual false-positive suppression
            let status = if score < self.config.auto_suppress_threshold {
                PepMatchStatus::AutoSuppressed
            } else if self.is_contextual_false_positive(req, &hit.entity, score) {
                PepMatchStatus::AutoSuppressed
            } else {
                PepMatchStatus::PendingReview
            };

            if status == PepMatchStatus::AutoSuppressed {
                info!(
                    consumer_id = %req.consumer_id,
                    matched_name = %hit.entity.name,
                    score,
                    "PEP match auto-suppressed by contextual filter"
                );
            } else {
                warn!(
                    consumer_id = %req.consumer_id,
                    matched_name = %hit.entity.name,
                    score,
                    risk_tier = risk_tier.as_str(),
                    "PEP match requires review"
                );
            }

            matches.push(PepMatch {
                match_id: Uuid::new_v4(),
                consumer_id: req.consumer_id,
                matched_name: hit.entity.name.clone(),
                matched_aliases: hit.entity.aliases.unwrap_or_default(),
                match_score: score,
                influence_level: influence,
                relationship_type: relationship,
                jurisdiction,
                cpi_score,
                risk_score,
                risk_tier,
                status,
                provider_entity_id: Some(hit.entity.id),
                screened_at: chrono::Utc::now(),
                reviewed_at: None,
                reviewed_by: None,
                review_notes: None,
            });
        }

        // Persist matches and audit entry
        let edd_case_id = self
            .repo
            .save_screening_result(req.consumer_id, &matches)
            .await
            .unwrap_or_else(|e| {
                error!(error = %e, "Failed to persist PEP screening result");
                None
            });

        let highest_risk_tier = matches
            .iter()
            .filter(|m| m.status == PepMatchStatus::PendingReview)
            .map(|m| &m.risk_tier)
            .max()
            .cloned();

        let edd_triggered = edd_case_id.is_some();

        PepScreeningResult {
            consumer_id: req.consumer_id,
            matches,
            highest_risk_tier,
            edd_triggered,
            edd_case_id,
            screened_at: chrono::Utc::now(),
        }
    }

    // -----------------------------------------------------------------------
    // Provider integration
    // -----------------------------------------------------------------------

    async fn fetch_provider_hits(&self, req: &PepScreeningRequest) -> Vec<ProviderHit> {
        if self.config.provider_api_key.is_empty() {
            // Dev/test mode — no provider configured
            return Vec::new();
        }

        let cache_key = format!(
            "pep:screen:{}:{}",
            req.consumer_id,
            slug(&req.full_name)
        );

        // Check negative cache
        if let Ok(Some(true)) =
            <crate::cache::RedisCache as crate::cache::Cache<bool>>::get(
                &*self.cache,
                &cache_key,
            )
            .await
        {
            return Vec::new();
        }

        let payload = ProviderSearchRequest {
            name: req.full_name.clone(),
            fuzziness: self.config.fuzziness,
            filters: ProviderFilters {
                types: vec!["pep".into(), "pep-class-1".into(), "pep-class-2".into(), "pep-class-3".into(), "pep-class-4".into()],
            },
            date_of_birth: req.date_of_birth.map(|d| d.to_string()),
            nationality: req.nationality.clone(),
        };

        match self
            .http
            .post(format!("{}/searches", self.config.provider_base_url))
            .bearer_auth(&self.config.provider_api_key)
            .json(&payload)
            .send()
            .await
        {
            Ok(resp) => match resp.json::<ProviderSearchResponse>().await {
                Ok(body) => {
                    if body.hits.is_empty() {
                        // Cache negative result
                        let _ = <crate::cache::RedisCache as crate::cache::Cache<bool>>::set(
                            &*self.cache,
                            &cache_key,
                            &true,
                            Some(std::time::Duration::from_secs(
                                self.config.negative_cache_ttl_secs,
                            )),
                        )
                        .await;
                    }
                    body.hits
                }
                Err(e) => {
                    error!(error = %e, "Failed to parse PEP provider response");
                    Vec::new()
                }
            },
            Err(e) => {
                error!(error = %e, "PEP provider request failed — failing open");
                Vec::new()
            }
        }
    }

    // -----------------------------------------------------------------------
    // Contextual false-positive filtering
    // -----------------------------------------------------------------------

    /// Returns true if the match can be automatically suppressed based on
    /// contextual signals (different country, different age group, etc.)
    fn is_contextual_false_positive(
        &self,
        req: &PepScreeningRequest,
        entity: &ProviderEntity,
        score: u8,
    ) -> bool {
        // If the consumer's country of residence is known and the PEP's
        // jurisdiction is completely different, and the score is borderline,
        // suppress automatically.
        if score < 80 {
            if let (Some(consumer_country), Some(entity_countries)) =
                (&req.country_of_residence, &entity.countries)
            {
                if !entity_countries.is_empty()
                    && !entity_countries
                        .iter()
                        .any(|c| c.eq_ignore_ascii_case(consumer_country))
                {
                    return true;
                }
            }
        }
        false
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn infer_influence_level(entity: &ProviderEntity) -> PepInfluenceLevel {
    let title_lower = entity
        .positions
        .as_ref()
        .and_then(|p| p.first())
        .map(|p| p.title.to_lowercase())
        .unwrap_or_default();

    if title_lower.contains("president")
        || title_lower.contains("prime minister")
        || title_lower.contains("head of state")
    {
        PepInfluenceLevel::HeadOfState
    } else if title_lower.contains("minister")
        || title_lower.contains("senator")
        || title_lower.contains("parliament")
        || title_lower.contains("judge")
    {
        PepInfluenceLevel::NationalSenior
    } else if title_lower.contains("governor")
        || title_lower.contains("general")
        || title_lower.contains("colonel")
    {
        PepInfluenceLevel::RegionalSenior
    } else if entity.entity_type.to_lowercase().contains("soe")
        || title_lower.contains("ceo")
        || title_lower.contains("director")
    {
        PepInfluenceLevel::StateEnterpriseExec
    } else {
        PepInfluenceLevel::LocalOfficial
    }
}

fn infer_relationship_type(entity: &ProviderEntity) -> PepRelationshipType {
    match entity
        .relationship_type
        .as_deref()
        .unwrap_or("direct")
        .to_lowercase()
        .as_str()
    {
        "family" | "relative" | "spouse" | "child" | "parent" => {
            PepRelationshipType::ImmediateFamily
        }
        "associate" | "business_partner" | "close_associate" => {
            PepRelationshipType::CloseAssociate
        }
        _ => PepRelationshipType::DirectPep,
    }
}

fn slug(s: &str) -> String {
    s.to_lowercase().replace(' ', "_")
}
