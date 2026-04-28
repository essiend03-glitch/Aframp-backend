//! Unit tests for the sanctions screening engine — Issue #419
//!
//! Tests cover:
//! - `fuzzy_score` / `normalise` correctness and edge cases
//! - `SanctionsScreener` local deny-list path (no provider key, no cache)
//! - Middleware path-matching logic

#[cfg(test)]
mod tests {
    use crate::sanctions::screener::{fuzzy_score, normalise};
    use crate::middleware::sanctions::requires_screening;

    // ── normalise ─────────────────────────────────────────────────────────────

    #[test]
    fn normalise_lowercases_and_collapses_whitespace() {
        assert_eq!(normalise("  John   DOE  "), "john doe");
    }

    #[test]
    fn normalise_strips_punctuation() {
        // Hyphens and dots are stripped; alphanumeric tokens remain
        assert_eq!(normalise("Al-Qaida, Inc."), "alqaida inc");
    }

    #[test]
    fn normalise_empty_string() {
        assert_eq!(normalise(""), "");
    }

    #[test]
    fn normalise_unicode_preserved() {
        // Non-ASCII alphanumeric characters are kept
        assert_eq!(normalise("Müller"), "müller");
    }

    // ── fuzzy_score ───────────────────────────────────────────────────────────

    #[test]
    fn fuzzy_score_identical_strings_returns_one() {
        assert!((fuzzy_score("john doe", "john doe") - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn fuzzy_score_completely_different_returns_low() {
        let score = fuzzy_score("john doe", "xyz");
        assert!(score < 0.5, "expected low score, got {score}");
    }

    #[test]
    fn fuzzy_score_one_char_typo_is_high() {
        // "Jhon Doe" vs "John Doe" — one substitution out of 8 chars → 0.875
        let score = fuzzy_score("jhon doe", "john doe");
        assert!(score > 0.8, "expected >0.8, got {score}");
    }

    #[test]
    fn fuzzy_score_transliteration_variant_is_high() {
        // q/g swap in a common transliteration
        let score = fuzzy_score("muammar gaddafi", "muammar qaddafi");
        assert!(score > 0.85, "expected >0.85, got {score}");
    }

    #[test]
    fn fuzzy_score_empty_inputs() {
        // Both empty → identical
        assert!((fuzzy_score("", "") - 1.0).abs() < f64::EPSILON);
        // One empty → 0
        assert!((fuzzy_score("abc", "") - 0.0).abs() < f64::EPSILON);
        assert!((fuzzy_score("", "abc") - 0.0).abs() < f64::EPSILON);
    }

    #[test]
    fn fuzzy_score_is_symmetric() {
        let ab = fuzzy_score("alice", "alise");
        let ba = fuzzy_score("alise", "alice");
        assert!((ab - ba).abs() < f64::EPSILON, "score not symmetric: {ab} vs {ba}");
    }

    #[test]
    fn fuzzy_score_prefix_match_is_moderate() {
        // "john" vs "john doe" — 4 chars vs 8 chars, distance = 4
        let score = fuzzy_score("john", "john doe");
        assert!(score > 0.4 && score < 0.8, "expected moderate score, got {score}");
    }

    // ── local deny-list matching ───────────────────────────────────────────────
    // These tests exercise the fuzzy_score + normalise pipeline directly,
    // mirroring what SanctionsScreener::local_screen does internally.

    #[test]
    fn deny_list_exact_match_hits() {
        let entry = normalise("TEST_SANCTIONED_ENTITY");
        let query = normalise("TEST_SANCTIONED_ENTITY");
        let score = fuzzy_score(&query, &entry);
        assert!(score >= 0.85, "exact match should score ≥0.85, got {score}");
    }

    #[test]
    fn deny_list_one_char_typo_hits() {
        // Missing 'I' in ENTITY → ENTTY (21 chars, 1 edit → 1 - 1/21 ≈ 0.952)
        let entry = normalise("TEST_SANCTIONED_ENTITY");
        let query = normalise("TEST_SANCTIONED_ENTTY");
        let score = fuzzy_score(&query, &entry);
        assert!(score >= 0.85, "one-char typo should still hit, got {score}");
    }

    #[test]
    fn deny_list_unrelated_name_misses() {
        let entry = normalise("TEST_SANCTIONED_ENTITY");
        let query = normalise("alice johnson");
        let score = fuzzy_score(&query, &entry);
        assert!(score < 0.85, "unrelated name should miss, got {score}");
    }

    // ── middleware path matching ───────────────────────────────────────────────

    #[test]
    fn screened_paths_are_matched() {
        assert!(requires_screening("/account/balance"));
        assert!(requires_screening("/api/v1/onramp/initiate"));
        assert!(requires_screening("/api/v1/offramp/withdraw"));
        assert!(requires_screening("/api/v1/mint/request"));
        assert!(requires_screening("/api/v1/transaction/history"));
        assert!(requires_screening("/api/v1/transfer/send"));
        assert!(requires_screening("/api/v1/redemption/redeem"));
    }

    #[test]
    fn non_screened_paths_are_not_matched() {
        assert!(!requires_screening("/health"));
        assert!(!requires_screening("/public/rates"));
        assert!(!requires_screening("/api/v1/rates"));
        assert!(!requires_screening("/api/v1/fees"));
        assert!(!requires_screening("/metrics"));
        assert!(!requires_screening("/"));
    }

    // ── ScreeningResult helpers ───────────────────────────────────────────────

    #[test]
    fn screening_result_is_blocked_on_hit() {
        use crate::sanctions::models::{ScreeningOutcome, ScreeningResult};
        let result = ScreeningResult {
            transaction_id: uuid::Uuid::new_v4(),
            outcome: ScreeningOutcome::Hit,
            matches: vec![],
            screened_at: chrono::Utc::now(),
            latency_ms: 10,
        };
        assert!(result.is_blocked());
    }

    #[test]
    fn screening_result_is_blocked_on_provider_error() {
        use crate::sanctions::models::{ScreeningOutcome, ScreeningResult};
        let result = ScreeningResult {
            transaction_id: uuid::Uuid::new_v4(),
            outcome: ScreeningOutcome::ProviderError,
            matches: vec![],
            screened_at: chrono::Utc::now(),
            latency_ms: 0,
        };
        assert!(result.is_blocked());
    }

    #[test]
    fn screening_result_is_not_blocked_on_clear() {
        use crate::sanctions::models::{ScreeningOutcome, ScreeningResult};
        let result = ScreeningResult {
            transaction_id: uuid::Uuid::new_v4(),
            outcome: ScreeningOutcome::Clear,
            matches: vec![],
            screened_at: chrono::Utc::now(),
            latency_ms: 5,
        };
        assert!(!result.is_blocked());
    }
}
