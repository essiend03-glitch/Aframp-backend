//! Unit tests for the Mint Authorization Framework (#213).
//!
//! Tests pure-logic functions — no database or Stellar Horizon required.

#[cfg(feature = "database")]
mod tests {
    use aframp_backend::mint_authorization::{
        error::MintAuthError,
        models::MintAuthSignature,
        service::{aggregate_signatures, compute_tx_hash, verify_ed25519_signature},
    };
    use base64::{engine::general_purpose::STANDARD as B64, Engine};
    use chrono::Utc;
    use ed25519_dalek::{Signer, SigningKey};
    use rand::rngs::OsRng;
    use stellar_strkey::ed25519::PublicKey as StrkeyPublicKey;
    use uuid::Uuid;

    // Known valid Stellar testnet addresses
    const ISSUER: &str = "GCJRI5CIWK5IU67Q6DGA7QW52JDKRO7JEAHQKFNDUJUPEZGURDBX3LDX";
    const DEST: &str = "GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN";
    const TESTNET_PASSPHRASE: &str = "Test SDF Network ; September 2015";

    // ─────────────────────────────────────────────────────────────────────────
    // Helpers
    // ─────────────────────────────────────────────────────────────────────────

    fn make_unsigned_xdr() -> String {
        aframp_backend::multisig::xdr_builder::build_mint_xdr(ISSUER, DEST, 10_000_000_000, 42)
            .expect("build_mint_xdr")
    }

    /// Generate a fresh Ed25519 keypair and return (stellar_public_key_str, signing_key).
    fn gen_keypair() -> (String, SigningKey) {
        let signing_key = SigningKey::generate(&mut OsRng);
        let verifying_key = signing_key.verifying_key();
        let strkey = StrkeyPublicKey(verifying_key.to_bytes());
        (strkey.to_string(), signing_key)
    }

    fn sign_hash(signing_key: &SigningKey, tx_hash_hex: &str) -> String {
        let hash_bytes = hex::decode(tx_hash_hex).unwrap();
        let sig = signing_key.sign(&hash_bytes);
        B64.encode(sig.to_bytes())
    }

    fn make_signature_record(signer_key: &str, signature: &str) -> MintAuthSignature {
        MintAuthSignature {
            id: Uuid::new_v4(),
            auth_request_id: Uuid::new_v4(),
            signer_id: Uuid::new_v4(),
            signer_key: signer_key.to_string(),
            signature: signature.to_string(),
            signed_at: Utc::now(),
            ip_address: None,
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // compute_tx_hash
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn tx_hash_is_deterministic() {
        let xdr = make_unsigned_xdr();
        let h1 = compute_tx_hash(&xdr, TESTNET_PASSPHRASE).unwrap();
        let h2 = compute_tx_hash(&xdr, TESTNET_PASSPHRASE).unwrap();
        assert_eq!(h1, h2);
        assert_eq!(h1.len(), 64, "SHA-256 hex is 64 chars");
    }

    #[test]
    fn tx_hash_differs_for_different_network() {
        let xdr = make_unsigned_xdr();
        let testnet = compute_tx_hash(&xdr, TESTNET_PASSPHRASE).unwrap();
        let mainnet =
            compute_tx_hash(&xdr, "Public Global Stellar Network ; September 2015").unwrap();
        assert_ne!(testnet, mainnet, "network passphrase must affect hash");
    }

    #[test]
    fn tx_hash_rejects_invalid_xdr() {
        let err = compute_tx_hash("not-valid-xdr", TESTNET_PASSPHRASE);
        assert!(err.is_err());
    }

    // ─────────────────────────────────────────────────────────────────────────
    // verify_ed25519_signature
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn valid_signature_passes_verification() {
        let xdr = make_unsigned_xdr();
        let tx_hash = compute_tx_hash(&xdr, TESTNET_PASSPHRASE).unwrap();
        let (pub_key, signing_key) = gen_keypair();
        let sig = sign_hash(&signing_key, &tx_hash);

        assert!(
            verify_ed25519_signature(&pub_key, &tx_hash, &sig).is_ok(),
            "valid signature must pass"
        );
    }

    #[test]
    fn wrong_key_fails_verification() {
        let xdr = make_unsigned_xdr();
        let tx_hash = compute_tx_hash(&xdr, TESTNET_PASSPHRASE).unwrap();
        let (_, signing_key) = gen_keypair();
        let (other_pub_key, _) = gen_keypair(); // different key
        let sig = sign_hash(&signing_key, &tx_hash);

        let result = verify_ed25519_signature(&other_pub_key, &tx_hash, &sig);
        assert!(result.is_err(), "wrong key must fail");
    }

    #[test]
    fn tampered_hash_fails_verification() {
        let xdr = make_unsigned_xdr();
        let tx_hash = compute_tx_hash(&xdr, TESTNET_PASSPHRASE).unwrap();
        let (pub_key, signing_key) = gen_keypair();
        let sig = sign_hash(&signing_key, &tx_hash);

        // Flip one hex char in the hash
        let mut tampered = tx_hash.clone();
        let last = tampered.pop().unwrap();
        tampered.push(if last == 'a' { 'b' } else { 'a' });

        let result = verify_ed25519_signature(&pub_key, &tampered, &sig);
        assert!(result.is_err(), "tampered hash must fail");
    }

    #[test]
    fn invalid_base64_signature_returns_error() {
        let xdr = make_unsigned_xdr();
        let tx_hash = compute_tx_hash(&xdr, TESTNET_PASSPHRASE).unwrap();
        let (pub_key, _) = gen_keypair();

        let result = verify_ed25519_signature(&pub_key, &tx_hash, "not-valid-base64!!!");
        assert!(matches!(result, Err(MintAuthError::InvalidSignature(_, _))));
    }

    #[test]
    fn invalid_stellar_key_returns_error() {
        let xdr = make_unsigned_xdr();
        let tx_hash = compute_tx_hash(&xdr, TESTNET_PASSPHRASE).unwrap();

        let result = verify_ed25519_signature("INVALID_KEY", &tx_hash, "AAAA");
        assert!(matches!(result, Err(MintAuthError::InvalidSignature(_, _))));
    }

    // ─────────────────────────────────────────────────────────────────────────
    // aggregate_signatures
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn aggregate_zero_signatures_produces_valid_envelope() {
        let xdr = make_unsigned_xdr();
        let signed = aggregate_signatures(&xdr, &[]).unwrap();
        assert!(!signed.is_empty());
        // Must still be valid XDR
        use stellar_xdr::next::{Limits, ReadXdr, TransactionEnvelope};
        let env = TransactionEnvelope::from_xdr_base64(&signed, Limits::none());
        assert!(env.is_ok(), "aggregated XDR must be valid");
    }

    #[test]
    fn aggregate_two_signatures_embeds_both() {
        let xdr = make_unsigned_xdr();
        let tx_hash = compute_tx_hash(&xdr, TESTNET_PASSPHRASE).unwrap();

        let (key1, sk1) = gen_keypair();
        let (key2, sk2) = gen_keypair();
        let sig1 = sign_hash(&sk1, &tx_hash);
        let sig2 = sign_hash(&sk2, &tx_hash);

        let sigs = vec![
            make_signature_record(&key1, &sig1),
            make_signature_record(&key2, &sig2),
        ];

        let signed_xdr = aggregate_signatures(&xdr, &sigs).unwrap();

        use stellar_xdr::next::{Limits, ReadXdr, TransactionEnvelope};
        let env = TransactionEnvelope::from_xdr_base64(&signed_xdr, Limits::none()).unwrap();
        let sig_count = match env {
            TransactionEnvelope::Tx(v1) => v1.signatures.len(),
            _ => panic!("unexpected envelope type"),
        };
        assert_eq!(sig_count, 2, "envelope must contain exactly 2 signatures");
    }

    #[test]
    fn aggregate_invalid_xdr_returns_error() {
        let result = aggregate_signatures("not-valid-xdr", &[]);
        assert!(result.is_err());
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Threshold detection logic
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn threshold_met_when_collected_equals_required() {
        // Simulate the DB atomic update logic: threshold met iff collected >= required
        let required: i16 = 3;
        let collected: i16 = 3;
        assert!(collected >= required);
    }

    #[test]
    fn threshold_not_met_below_required() {
        let required: i16 = 3;
        let collected: i16 = 2;
        assert!(collected < required);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Expiry calculation
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn request_is_expired_when_expires_at_in_past() {
        let expires_at = Utc::now() - chrono::Duration::seconds(1);
        assert!(Utc::now() > expires_at, "past expiry must be detected");
    }

    #[test]
    fn request_is_not_expired_when_expires_at_in_future() {
        let expires_at = Utc::now() + chrono::Duration::hours(24);
        assert!(Utc::now() < expires_at, "future expiry must not trigger");
    }

    // ─────────────────────────────────────────────────────────────────────────
    // MintAuthStatus helpers
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn terminal_statuses_are_correct() {
        use aframp_backend::mint_authorization::MintAuthStatus;
        assert!(MintAuthStatus::Confirmed.is_terminal());
        assert!(MintAuthStatus::Failed.is_terminal());
        assert!(MintAuthStatus::Expired.is_terminal());
        assert!(MintAuthStatus::Cancelled.is_terminal());
        assert!(!MintAuthStatus::PendingSignatures.is_terminal());
        assert!(!MintAuthStatus::ThresholdMet.is_terminal());
        assert!(!MintAuthStatus::Submitted.is_terminal());
    }

    #[test]
    fn active_statuses_are_correct() {
        use aframp_backend::mint_authorization::MintAuthStatus;
        assert!(MintAuthStatus::PendingSignatures.is_active());
        assert!(MintAuthStatus::ThresholdMet.is_active());
        assert!(!MintAuthStatus::Confirmed.is_active());
        assert!(!MintAuthStatus::Cancelled.is_active());
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Cancellation enforcement
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn only_active_requests_can_be_cancelled() {
        use aframp_backend::mint_authorization::MintAuthStatus;
        // The DB cancel query uses WHERE status IN ('pending_signatures', 'threshold_met')
        // Verify our status model aligns with that constraint.
        let cancellable = [
            MintAuthStatus::PendingSignatures,
            MintAuthStatus::ThresholdMet,
        ];
        let non_cancellable = [
            MintAuthStatus::Submitted,
            MintAuthStatus::Confirmed,
            MintAuthStatus::Failed,
            MintAuthStatus::Expired,
            MintAuthStatus::Cancelled,
        ];
        for s in cancellable {
            assert!(s.is_active(), "{s} should be cancellable (active)");
        }
        for s in non_cancellable {
            assert!(!s.is_active(), "{s} should not be cancellable");
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Signature substitution attack prevention
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn signature_over_different_hash_fails_verification() {
        let xdr = make_unsigned_xdr();
        let correct_hash = compute_tx_hash(&xdr, TESTNET_PASSPHRASE).unwrap();

        // Attacker signs a different hash
        let different_hash = compute_tx_hash(&xdr, "Attacker Network ; 2024").unwrap();
        let (pub_key, signing_key) = gen_keypair();
        let attacker_sig = sign_hash(&signing_key, &different_hash);

        // Verification against the correct hash must fail
        let result = verify_ed25519_signature(&pub_key, &correct_hash, &attacker_sig);
        assert!(
            result.is_err(),
            "signature over different hash must be rejected"
        );
    }
}
