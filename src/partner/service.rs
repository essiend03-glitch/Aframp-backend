use chrono::Utc;
use rand::Rng;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{
    error::PartnerError,
    models::{
        DeprecationNotice, Partner, PartnerCredential, ProvisionCredentialRequest,
        ProvisionedCredential, RegisterPartnerRequest, ValidationResult,
    },
    repository::PartnerRepository,
};

const VALID_PARTNER_TYPES: &[&str] = &["bank", "fintech", "liquidity_provider"];
const VALID_CREDENTIAL_TYPES: &[&str] = &["oauth2_client", "mtls_cert", "api_key"];
const DEFAULT_RATE_LIMIT: i32 = 500;
const DEFAULT_API_VERSION: &str = "v1";

#[derive(Clone)]
pub struct PartnerService {
    repo: PartnerRepository,
}

impl PartnerService {
    pub fn new(repo: PartnerRepository) -> Self {
        Self { repo }
    }

    // ── Registration ──────────────────────────────────────────────────────────

    pub async fn register(&self, req: RegisterPartnerRequest) -> Result<Partner, PartnerError> {
        if !VALID_PARTNER_TYPES.contains(&req.partner_type.as_str()) {
            return Err(PartnerError::InvalidPartnerType(req.partner_type));
        }
        if self.repo.find_by_organisation(&req.organisation).await?.is_some() {
            return Err(PartnerError::AlreadyExists);
        }
        let now = Utc::now();
        let partner = Partner {
            id: Uuid::new_v4(),
            name: req.name,
            organisation: req.organisation,
            partner_type: req.partner_type,
            status: "sandbox".to_string(),
            contact_email: req.contact_email,
            ip_whitelist: req.ip_whitelist.unwrap_or_default(),
            rate_limit_per_minute: req.rate_limit_per_minute.unwrap_or(DEFAULT_RATE_LIMIT),
            api_version: req.api_version.unwrap_or_else(|| DEFAULT_API_VERSION.to_string()),
            created_at: now,
            updated_at: now,
        };
        self.repo.create(&partner).await
    }

    pub async fn get(&self, id: Uuid) -> Result<Partner, PartnerError> {
        self.repo.find_by_id(id).await
    }

    // ── Credential provisioning ───────────────────────────────────────────────

    pub async fn provision_credential(
        &self,
        partner_id: Uuid,
        req: ProvisionCredentialRequest,
    ) -> Result<ProvisionedCredential, PartnerError> {
        if !VALID_CREDENTIAL_TYPES.contains(&req.credential_type.as_str()) {
            return Err(PartnerError::InvalidCredentialType(req.credential_type));
        }
        let partner = self.repo.find_by_id(partner_id).await?;
        if partner.status == "suspended" {
            return Err(PartnerError::Suspended);
        }

        let scopes = req.scopes.unwrap_or_else(|| vec!["partner:read".to_string()]);
        let now = Utc::now();
        let cred_id = Uuid::new_v4();

        let (cred, secret) = match req.credential_type.as_str() {
            "oauth2_client" => {
                let client_id = format!("partner_{}", &cred_id.to_string()[..8]);
                let raw_secret = generate_secret(32);
                let secret_hash = sha256_hex(&raw_secret);
                let cred = PartnerCredential {
                    id: cred_id,
                    partner_id,
                    credential_type: "oauth2_client".to_string(),
                    client_id: Some(client_id.clone()),
                    client_secret_hash: Some(secret_hash),
                    certificate_fingerprint: None,
                    api_key_hash: None,
                    api_key_prefix: None,
                    scopes: scopes.clone(),
                    environment: req.environment.clone(),
                    expires_at: req.expires_at,
                    revoked_at: None,
                    created_at: now,
                };
                (cred, Some(raw_secret))
            }
            "api_key" => {
                let prefix = format!("pk_{}", &cred_id.to_string()[..8]);
                let raw_key = format!("{}.{}", prefix, generate_secret(40));
                let key_hash = sha256_hex(&raw_key);
                let cred = PartnerCredential {
                    id: cred_id,
                    partner_id,
                    credential_type: "api_key".to_string(),
                    client_id: None,
                    client_secret_hash: None,
                    certificate_fingerprint: None,
                    api_key_hash: Some(key_hash),
                    api_key_prefix: Some(prefix.clone()),
                    scopes: scopes.clone(),
                    environment: req.environment.clone(),
                    expires_at: req.expires_at,
                    revoked_at: None,
                    created_at: now,
                };
                (cred, Some(raw_key))
            }
            "mtls_cert" => {
                let pem = req.certificate_pem.ok_or_else(|| {
                    PartnerError::ValidationFailed("certificate_pem required for mtls_cert".into())
                })?;
                let fingerprint = sha256_hex(pem.trim());
                let cred = PartnerCredential {
                    id: cred_id,
                    partner_id,
                    credential_type: "mtls_cert".to_string(),
                    client_id: None,
                    client_secret_hash: None,
                    certificate_fingerprint: Some(fingerprint.clone()),
                    api_key_hash: None,
                    api_key_prefix: None,
                    scopes: scopes.clone(),
                    environment: req.environment.clone(),
                    expires_at: req.expires_at,
                    revoked_at: None,
                    created_at: now,
                };
                (cred, None)
            }
            _ => unreachable!(),
        };

        let saved = self.repo.create_credential(&cred).await?;
        Ok(ProvisionedCredential {
            credential_id: saved.id,
            credential_type: saved.credential_type,
            environment: saved.environment,
            secret,
            client_id: saved.client_id,
            api_key_prefix: saved.api_key_prefix,
            certificate_fingerprint: saved.certificate_fingerprint,
            scopes,
            expires_at: saved.expires_at,
        })
    }

    // ── Validation engine ─────────────────────────────────────────────────────

    /// Run the automated certification test suite against a partner's sandbox
    /// credentials. Returns per-test results; all must pass for production access.
    pub async fn run_validation(&self, partner_id: Uuid) -> Result<Vec<ValidationResult>, PartnerError> {
        let partner = self.repo.find_by_id(partner_id).await?;
        let now = Utc::now();
        let mut results = Vec::new();

        // Test 1: partner is in sandbox status
        results.push(ValidationResult {
            partner_id,
            test_name: "sandbox_status".to_string(),
            passed: partner.status == "sandbox",
            detail: format!("Partner status is '{}'", partner.status),
            tested_at: now,
        });

        // Test 2: at least one active credential provisioned
        let has_cred = self
            .repo
            .has_active_credential(partner_id)
            .await
            .unwrap_or(false);
        results.push(ValidationResult {
            partner_id,
            test_name: "credential_provisioned".to_string(),
            passed: has_cred,
            detail: if has_cred {
                "Active credential found".to_string()
            } else {
                "No active credential found — provision one first".to_string()
            },
            tested_at: now,
        });

        // Test 3: IP whitelist configured (warn only — not a hard fail)
        results.push(ValidationResult {
            partner_id,
            test_name: "ip_whitelist_configured".to_string(),
            passed: !partner.ip_whitelist.is_empty(),
            detail: if partner.ip_whitelist.is_empty() {
                "No IP whitelist — recommended for production".to_string()
            } else {
                format!("{} IP(s) whitelisted", partner.ip_whitelist.len())
            },
            tested_at: now,
        });

        // Test 4: API version not deprecated
        let deprecated = self
            .repo
            .deprecation_for_version(&partner.api_version)
            .await?;
        results.push(ValidationResult {
            partner_id,
            test_name: "api_version_current".to_string(),
            passed: deprecated.is_none(),
            detail: match &deprecated {
                None => format!("API version '{}' is current", partner.api_version),
                Some(d) => format!(
                    "API version '{}' is deprecated — sunset {}",
                    partner.api_version, d.sunset_at
                ),
            },
            tested_at: now,
        });

        Ok(results)
    }

    // ── Deprecation notices ───────────────────────────────────────────────────

    pub async fn deprecation_notices(&self) -> Result<Vec<DeprecationNotice>, PartnerError> {
        let rows = self.repo.active_deprecations().await?;
        Ok(rows
            .into_iter()
            .map(|d| {
                let days = (d.sunset_at - Utc::now()).num_days().max(0);
                DeprecationNotice {
                    api_version: d.api_version,
                    deprecated_at: d.deprecated_at,
                    sunset_at: d.sunset_at,
                    migration_guide_url: d.migration_guide_url,
                    days_until_sunset: days,
                }
            })
            .collect())
    }

    // ── Promote to production ─────────────────────────────────────────────────

    /// Promote a sandbox partner to production status.
    /// All validation tests must pass before promotion is allowed.
    pub async fn promote_to_production(&self, partner_id: Uuid) -> Result<Partner, PartnerError> {
        let partner = self.repo.find_by_id(partner_id).await?;
        if partner.status != "sandbox" {
            return Err(PartnerError::ValidationFailed(format!(
                "Partner must be in sandbox status to promote; current status is '{}'",
                partner.status
            )));
        }

        let results = self.run_validation(partner_id).await?;
        let failed: Vec<_> = results.iter().filter(|r| !r.passed).collect();
        if !failed.is_empty() {
            let names: Vec<&str> = failed.iter().map(|r| r.test_name.as_str()).collect();
            return Err(PartnerError::ValidationFailed(format!(
                "Certification tests failed: {}",
                names.join(", ")
            )));
        }

        self.repo.update_status(partner_id, "active").await?;
        self.repo.find_by_id(partner_id).await
    }

    // ── Revoke credential ─────────────────────────────────────────────────────

    pub async fn revoke_credential(
        &self,
        partner_id: Uuid,
        credential_id: Uuid,
    ) -> Result<(), PartnerError> {
        // Verify the credential belongs to this partner
        let cred = self.repo.find_credential_by_id(credential_id).await?;
        if cred.partner_id != partner_id {
            return Err(PartnerError::CredentialNotFound);
        }
        self.repo.revoke_credential(credential_id).await
    }

    // ── Rate limit check ──────────────────────────────────────────────────────

    pub async fn check_rate_limit(&self, partner_id: Uuid) -> Result<(), PartnerError> {
        let partner = self.repo.find_by_id(partner_id).await?;
        let count = self.repo.increment_rate_counter(partner_id).await?;
        if count > partner.rate_limit_per_minute as i64 {
            return Err(PartnerError::RateLimitExceeded);
        }
        Ok(())
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn generate_secret(len: usize) -> String {
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| rng.sample(rand::distributions::Alphanumeric) as char)
        .collect()
}

fn sha256_hex(input: &str) -> String {
    let mut h = Sha256::new();
    h.update(input.as_bytes());
    hex::encode(h.finalize())
}
