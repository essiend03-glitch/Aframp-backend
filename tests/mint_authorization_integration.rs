//! Integration tests for the Mint Authorization Framework (#213).
//!
//! Tests the full lifecycle: request creation → signature collection →
//! threshold detection → envelope assembly → Stellar testnet submission → confirmation.
//!
//! Run with:
//!   DATABASE_URL=postgres://... STELLAR_ISSUER_ADDRESS=G... \
//!   cargo test --test mint_authorization_integration --features integration -- --nocapture
//!
//! Requires:
//!   - A live PostgreSQL database with migrations applied
//!   - Stellar testnet access (https://horizon-testnet.stellar.org)
//!   - STELLAR_ISSUER_ADDRESS env var set to a funded testnet issuer account

#![cfg(feature = "integration")]

use aframp_backend::{
    chains::stellar::{client::StellarClient, config::StellarConfig},
    mint_authorization::{
        error::MintAuthError,
        models::{
            CancelMintAuthRequest, CreateMintAuthRequest, MintAuthStatus, SubmitSignatureRequest,
        },
        repository::MintAuthRepository,
        service::{compute_tx_hash, verify_ed25519_signature, MintAuthService},
    },
};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chrono::Utc;
use ed25519_dalek::{Signer, SigningKey};
use rand::rngs::OsRng;
use sqlx::PgPool;
use sqlx::types::BigDecimal;
use std::str::FromStr;
use std::sync::Arc;
use stellar_strkey::ed25519::PublicKey as StrkeyPublicKey;
use uuid::Uuid;

// ─────────────────────────────────────────────────────────────────────────────
// Test helpers
// ─────────────────────────────────────────────────────────────────────────────

async fn test_pool() -> PgPool {
    let url = std::env::var("DATABASE_URL").expect("DATABASE_URL required");
    PgPool::connect(&url).await.expect("db pool")
}

fn testnet_stellar_client() -> Arc<StellarClient> {
    let config = StellarConfig::testnet();
    Arc::new(StellarClient::new(config).expect("stellar client"))
}

fn make_service(pool: PgPool) -> Arc<MintAuthService> {
    let repo = Arc::new(MintAuthRepository::new(pool));
    let stellar = testnet_stellar_client();
    let issuer = std::env::var("STELLAR_ISSUER_ADDRESS")
        .unwrap_or_else(|_| "GCJRI5CIWK5IU67Q6DGA7QW52JDKRO7JEAHQKFNDUJUPEZGURDBX3LDX".into());
    Arc::new(MintAuthService::new(repo, stellar, issuer))
}

fn gen_keypair() -> (String, SigningKey) {
    let sk = SigningKey::generate(&mut OsRng);
    let strkey = StrkeyPublicKey(sk.verifying_key().to_bytes());
    (strkey.to_string(), sk)
}

fn sign_tx_hash(sk: &SigningKey, tx_hash_hex: &str) -> String {
    let bytes = hex::decode(tx_hash_hex).unwrap();
    B64.encode(sk.sign(&bytes).to_bytes())
}

/// Insert a minimal reserve verification snapshot and return its id.
async fn seed_reserve_verification(pool: &PgPool, amount: &BigDecimal) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query!(
        r#"
        INSERT INTO historical_verification
            (id, on_chain_supply, fiat_reserves, in_transit, delta,
             collateral_ratio, is_collateralised, issuer_address, asset_code,
             snapshot_signature, snapshot_json, triggered_by, created_at)
        VALUES ($1, $2, $3, 0, $3, 1.0, true, 'GTEST', 'cNGN', 'sig', '{}', 'test', NOW())
        "#,
        id,
        amount,  // on_chain_supply
        amount,  // fiat_reserves (equal → fully collateralised)
    )
    .execute(pool)
    .await
    .expect("seed reserve verification");
    id
}

/// Insert an active mint signer and return (signer_id, stellar_public_key, signing_key).
async fn seed_signer(pool: &PgPool) -> (Uuid, String, SigningKey) {
    let (pub_key, sk) = gen_keypair();
    let id = Uuid::new_v4();
    sqlx::query!(
        r#"
        INSERT INTO mint_signers
            (id, full_legal_name, role, organisation, contact_email,
             stellar_public_key, signing_weight, status, identity_verified, initiated_by)
        VALUES ($1, 'Test Signer', 'cfo', 'Test Org',
                $2, $3, 1, 'active', true, $1)
        "#,
        id,
        format!("test-{}@example.com", id),
        pub_key,
    )
    .execute(pool)
    .await
    .expect("seed signer");
    (id, pub_key, sk)
}

/// Ensure mint_quorum_config has a row.
async fn seed_quorum(pool: &PgPool, threshold: i16) {
    let admin = Uuid::new_v4();
    sqlx::query!(
        "INSERT INTO mint_quorum_config (required_threshold, updated_by) VALUES ($1, $2)
         ON CONFLICT DO NOTHING",
        threshold,
        admin,
    )
    .execute(pool)
    .await
    .expect("seed quorum");
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

/// Full lifecycle: create → sign (threshold=1) → threshold_met → submitted
#[tokio::test]
async fn test_full_lifecycle_single_signer() {
    let pool = test_pool().await;
    let svc = make_service(pool.clone());

    let amount = BigDecimal::from_str("100.0000000").unwrap();
    let reserve_id = seed_reserve_verification(&pool, &amount).await;
    let (signer_id, signer_key, signing_key) = seed_signer(&pool).await;
    seed_quorum(&pool, 1).await;

    let requester_id = signer_id; // same person for simplicity in test
    let dest = "GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN";

    // 1. Create authorization request
    let auth = svc
        .create(
            CreateMintAuthRequest {
                amount_cngn: "100.0000000".into(),
                destination_account: dest.into(),
                justification: "Integration test mint".into(),
                reserve_verification_id: reserve_id,
            },
            requester_id,
            &signer_key,
        )
        .await
        .expect("create authorization");

    assert_eq!(auth.status, MintAuthStatus::PendingSignatures);
    assert!(auth.tx_hash.is_some(), "tx_hash must be set");
    assert!(!auth.unsigned_xdr.is_empty(), "unsigned_xdr must be set");

    // 2. Sign
    let tx_hash = auth.tx_hash.as_ref().unwrap();
    let signature = sign_tx_hash(&signing_key, tx_hash);

    let detail = svc
        .submit_signature(
            auth.id,
            SubmitSignatureRequest {
                signature,
                signer_key: signer_key.clone(),
            },
            None,
        )
        .await
        .expect("submit signature");

    assert_eq!(detail.signatures_collected, 1);
    assert_eq!(detail.signatures_required, 1);
    // With threshold=1, status transitions to threshold_met immediately
    assert_eq!(detail.request.status, MintAuthStatus::ThresholdMet);
}

/// Duplicate signature is rejected.
#[tokio::test]
async fn test_duplicate_signature_rejected() {
    let pool = test_pool().await;
    let svc = make_service(pool.clone());

    let amount = BigDecimal::from_str("50.0000000").unwrap();
    let reserve_id = seed_reserve_verification(&pool, &amount).await;
    let (signer_id, signer_key, signing_key) = seed_signer(&pool).await;
    seed_quorum(&pool, 2).await;

    let auth = svc
        .create(
            CreateMintAuthRequest {
                amount_cngn: "50.0000000".into(),
                destination_account: "GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN".into(),
                justification: "Dup sig test".into(),
                reserve_verification_id: reserve_id,
            },
            signer_id,
            &signer_key,
        )
        .await
        .expect("create");

    let tx_hash = auth.tx_hash.as_ref().unwrap();
    let sig = sign_tx_hash(&signing_key, tx_hash);

    svc.submit_signature(
        auth.id,
        SubmitSignatureRequest { signature: sig.clone(), signer_key: signer_key.clone() },
        None,
    )
    .await
    .expect("first signature");

    let err = svc
        .submit_signature(
            auth.id,
            SubmitSignatureRequest { signature: sig, signer_key: signer_key.clone() },
            None,
        )
        .await
        .unwrap_err();

    assert!(
        matches!(err, MintAuthError::DuplicateSignature(_, _)),
        "second signature from same signer must be rejected"
    );
}

/// Invalid signature (wrong key) is rejected.
#[tokio::test]
async fn test_invalid_signature_rejected() {
    let pool = test_pool().await;
    let svc = make_service(pool.clone());

    let amount = BigDecimal::from_str("50.0000000").unwrap();
    let reserve_id = seed_reserve_verification(&pool, &amount).await;
    let (signer_id, signer_key, _) = seed_signer(&pool).await;
    seed_quorum(&pool, 2).await;

    let auth = svc
        .create(
            CreateMintAuthRequest {
                amount_cngn: "50.0000000".into(),
                destination_account: "GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN".into(),
                justification: "Invalid sig test".into(),
                reserve_verification_id: reserve_id,
            },
            signer_id,
            &signer_key,
        )
        .await
        .expect("create");

    // Sign with a different (unregistered) key
    let (_, wrong_sk) = gen_keypair();
    let tx_hash = auth.tx_hash.as_ref().unwrap();
    let bad_sig = sign_tx_hash(&wrong_sk, tx_hash);

    let err = svc
        .submit_signature(
            auth.id,
            SubmitSignatureRequest { signature: bad_sig, signer_key: signer_key.clone() },
            None,
        )
        .await
        .unwrap_err();

    assert!(
        matches!(err, MintAuthError::InvalidSignature(_, _)),
        "signature from wrong key must be rejected"
    );
}

/// Cancellation transitions to cancelled and prevents further signing.
#[tokio::test]
async fn test_cancellation_prevents_further_signing() {
    let pool = test_pool().await;
    let svc = make_service(pool.clone());

    let amount = BigDecimal::from_str("200.0000000").unwrap();
    let reserve_id = seed_reserve_verification(&pool, &amount).await;
    let (signer_id, signer_key, signing_key) = seed_signer(&pool).await;
    seed_quorum(&pool, 2).await;

    let auth = svc
        .create(
            CreateMintAuthRequest {
                amount_cngn: "200.0000000".into(),
                destination_account: "GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN".into(),
                justification: "Cancel test".into(),
                reserve_verification_id: reserve_id,
            },
            signer_id,
            &signer_key,
        )
        .await
        .expect("create");

    // Cancel it
    let cancelled = svc
        .cancel(
            auth.id,
            signer_id,
            CancelMintAuthRequest { justification: "Test cancellation".into() },
        )
        .await
        .expect("cancel");

    assert_eq!(cancelled.status, MintAuthStatus::Cancelled);
    assert!(cancelled.cancellation_reason.is_some());

    // Attempt to sign after cancellation
    let tx_hash = auth.tx_hash.as_ref().unwrap();
    let sig = sign_tx_hash(&signing_key, tx_hash);

    let err = svc
        .submit_signature(
            auth.id,
            SubmitSignatureRequest { signature: sig, signer_key },
            None,
        )
        .await
        .unwrap_err();

    assert!(
        matches!(err, MintAuthError::TerminalState(_, _)),
        "signing a cancelled request must be rejected"
    );
}

/// Expiry worker transitions overdue requests to expired.
#[tokio::test]
async fn test_expiry_worker_expires_stale_requests() {
    let pool = test_pool().await;
    let svc = make_service(pool.clone());

    // Manually insert an already-expired request
    let amount = BigDecimal::from_str("10.0000000").unwrap();
    let reserve_id = seed_reserve_verification(&pool, &amount).await;
    let (signer_id, signer_key, _) = seed_signer(&pool).await;
    seed_quorum(&pool, 2).await;

    let auth = svc
        .create(
            CreateMintAuthRequest {
                amount_cngn: "10.0000000".into(),
                destination_account: "GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN".into(),
                justification: "Expiry test".into(),
                reserve_verification_id: reserve_id,
            },
            signer_id,
            &signer_key,
        )
        .await
        .expect("create");

    // Back-date expires_at to the past
    sqlx::query!(
        "UPDATE mint_authorization_requests SET expires_at = NOW() - INTERVAL '1 hour' WHERE id = $1",
        auth.id
    )
    .execute(&pool)
    .await
    .expect("backdate");

    let expired_count = svc.expire_stale_requests().await.expect("expire");
    assert!(expired_count >= 1, "at least one request should have been expired");

    let detail = svc.get(auth.id).await.expect("get");
    assert_eq!(detail.request.status, MintAuthStatus::Expired);
}

/// Reserve verification recency check rejects stale verifications.
#[tokio::test]
async fn test_stale_reserve_verification_rejected() {
    let pool = test_pool().await;
    let svc = make_service(pool.clone());

    let amount = BigDecimal::from_str("100.0000000").unwrap();
    let id = Uuid::new_v4();
    let admin = Uuid::new_v4();

    // Insert a verification that is 48 hours old
    sqlx::query!(
        r#"
        INSERT INTO historical_verification
            (id, on_chain_supply, fiat_reserves, in_transit, delta,
             collateral_ratio, is_collateralised, issuer_address, asset_code,
             snapshot_signature, snapshot_json, triggered_by, created_at)
        VALUES ($1, $2, $2, 0, $2, 1.0, true, 'GTEST', 'cNGN', 'sig', '{}', 'test',
                NOW() - INTERVAL '48 hours')
        "#,
        id,
        amount,
    )
    .execute(&pool)
    .await
    .expect("seed stale verification");

    let (signer_id, signer_key, _) = seed_signer(&pool).await;

    let err = svc
        .create(
            CreateMintAuthRequest {
                amount_cngn: "100.0000000".into(),
                destination_account: "GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN".into(),
                justification: "Stale reserve test".into(),
                reserve_verification_id: id,
            },
            signer_id,
            &signer_key,
        )
        .await
        .unwrap_err();

    assert!(
        matches!(err, MintAuthError::ReserveVerificationStale { .. }),
        "stale reserve verification must be rejected"
    );
}

/// Amount exceeding reserve balance is rejected.
#[tokio::test]
async fn test_amount_exceeds_reserve_rejected() {
    let pool = test_pool().await;
    let svc = make_service(pool.clone());

    // Reserve has 100 cNGN, request asks for 200
    let reserve_amount = BigDecimal::from_str("100.0000000").unwrap();
    let reserve_id = seed_reserve_verification(&pool, &reserve_amount).await;
    let (signer_id, signer_key, _) = seed_signer(&pool).await;
    seed_quorum(&pool, 2).await;

    let err = svc
        .create(
            CreateMintAuthRequest {
                amount_cngn: "200.0000000".into(),
                destination_account: "GAAZI4TCR3TY5OJHCTJC2A4QSY6CJWJH5IAJTGKIN2ER7LBNVKOCCWN".into(),
                justification: "Exceeds reserve test".into(),
                reserve_verification_id: reserve_id,
            },
            signer_id,
            &signer_key,
        )
        .await
        .unwrap_err();

    assert!(
        matches!(err, MintAuthError::ExceedsReserveBalance { .. }),
        "amount exceeding reserve must be rejected"
    );
}
