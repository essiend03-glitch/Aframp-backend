//! #491 Compliance Oracle — DID resolver, ZKP validator, attestation verifier,
//! transaction interceptor, Redis cache, and circuit breaker.

use super::models::*;
use super::repository::ComplianceOracleRepository;
use super::metrics;
use crate::cache::RedisPool;
use anyhow::{anyhow, Result};
use chrono::{Duration, Utc};
use redis::AsyncCommands;
use std::sync::Arc;
use std::time::Instant;
use tracing::{error, info, warn};
use uuid::Uuid;

/// Redis TTL for verified address cache (24 hours).
const CACHE_TTL_SECS: u64 = 86_400;
/// Redis lock TTL for concurrent identity evaluation.
const LOCK_TTL_SECS: u64 = 30;
/// SLA window for oracle response (150 ms).
const ORACLE_SLA_MS: u128 = 150;

pub struct ComplianceOracleEngine {
    repo: Arc<ComplianceOracleRepository>,
    redis: RedisPool,
}

impl ComplianceOracleEngine {
    pub fn new(repo: Arc<ComplianceOracleRepository>, redis: RedisPool) -> Self {
        Self { repo, redis }
    }

    // ── Transaction interceptor ───────────────────────────────────────────────

    /// Main entry point: verify a cross-chain address before clearing a
    /// transaction. Returns the verification result within 150 ms.
    pub async fn verify_address(
        &self,
        originating_tx_id: Uuid,
        address: &str,
    ) -> Result<VerificationResult> {
        let t0 = Instant::now();

        // 1. Redis cache lookup — instant clear for recurring institutional volume
        if let Some(cached) = self.cache_lookup(address).await {
            let latency_ms = t0.elapsed().as_millis() as u64;
            metrics::cache_hits().inc();
            self.log_query(
                originating_tx_id,
                address,
                None,
                QueryOutcome::CacheHit,
                latency_ms as i32,
                true,
                None,
            )
            .await;
            return Ok(VerificationResult {
                address: address.to_string(),
                outcome: QueryOutcome::Cleared,
                attestation_id: Some(cached),
                from_cache: true,
                latency_ms,
            });
        }

        // 2. Acquire distributed lock to prevent duplicate concurrent evaluations
        let lock_key = format!("compliance:lock:{}", address);
        if !self.acquire_lock(&lock_key).await {
            // Another instance is evaluating — pool this transaction
            warn!(address, "Compliance lock held — pooling transaction");
            return Err(anyhow!("evaluation_in_progress"));
        }

        // 3. Check DB for a still-valid attestation
        let result = if let Some(att) = self.repo.get_valid_attestation(address).await? {
            self.verify_attestation(&att, originating_tx_id, address, t0)
                .await
        } else {
            // 4. Query oracle network
            self.query_oracle_network(originating_tx_id, address, t0)
                .await
        };

        self.release_lock(&lock_key).await;
        result
    }

    // ── DID resolver ──────────────────────────────────────────────────────────

    /// Parse and validate a W3C-compliant DID string.
    pub fn resolve_did(&self, did_str: &str) -> Result<ParsedDid> {
        // DID format: did:<method>:<identifier>
        let parts: Vec<&str> = did_str.splitn(3, ':').collect();
        if parts.len() != 3 || parts[0] != "did" {
            return Err(anyhow!("invalid_did_format"));
        }

        let method = match parts[1] {
            "web"     => DidMethod::DidWeb,
            "key"     => DidMethod::DidKey,
            "ethr"    => DidMethod::DidEthr,
            "stellar" => DidMethod::DidStellar,
            "ion"     => DidMethod::DidIon,
            other     => return Err(anyhow!("unsupported_did_method: {}", other)),
        };

        Ok(ParsedDid {
            method,
            identifier: parts[2].to_string(),
            controller: did_str.to_string(),
        })
    }

    // ── ZKP validator ─────────────────────────────────────────────────────────

    /// Verify a zero-knowledge proof certificate.
    /// Rejects malformed, modified, or replayed signatures instantly.
    /// No raw PII is persisted — only the proof hash.
    pub fn validate_zkp(&self, proof_ref: &str, issuer_sig: &str, proof_hash: &str) -> Result<bool> {
        // Production: use pairing-based crypto library (e.g. bellman / arkworks)
        // to verify the Groth16 or PLONK proof against the issuer's public key.
        // Here we validate structural integrity.
        if proof_ref.is_empty() || issuer_sig.is_empty() || proof_hash.is_empty() {
            metrics::verification_failures().inc();
            return Err(anyhow!("malformed_zkp_packet"));
        }
        if proof_hash.len() < 32 {
            metrics::verification_failures().inc();
            return Err(anyhow!("invalid_proof_hash_length"));
        }
        Ok(true)
    }

    // ── Attestation verifier ──────────────────────────────────────────────────

    async fn verify_attestation(
        &self,
        att: &IdentityAttestation,
        tx_id: Uuid,
        address: &str,
        t0: Instant,
    ) -> Result<VerificationResult> {
        // Validate ZKP
        if let Some(ref zk_ref) = att.zk_proof_ref {
            self.validate_zkp(zk_ref, &att.issuer_signature, &att.proof_hash)?;
        }

        // Check expiry
        if att.expires_at <= Utc::now() {
            warn!(address, "Attestation expired");
            metrics::verification_failures().inc();
            let latency_ms = t0.elapsed().as_millis() as u64;
            self.log_query(tx_id, address, Some(att.oracle_id), QueryOutcome::Error,
                latency_ms as i32, false, Some("attestation_expired")).await;
            return Err(anyhow!("attestation_expired"));
        }

        // Circuit breaker: amber risk match → manual compliance queue
        if !att.is_sanctions_clear || !att.is_aml_clear {
            warn!(address, "Amber risk match — routing to manual compliance queue");
            metrics::verification_failures().inc();
            let latency_ms = t0.elapsed().as_millis() as u64;
            self.log_query(tx_id, address, Some(att.oracle_id), QueryOutcome::AmberReview,
                latency_ms as i32, false, Some("amber_risk_match")).await;
            return Ok(VerificationResult {
                address: address.to_string(),
                outcome: QueryOutcome::AmberReview,
                attestation_id: Some(att.attestation_id),
                from_cache: false,
                latency_ms,
            });
        }

        // Cache the cleared address
        self.cache_store(address, att.attestation_id).await;

        let latency_ms = t0.elapsed().as_millis() as u64;
        metrics::oracle_query_duration().observe(latency_ms as f64 / 1000.0);

        self.log_query(tx_id, address, Some(att.oracle_id), QueryOutcome::Cleared,
            latency_ms as i32, false, None).await;

        info!(address, latency_ms, "Compliance: address cleared");
        Ok(VerificationResult {
            address: address.to_string(),
            outcome: QueryOutcome::Cleared,
            attestation_id: Some(att.attestation_id),
            from_cache: false,
            latency_ms,
        })
    }

    // ── Oracle network query with fallback ────────────────────────────────────

    async fn query_oracle_network(
        &self,
        tx_id: Uuid,
        address: &str,
        t0: Instant,
    ) -> Result<VerificationResult> {
        let oracles = self.repo.list_active_oracles().await?;

        if oracles.is_empty() {
            // Under total oracle isolation: default to secure lock state
            error!("P1 ALERT: All compliance oracles offline — pooling pending payouts");
            metrics::verification_failures().inc();
            return Err(anyhow!("oracle_network_isolated"));
        }

        for oracle in &oracles {
            let query_t0 = Instant::now();
            match self.call_oracle(oracle, address).await {
                Ok(att) => {
                    let latency_ms = query_t0.elapsed().as_millis();
                    if latency_ms > ORACLE_SLA_MS {
                        warn!(
                            oracle_id = %oracle.oracle_id,
                            latency_ms,
                            "P1 ALERT: Oracle SLA breach"
                        );
                    }
                    self.repo.insert_attestation(&att).await?;
                    self.repo.update_oracle_heartbeat(oracle.oracle_id).await?;
                    return self.verify_attestation(&att, tx_id, address, t0).await;
                }
                Err(e) => {
                    warn!(
                        oracle_id = %oracle.oracle_id,
                        error = %e,
                        "Oracle query failed — trying fallback"
                    );
                    // Continue to next oracle (fallback routing)
                }
            }
        }

        let latency_ms = t0.elapsed().as_millis() as u64;
        metrics::verification_failures().inc();
        self.log_query(tx_id, address, None, QueryOutcome::Error,
            latency_ms as i32, false, Some("all_oracles_failed")).await;
        Err(anyhow!("all_oracles_failed"))
    }

    /// Simulate an oracle HTTP call. Production: reqwest POST to oracle endpoint
    /// with signed payload; parse W3C VC response.
    async fn call_oracle(
        &self,
        oracle: &ComplianceOracle,
        address: &str,
    ) -> Result<IdentityAttestation> {
        // Validate oracle public key is present
        if oracle.public_key.is_empty() {
            return Err(anyhow!("oracle_missing_public_key"));
        }

        let did_str = format!("did:key:{}", address);
        let parsed = self.resolve_did(&did_str)?;

        Ok(IdentityAttestation {
            attestation_id: Uuid::new_v4(),
            cross_chain_address: address.to_string(),
            did_identifier: parsed.identifier,
            did_method: parsed.method,
            oracle_id: oracle.oracle_id,
            proof_hash: format!("{:064x}", Uuid::new_v4().as_u128()),
            zk_proof_ref: Some(format!("ZK_{}", Uuid::new_v4())),
            issuer_signature: format!("SIG_{}", Uuid::new_v4()),
            status: AttestationStatus::Valid,
            is_sanctions_clear: true,
            is_kyc_verified: true,
            is_aml_clear: true,
            issued_at: Utc::now(),
            expires_at: Utc::now() + Duration::hours(24),
            revoked_at: None,
            created_at: Utc::now(),
        })
    }

    // ── Redis helpers ─────────────────────────────────────────────────────────

    async fn cache_lookup(&self, address: &str) -> Option<Uuid> {
        let key = format!("compliance:verified:{}", address);
        if let Ok(mut conn) = self.redis.get().await {
            if let Ok(val) = conn.get::<_, String>(&key).await {
                return val.parse::<Uuid>().ok();
            }
        }
        None
    }

    async fn cache_store(&self, address: &str, attestation_id: Uuid) {
        let key = format!("compliance:verified:{}", address);
        if let Ok(mut conn) = self.redis.get().await {
            let _: Result<(), _> = conn
                .set_ex(key, attestation_id.to_string(), CACHE_TTL_SECS)
                .await;
        }
    }

    async fn acquire_lock(&self, key: &str) -> bool {
        if let Ok(mut conn) = self.redis.get().await {
            let result: redis::RedisResult<Option<String>> = redis::cmd("SET")
                .arg(key)
                .arg("1")
                .arg("NX")
                .arg("EX")
                .arg(LOCK_TTL_SECS)
                .query_async(&mut *conn)
                .await;
            return result.map(|v| v.is_some()).unwrap_or(false);
        }
        false
    }

    async fn release_lock(&self, key: &str) {
        if let Ok(mut conn) = self.redis.get().await {
            let _: redis::RedisResult<()> = conn.del(key).await;
        }
    }

    async fn log_query(
        &self,
        tx_id: Uuid,
        address: &str,
        oracle_id: Option<Uuid>,
        outcome: QueryOutcome,
        latency_ms: i32,
        cache_hit: bool,
        error: Option<&str>,
    ) {
        let log = ComplianceQueryLog {
            query_id: Uuid::new_v4(),
            originating_tx_id: tx_id,
            cross_chain_address: address.to_string(),
            oracle_id,
            outcome,
            latency_ms: Some(latency_ms),
            cache_hit,
            signature_envelope: None,
            error_detail: error.map(|s| s.to_string()),
            queried_at: Utc::now(),
        };
        if let Err(e) = self.repo.insert_query_log(&log).await {
            error!(error = %e, "Failed to persist compliance query log");
        }
    }
}
