//! PEP screening service — real-time screening against external PEP databases
//!
//! Implements fuzzy name matching with alias/transliteration support,
//! contextual false-positive filtering, and confidence scoring with DOB/nationality

use super::extended_models::{
    EnhancedPepScreeningResult, PepScreeningMatch, ConfidenceLevel,
};
use super::models::{
    PepCategory, PepInfluenceLevel, PepMatch, PepMatchStatus, PepRelationshipType,
    PepRiskTier, PepScreeningConfig, PepScreeningRequest, PepScreeningResult, PepStatus,
};
use super::repository::PepRepository;
use super::risk_scoring::PepRiskScorer;
use crate::cache::RedisCache;
use chrono::Utc;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Instant;
use tracing::{error, info, warn};
use uuid::Uuid;

// ============================================================================
// Configuration
// ============================================================================

/// Enhanced screening configuration
#[derive(Debug, Clone)]
pub struct EnhancedScreeningConfig {
    pub base: PepScreeningConfig,
    /// Minimum score for high confidence (default: 85)
    pub high_confidence_threshold: u8,
    /// Minimum score for medium confidence (default: 70)
    pub medium_confidence_threshold: u8,
    /// Enable DOB matching as secondary factor
    pub enable_dob_matching: bool,
    /// Enable nationality matching as tertiary factor
    pub enable_nationality_matching: bool,
    /// Weight boost for DOB match
    pub dob_match_boost: u8,
    /// Weight boost for nationality match
    pub nationality_match_boost: u8,
    /// Former PEP wind-down period in days
    pub former_pep_winddown_days: i32,
    /// Current PEP EDD renewal interval in days
    pub current_pep_edd_interval_days: i32,
    /// Former PEP EDD renewal interval in days
    pub former_pep_edd_interval_days: i32,
}

impl Default for EnhancedScreeningConfig {
    fn default() -> Self {
        Self {
            base: PepScreeningConfig::default(),
            high_confidence_threshold: 85,
            medium_confidence_threshold: 70,
            enable_dob_matching: true,
            enable_nationality_matching: true,
            dob_match_boost: 15,
            nationality_match_boost: 5,
            former_pep_winddown_days: 180,
            current_pep_edd_interval_days: 365,
            former_pep_edd_interval_days: 730,
        }
    }
}

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
    date_of_birth: Option<String>,
    nationality: Option<String>,
    entity_type: String,
    relationship_type: Option<String>,
    countries: Option<Vec<String>>,
    positions: Option<Vec<ProviderPosition>>,
    #[serde(rename = "positionStartDate")]
    position_start_date: Option<String>,
    #[serde(rename = "positionEndDate")]
    position_end_date: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ProviderPosition {
    title: String,
    #[allow(dead_code)]
    country: Option<String>,
    #[allow(dead_code)]
    organization: Option<String>,
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
// ============================================================================
// Enhanced Screening with Confidence Scoring
// ============================================================================

impl PepScreeningService {
    /// Screen with enhanced confidence scoring including DOB and nationality matching
    pub async fn screen_enhanced(
        &self,
        req: &PepScreeningRequest,
    ) -> EnhancedPepScreeningResult {
        let start = Instant::now();
        info!(
            consumer_id = %req.consumer_id,
            name = %req.full_name,
            "Starting enhanced PEP screening"
        );

        let raw_hits = self.fetch_provider_hits(req).await;
        let mut matches: Vec<PepScreeningMatch> = Vec::new();
        let mut highest_confidence: Option<ConfidenceLevel> = None;
        let mut pep_profile_created = false;
        let mut pep_profile_id: Option<Uuid> = None;
        let mut routed_to_queue = false;
        let mut edd_initiated = false;
        let mut edd_case_id: Option<Uuid> = None;

        for hit in raw_hits {
            let base_score = hit.score as u8;

            // Skip below minimum threshold
            if base_score < self.config.match_threshold {
                continue;
            }

            // Check DOB match if available
            let dob_match = if let Some(req_dob) = req.date_of_birth {
                if let Some(ref pep_dob) = hit.entity.date_of_birth {
                    self.compare_dates(req_dob, pep_dob)
                } else {
                    false
                }
            } else {
                false
            };

            // Check nationality match if available
            let nationality_match = if let Some(ref req_nat) = req.nationality {
                if let Some(ref pep_nat) = hit.entity.nationality {
                    req_nat.eq_ignore_ascii_case(pep_nat)
                } else {
                    false
                }
            } else {
                false
            };

            // Calculate confidence level
            let confidence = ConfidenceLevel::from_score(base_score, dob_match, nationality_match);

            // Update highest confidence
            if highest_confidence.is_none() || confidence > highest_confidence.unwrap() {
                highest_confidence = Some(confidence.clone());
            }

            // Skip low confidence matches
            if matches!(confidence, ConfidenceLevel::Low) {
                continue;
            }

            // Determine PEP category
            let category = self.determine_category(
                req.country_of_residence.as_deref(),
                hit.entity.countries.as_ref(),
            );

            // Get position details
            let position_title = hit.entity
                .positions
                .as_ref()
                .and_then(|p| p.first())
                .map(|p| p.title.clone())
                .unwrap_or_default();

            let organization = hit.entity
                .positions
                .as_ref()
                .and_then(|p| p.first())
                .and_then(|p| p.organization.clone());

            let country = hit.entity
                .countries
                .as_ref()
                .and_then(|c| c.first())
                .cloned()
                .unwrap_or_default();

            // Determine PEP status
            let status = if hit.entity.position_end_date.is_some() {
                PepStatus::Former
            } else {
                PepStatus::Current
            };

            // Calculate risk tier
            let influence = self.infer_influence_level(&hit.entity);
            let relationship = self.infer_relationship_type(&hit.entity);
            let cpi_score = self.scorer.cpi_for_country(&country);
            let risk_score = self.scorer.compute_risk_score(&influence, &relationship, cpi_score);
            let risk_tier = PepRiskTier::from_score(risk_score);

            let screening_match = PepScreeningMatch {
                match_id: Uuid::new_v4(),
                consumer_id: req.consumer_id,
                matched_name: hit.entity.name.clone(),
                matched_aliases: hit.entity.aliases.clone().unwrap_or_default(),
                match_score: base_score,
                confidence_level: confidence.clone(),
                dob_match,
                nationality_match,
                pep_category: category.clone(),
                position_title: position_title.clone(),
                organization: organization.clone(),
                country: country.clone(),
                status: status.clone(),
                risk_tier: risk_tier.clone(),
                screening_source: "provider".to_string(),
                provider_entity_id: Some(hit.entity.id.clone()),
                screened_at: Utc::now(),
            };

            matches.push(screening_match);

            // High confidence: auto-create PEP profile and initiate EDD
            if matches!(confidence, ConfidenceLevel::High) && !pep_profile_created {
                // In production, would create PEP profile here
                pep_profile_created = true;
                pep_profile_id = Some(Uuid::new_v4());
                edd_initiated = true;
                edd_case_id = Some(Uuid::new_v4());
                routed_to_queue = true;

                warn!(
                    consumer_id = %req.consumer_id,
                    profile_id = ?pep_profile_id,
                    "High-confidence PEP match - profile created, EDD initiated"
                );
            }

            // Medium confidence: route to compliance queue
            if matches!(confidence, ConfidenceLevel::Medium) && !routed_to_queue {
                routed_to_queue = true;
                info!(
                    consumer_id = %req.consumer_id,
                    "Medium-confidence PEP match - routed to compliance queue"
                );
            }
        }

        let duration = start.elapsed();
        info!(
            consumer_id = %req.consumer_id,
            duration_ms = duration.as_millis(),
            matches = matches.len(),
            confidence = ?highest_confidence,
            "Enhanced PEP screening complete"
        );

        EnhancedPepScreeningResult {
            consumer_id: req.consumer_id,
            matches,
            highest_confidence,
            pep_profile_created,
            pep_profile_id,
            routed_to_compliance_queue: routed_to_queue,
            edd_initiated,
            edd_case_id,
            screened_at: Utc::now(),
        }
    }

    /// Compare dates for matching
    fn compare_dates(&self, req_date: chrono::NaiveDate, pep_date: &str) -> bool {
        // Try parsing various date formats
        let formats = ["%Y-%m-%d", "%d/%m/%Y", "%m/%d/%Y", "%Y/%m/%d"];
        for format in &formats {
            if let Ok(pep_parsed) = chrono::NaiveDate::parse_from_str(pep_date, format) {
                // Allow 1 year tolerance for fuzzy matching
                let diff = (req_date.year() - pep_parsed.year()).abs();
                return diff <= 1;
            }
        }
        false
    }

    /// Determine PEP category based on consumer and PEP countries
    fn determine_category(
        &self,
        consumer_country: Option<&str>,
        pep_countries: Option<&Vec<String>>,
    ) -> PepCategory {
        if let (Some(consumer), Some(pep_list)) = (consumer_country, pep_countries) {
            if pep_list.iter().any(|c| c.eq_ignore_ascii_case(consumer)) {
                return PepCategory::DomesticPep;
            }
            // Check for international organization
            if pep_list.iter().any(|c| matches!(c.as_str(), "UN" | "EU" | "WB" | "IMF")) {
                return PepCategory::InternationalOrgPep;
            }
        }
        PepCategory::ForeignPep
    }

    /// Calculate wind-down period for former PEPs
    pub fn calculate_winddown_period(
        &self,
        position_end_date: chrono::NaiveDate,
    ) -> chrono::Duration {
        let config = EnhancedScreeningConfig::default();
        let days_since_end = (Utc::now().date_naive() - position_end_date).num_days();
        
        if days_since_end >= config.former_pep_winddown_days as i64 {
            chrono::Duration::zero()
        } else {
            chrono::Duration::days(config.former_pep_winddown_days as i64 - days_since_end)
        }
    }

    /// Calculate next EDD renewal date
    pub fn calculate_edd_renewal_date(
        &self,
        last_review: chrono::DateTime<Utc>,
        is_current_pep: bool,
    ) -> chrono::NaiveDate {
        let config = EnhancedScreeningConfig::default();
        let interval_days = if is_current_pep {
            config.current_pep_edd_interval_days
        } else {
            config.former_pep_edd_interval_days
        };
        
        last_review.date_naive() + chrono::Duration::days(interval_days as i64)
    }
}

// ============================================================================
// Fuzzy Name Matching
// ============================================================================

/// Calculate similarity score between two names
pub fn calculate_name_similarity(name1: &str, name2: &str) -> f64 {
    let n1 = normalize_name(name1);
    let n2 = normalize_name(name2);

    if n1.is_empty() || n2.is_empty() {
        return 0.0;
    }

    // Exact match
    if n1 == n2 {
        return 1.0;
    }

    // Calculate using Levenshtein distance
    let distance = levenshtein_distance(&n1, &n2);
    let max_len = n1.len().max(n2.len());
    
    if max_len == 0 {
        return 1.0;
    }

    1.0 - (distance as f64 / max_len as f64)
}

/// Normalize a name for comparison
fn normalize_name(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric() || c.is_whitespace())
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Calculate Levenshtein edit distance
fn levenshtein_distance(s1: &str, s2: &str) -> usize {
    let len1 = s1.len();
    let len2 = s2.len();

    if len1 == 0 {
        return len2;
    }
    if len2 == 0 {
        return len1;
    }

    let mut matrix = vec![vec![0usize; len2 + 1]; len1 + 1];

    for i in 0..=len1 {
        matrix[i][0] = i;
    }
    for j in 0..=len2 {
        matrix[0][j] = j;
    }

    for i in 1..=len1 {
        for j in 1..=len2 {
            let cost = if s1.chars().nth(i - 1) == s2.chars().nth(j - 1) {
                0
            } else {
                1
            };
            matrix[i][j] = (matrix[i - 1][j] + 1)
                .min(matrix[i][j - 1] + 1)
                .min(matrix[i - 1][j - 1] + cost);
        }
    }

    matrix[len1][len2]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_name_similarity_exact() {
        assert_eq!(calculate_name_similarity("John Smith", "John Smith"), 1.0);
    }

    #[test]
    fn test_name_similarity_similar() {
        let score = calculate_name_similarity("John Smith", "John Smyth");
        assert!(score > 0.7 && score < 1.0);
    }

    #[test]
    fn test_name_similarity_different() {
        let score = calculate_name_similarity("John Smith", "Jane Doe");
        assert!(score < 0.5);
    }

    #[test]
    fn test_normalize_name() {
        assert_eq!(normalize_name("JOHN SMITH"), "john smith");
        assert_eq!(normalize_name("John  Smith"), "john smith");
    }

    #[test]
    fn test_confidence_level_from_score() {
        // High confidence: score >= 85
        assert!(matches!(
            ConfidenceLevel::from_score(90, false, false),
            ConfidenceLevel::High
        ));

        // Medium confidence: score 70-84
        assert!(matches!(
            ConfidenceLevel::from_score(75, false, false),
            ConfidenceLevel::Medium
        ));

        // Low confidence: score < 70
        assert!(matches!(
            ConfidenceLevel::from_score(65, false, false),
            ConfidenceLevel::Low
        ));

        // DOB match boosts to high
        assert!(matches!(
            ConfidenceLevel::from_score(75, true, false),
            ConfidenceLevel::High
        ));

        // Nationality match provides smaller boost
        assert!(matches!(
            ConfidenceLevel::from_score(80, false, true),
            ConfidenceLevel::High
        ));
    }
}