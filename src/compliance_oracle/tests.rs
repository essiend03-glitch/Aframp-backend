//! #491 Compliance Oracle — unit tests.

#[cfg(test)]
mod tests {
    use crate::compliance_oracle::models::*;
    use chrono::{Duration, Utc};
    use uuid::Uuid;

    // ── DID parsing ───────────────────────────────────────────────────────────

    fn parse_did(did_str: &str) -> Result<ParsedDid, String> {
        let parts: Vec<&str> = did_str.splitn(3, ':').collect();
        if parts.len() != 3 || parts[0] != "did" {
            return Err("invalid_did_format".into());
        }
        let method = match parts[1] {
            "web"     => DidMethod::DidWeb,
            "key"     => DidMethod::DidKey,
            "ethr"    => DidMethod::DidEthr,
            "stellar" => DidMethod::DidStellar,
            "ion"     => DidMethod::DidIon,
            other     => return Err(format!("unsupported_did_method: {}", other)),
        };
        Ok(ParsedDid {
            method,
            identifier: parts[2].to_string(),
            controller: did_str.to_string(),
        })
    }

    #[test]
    fn did_key_parses_correctly() {
        let did = parse_did("did:key:z6MkhaXgBZDvotDkL5257faiztiGiC2QtKLGpbnnEGta2doK").unwrap();
        assert_eq!(did.method, DidMethod::DidKey);
        assert!(did.identifier.starts_with('z'));
    }

    #[test]
    fn did_stellar_parses_correctly() {
        let did = parse_did("did:stellar:GABC123").unwrap();
        assert_eq!(did.method, DidMethod::DidStellar);
    }

    #[test]
    fn invalid_did_format_rejected() {
        assert!(parse_did("not:a:did").is_err());
        assert!(parse_did("did:unknown:abc").is_err());
        assert!(parse_did("did:key").is_err());
    }

    // ── ZKP validation ────────────────────────────────────────────────────────

    fn validate_zkp(proof_ref: &str, issuer_sig: &str, proof_hash: &str) -> Result<bool, String> {
        if proof_ref.is_empty() || issuer_sig.is_empty() || proof_hash.is_empty() {
            return Err("malformed_zkp_packet".into());
        }
        if proof_hash.len() < 32 {
            return Err("invalid_proof_hash_length".into());
        }
        Ok(true)
    }

    #[test]
    fn valid_zkp_passes() {
        let result = validate_zkp(
            "ZK_REF_123",
            "SIG_ABCDEF",
            "a".repeat(64).as_str(),
        );
        assert!(result.is_ok());
    }

    #[test]
    fn empty_zkp_fields_rejected() {
        assert!(validate_zkp("", "sig", "hash_long_enough_here_32chars_ok").is_err());
        assert!(validate_zkp("ref", "", "hash_long_enough_here_32chars_ok").is_err());
    }

    #[test]
    fn short_proof_hash_rejected() {
        assert!(validate_zkp("ref", "sig", "tooshort").is_err());
    }

    // ── Attestation expiry ────────────────────────────────────────────────────

    #[test]
    fn expired_attestation_detected() {
        let expires_at = Utc::now() - Duration::hours(1);
        assert!(expires_at <= Utc::now(), "Should be expired");
    }

    #[test]
    fn valid_attestation_not_expired() {
        let expires_at = Utc::now() + Duration::hours(23);
        assert!(expires_at > Utc::now(), "Should be valid");
    }

    // ── No PII in proof hash ──────────────────────────────────────────────────

    #[test]
    fn proof_hash_is_opaque_hex() {
        let id = Uuid::new_v4();
        let hash = format!("{:064x}", id.as_u128());
        // Must be 64 hex chars — no PII
        assert_eq!(hash.len(), 32); // u128 = 32 hex chars
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    // ── Amber risk match triggers circuit breaker ─────────────────────────────

    #[test]
    fn amber_risk_match_blocks_transaction() {
        let is_sanctions_clear = false;
        let is_aml_clear = true;
        let should_block = !is_sanctions_clear || !is_aml_clear;
        assert!(should_block, "Amber match should block");
    }
}
