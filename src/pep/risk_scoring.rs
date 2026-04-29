//! PEP tiered risk scoring engine
//!
//! Composite score = influence_weight × relationship_multiplier × jurisdiction_factor
//! where jurisdiction_factor = 1.0 − (cpi_score / 100.0)
//! (lower CPI = higher corruption = higher risk)

use super::models::{PepInfluenceLevel, PepRelationshipType};
use std::collections::HashMap;

pub struct PepRiskScorer {
    /// ISO 3166-1 alpha-2 → CPI score (0–100, higher = cleaner)
    cpi_table: HashMap<&'static str, u8>,
}

impl PepRiskScorer {
    pub fn new() -> Self {
        Self {
            cpi_table: build_cpi_table(),
        }
    }

    /// Look up the CPI score for a country code.
    /// Returns 50 (neutral) if the country is not in the table.
    pub fn cpi_for_country(&self, country_code: &str) -> u8 {
        *self
            .cpi_table
            .get(country_code.to_uppercase().as_str())
            .unwrap_or(&50)
    }

    /// Compute composite PEP risk score (0.0–1.0).
    ///
    /// Formula:
    ///   influence_weight × relationship_multiplier × jurisdiction_factor
    ///
    /// jurisdiction_factor = 1.0 − (cpi / 100.0)
    /// (a country with CPI=10 contributes factor=0.90; CPI=90 → factor=0.10)
    pub fn compute_risk_score(
        &self,
        influence: &PepInfluenceLevel,
        relationship: &PepRelationshipType,
        cpi_score: u8,
    ) -> f64 {
        let influence_weight = influence.base_weight();
        let rel_multiplier = relationship.weight_multiplier();
        let jurisdiction_factor = 1.0 - (cpi_score as f64 / 100.0);

        let raw = influence_weight * rel_multiplier * jurisdiction_factor;
        // Clamp to [0.0, 1.0]
        raw.clamp(0.0, 1.0)
    }
}

impl Default for PepRiskScorer {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// CPI table — Transparency International 2023 CPI scores (selected countries)
// ---------------------------------------------------------------------------

fn build_cpi_table() -> HashMap<&'static str, u8> {
    let mut m = HashMap::new();
    // High CPI (clean)
    m.insert("DK", 90); // Denmark
    m.insert("FI", 87); // Finland
    m.insert("NZ", 85); // New Zealand
    m.insert("NO", 84); // Norway
    m.insert("SG", 83); // Singapore
    m.insert("SE", 82); // Sweden
    m.insert("CH", 82); // Switzerland
    m.insert("NL", 79); // Netherlands
    m.insert("DE", 78); // Germany
    m.insert("GB", 71); // United Kingdom
    m.insert("AU", 75); // Australia
    m.insert("CA", 76); // Canada
    m.insert("US", 69); // United States
    m.insert("FR", 71); // France
    m.insert("JP", 73); // Japan
    // Mid CPI
    m.insert("GH", 43); // Ghana
    m.insert("KE", 31); // Kenya
    m.insert("ZA", 41); // South Africa
    m.insert("NG", 25); // Nigeria
    m.insert("EG", 35); // Egypt
    m.insert("MA", 38); // Morocco
    m.insert("TZ", 40); // Tanzania
    m.insert("UG", 26); // Uganda
    m.insert("ET", 37); // Ethiopia
    m.insert("CM", 26); // Cameroon
    m.insert("CI", 37); // Côte d'Ivoire
    m.insert("SN", 43); // Senegal
    m.insert("RW", 53); // Rwanda
    m.insert("BW", 59); // Botswana
    m.insert("MU", 54); // Mauritius
    // Low CPI (high corruption)
    m.insert("SS", 13); // South Sudan
    m.insert("SY", 13); // Syria
    m.insert("SO", 11); // Somalia
    m.insert("YE", 16); // Yemen
    m.insert("VE", 13); // Venezuela
    m.insert("AF", 20); // Afghanistan
    m.insert("KP", 17); // North Korea
    m.insert("IR", 25); // Iran
    m.insert("MM", 20); // Myanmar
    m.insert("LY", 18); // Libya
    m.insert("SD", 22); // Sudan
    m.insert("CD", 20); // DR Congo
    m.insert("ZW", 23); // Zimbabwe
    m.insert("HT", 17); // Haiti
    m.insert("NI", 19); // Nicaragua
    m
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_risk_score_head_of_state_low_cpi() {
        let scorer = PepRiskScorer::new();
        // Nigeria CPI=25 → jurisdiction_factor=0.75
        // HeadOfState weight=1.0, DirectPep multiplier=1.0
        // score = 1.0 × 1.0 × 0.75 = 0.75 → High tier
        let score = scorer.compute_risk_score(
            &PepInfluenceLevel::HeadOfState,
            &PepRelationshipType::DirectPep,
            25,
        );
        assert!(score > 0.60, "Expected High tier, got {}", score);
    }

    #[test]
    fn test_risk_score_local_official_high_cpi() {
        let scorer = PepRiskScorer::new();
        // Denmark CPI=90 → jurisdiction_factor=0.10
        // LocalOfficial weight=0.40, CloseAssociate multiplier=0.55
        // score = 0.40 × 0.55 × 0.10 = 0.022 → Low tier
        let score = scorer.compute_risk_score(
            &PepInfluenceLevel::LocalOfficial,
            &PepRelationshipType::CloseAssociate,
            90,
        );
        assert!(score < 0.30, "Expected Low tier, got {}", score);
    }

    #[test]
    fn test_risk_tier_from_score() {
        use super::super::models::PepRiskTier;
        assert_eq!(PepRiskTier::from_score(0.85), PepRiskTier::Critical);
        assert_eq!(PepRiskTier::from_score(0.65), PepRiskTier::High);
        assert_eq!(PepRiskTier::from_score(0.45), PepRiskTier::Medium);
        assert_eq!(PepRiskTier::from_score(0.10), PepRiskTier::Low);
    }

    #[test]
    fn test_cpi_lookup_unknown_country() {
        let scorer = PepRiskScorer::new();
        assert_eq!(scorer.cpi_for_country("XX"), 50);
    }
}
