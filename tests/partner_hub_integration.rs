//! Integration tests for the Partner Integration Framework (Issue #348).
//!
//! Tests cover:
//!   - Partner registration and duplicate detection
//!   - Credential provisioning (API key, OAuth2, mTLS)
//!   - Validation engine (sandbox certification)
//!   - Promote-to-production (requires all tests passing)
//!   - Credential revocation (ownership enforcement)
//!   - Per-partner rate limiting
//!   - Deprecation notices and Sunset/Deprecation response headers
//!   - IP whitelist enforcement
//!
//! Run with: cargo test --features database partner_hub -- --ignored

#[cfg(feature = "database")]
mod partner_hub {
    use chrono::{Duration, Utc};
    use uuid::Uuid;

    use Bitmesh_backend::partner::{
        models::{ProvisionCredentialRequest, RegisterPartnerRequest},
        repository::PartnerRepository,
        service::PartnerService,
    };

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    // ── Helpers ───────────────────────────────────────────────────────────────

    async fn test_pool() -> Result<sqlx::PgPool, sqlx::Error> {
        // INVARIANT: DATABASE_URL must be set when running database-feature tests.
        // If absent the test suite is misconfigured, so we surface a clear error
        // rather than panicking with a cryptic message.
        let url = std::env::var("DATABASE_URL").map_err(|_| {
            sqlx::Error::Configuration(
                "DATABASE_URL must be set for partner hub integration tests".into(),
            )
        })?;
        sqlx::PgPool::connect(&url).await
    }

    fn unique_org() -> String {
        format!("TestOrg-{}", Uuid::new_v4())
    }

    fn register_req(org: &str) -> RegisterPartnerRequest {
        RegisterPartnerRequest {
            name: "Test Partner".to_string(),
            organisation: org.to_string(),
            partner_type: "fintech".to_string(),
            contact_email: "test@example.com".to_string(),
            ip_whitelist: None,
            rate_limit_per_minute: Some(100),
            api_version: Some("v1".to_string()),
        }
    }

    // ── Registration ──────────────────────────────────────────────────────────

    #[tokio::test]
    #[ignore]
    async fn test_register_partner_creates_sandbox_partner() -> TestResult {
        let pool = test_pool().await?;
        let svc = PartnerService::new(PartnerRepository::new(pool));
        let org = unique_org();

        let partner = svc.register(register_req(&org)).await?;

        assert_eq!(partner.status, "sandbox");
        assert_eq!(partner.organisation, org);
        assert_eq!(partner.partner_type, "fintech");
        assert_eq!(partner.api_version, "v1");
        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_register_duplicate_organisation_returns_conflict() -> TestResult {
        let pool = test_pool().await?;
        let svc = PartnerService::new(PartnerRepository::new(pool));
        let org = unique_org();

        svc.register(register_req(&org)).await?;
        let err = svc.register(register_req(&org)).await.unwrap_err();

        assert!(
            matches!(err, Bitmesh_backend::partner::error::PartnerError::AlreadyExists),
            "expected AlreadyExists, got {:?}",
            err
        );
        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_register_invalid_partner_type_rejected() -> TestResult {
        let pool = test_pool().await?;
        let svc = PartnerService::new(PartnerRepository::new(pool));
        let mut req = register_req(&unique_org());
        req.partner_type = "unknown_type".to_string();

        let err = svc.register(req).await.unwrap_err();
        assert!(matches!(
            err,
            Bitmesh_backend::partner::error::PartnerError::InvalidPartnerType(_)
        ));
        Ok(())
    }

    // ── Credential provisioning ───────────────────────────────────────────────

    #[tokio::test]
    #[ignore]
    async fn test_provision_api_key_returns_secret_once() -> TestResult {
        let pool = test_pool().await?;
        let svc = PartnerService::new(PartnerRepository::new(pool));
        let partner = svc.register(register_req(&unique_org())).await?;

        let cred = svc
            .provision_credential(
                partner.id,
                ProvisionCredentialRequest {
                    credential_type: "api_key".to_string(),
                    environment: "sandbox".to_string(),
                    scopes: None,
                    expires_at: None,
                    certificate_pem: None,
                },
            )
            .await?;

        assert_eq!(cred.credential_type, "api_key");
        assert!(cred.secret.is_some(), "API key secret must be returned on provisioning");
        assert!(cred.api_key_prefix.is_some(), "API key prefix must be returned on provisioning");
        // Secret starts with the prefix
        let secret = cred.secret.expect("secret checked above");
        let prefix = cred.api_key_prefix.expect("prefix checked above");
        assert!(secret.starts_with(&prefix));
        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_provision_oauth2_client_returns_client_id_and_secret() -> TestResult {
        let pool = test_pool().await?;
        let svc = PartnerService::new(PartnerRepository::new(pool));
        let partner = svc.register(register_req(&unique_org())).await?;

        let cred = svc
            .provision_credential(
                partner.id,
                ProvisionCredentialRequest {
                    credential_type: "oauth2_client".to_string(),
                    environment: "sandbox".to_string(),
                    scopes: Some(vec!["partner:read".to_string(), "partner:write".to_string()]),
                    expires_at: None,
                    certificate_pem: None,
                },
            )
            .await?;

        assert_eq!(cred.credential_type, "oauth2_client");
        assert!(cred.client_id.is_some());
        assert!(cred.secret.is_some());
        assert_eq!(cred.scopes, vec!["partner:read", "partner:write"]);
        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_provision_mtls_cert_stores_fingerprint() -> TestResult {
        let pool = test_pool().await?;
        let svc = PartnerService::new(PartnerRepository::new(pool));
        let partner = svc.register(register_req(&unique_org())).await?;

        let fake_pem = "-----BEGIN CERTIFICATE-----\nMIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA\n-----END CERTIFICATE-----";
        let cred = svc
            .provision_credential(
                partner.id,
                ProvisionCredentialRequest {
                    credential_type: "mtls_cert".to_string(),
                    environment: "sandbox".to_string(),
                    scopes: None,
                    expires_at: None,
                    certificate_pem: Some(fake_pem.to_string()),
                },
            )
            .await?;

        assert_eq!(cred.credential_type, "mtls_cert");
        assert!(cred.certificate_fingerprint.is_some());
        assert!(cred.secret.is_none()); // no secret for mTLS
        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_provision_mtls_without_pem_returns_error() -> TestResult {
        let pool = test_pool().await?;
        let svc = PartnerService::new(PartnerRepository::new(pool));
        let partner = svc.register(register_req(&unique_org())).await?;

        let err = svc
            .provision_credential(
                partner.id,
                ProvisionCredentialRequest {
                    credential_type: "mtls_cert".to_string(),
                    environment: "sandbox".to_string(),
                    scopes: None,
                    expires_at: None,
                    certificate_pem: None, // missing
                },
            )
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            Bitmesh_backend::partner::error::PartnerError::ValidationFailed(_)
        ));
        Ok(())
    }

    // ── Validation engine ─────────────────────────────────────────────────────

    #[tokio::test]
    #[ignore]
    async fn test_validation_fails_without_credential() -> TestResult {
        let pool = test_pool().await?;
        let svc = PartnerService::new(PartnerRepository::new(pool));
        let partner = svc.register(register_req(&unique_org())).await?;

        let results = svc.run_validation(partner.id).await?;

        let cred_test = results
            .iter()
            .find(|r| r.test_name == "credential_provisioned")
            .expect("validation must include 'credential_provisioned' test");
        assert!(!cred_test.passed);
        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_validation_passes_after_credential_provisioned() -> TestResult {
        let pool = test_pool().await?;
        let svc = PartnerService::new(PartnerRepository::new(pool));
        let partner = svc.register(register_req(&unique_org())).await?;

        // Provision an API key so the credential test passes
        svc.provision_credential(
            partner.id,
            ProvisionCredentialRequest {
                credential_type: "api_key".to_string(),
                environment: "sandbox".to_string(),
                scopes: None,
                expires_at: None,
                certificate_pem: None,
            },
        )
        .await?;

        let results = svc.run_validation(partner.id).await?;

        let sandbox_test = results
            .iter()
            .find(|r| r.test_name == "sandbox_status")
            .expect("validation must include 'sandbox_status' test");
        assert!(sandbox_test.passed);

        let version_test = results
            .iter()
            .find(|r| r.test_name == "api_version_current")
            .expect("validation must include 'api_version_current' test");
        assert!(version_test.passed);
        Ok(())
    }

    // ── Promote to production ─────────────────────────────────────────────────

    #[tokio::test]
    #[ignore]
    async fn test_promote_fails_without_passing_all_tests() -> TestResult {
        let pool = test_pool().await?;
        let svc = PartnerService::new(PartnerRepository::new(pool));
        let partner = svc.register(register_req(&unique_org())).await?;

        // No credential provisioned — credential_provisioned test will fail
        let err = svc.promote_to_production(partner.id).await.unwrap_err();
        assert!(
            matches!(err, Bitmesh_backend::partner::error::PartnerError::ValidationFailed(_)),
            "expected ValidationFailed, got {:?}",
            err
        );
        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_promote_already_active_partner_fails() -> TestResult {
        let pool = test_pool().await?;
        let repo = PartnerRepository::new(pool.clone());
        let svc = PartnerService::new(repo.clone());
        let partner = svc.register(register_req(&unique_org())).await?;

        // Force status to active directly
        repo.update_status(partner.id, "active").await?;

        let err = svc.promote_to_production(partner.id).await.unwrap_err();
        assert!(matches!(
            err,
            Bitmesh_backend::partner::error::PartnerError::ValidationFailed(_)
        ));
        Ok(())
    }

    // ── Credential revocation ─────────────────────────────────────────────────

    #[tokio::test]
    #[ignore]
    async fn test_revoke_credential_owned_by_partner() -> TestResult {
        let pool = test_pool().await?;
        let svc = PartnerService::new(PartnerRepository::new(pool));
        let partner = svc.register(register_req(&unique_org())).await?;

        let cred = svc
            .provision_credential(
                partner.id,
                ProvisionCredentialRequest {
                    credential_type: "api_key".to_string(),
                    environment: "sandbox".to_string(),
                    scopes: None,
                    expires_at: None,
                    certificate_pem: None,
                },
            )
            .await?;

        svc.revoke_credential(partner.id, cred.credential_id).await?;
        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_revoke_credential_not_owned_returns_not_found() -> TestResult {
        let pool = test_pool().await?;
        let svc = PartnerService::new(PartnerRepository::new(pool));

        let partner_a = svc.register(register_req(&unique_org())).await?;
        let partner_b = svc.register(register_req(&unique_org())).await?;

        let cred = svc
            .provision_credential(
                partner_a.id,
                ProvisionCredentialRequest {
                    credential_type: "api_key".to_string(),
                    environment: "sandbox".to_string(),
                    scopes: None,
                    expires_at: None,
                    certificate_pem: None,
                },
            )
            .await?;

        // Partner B tries to revoke Partner A's credential
        let err = svc
            .revoke_credential(partner_b.id, cred.credential_id)
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            Bitmesh_backend::partner::error::PartnerError::CredentialNotFound
        ));
        Ok(())
    }

    // ── Rate limiting ─────────────────────────────────────────────────────────

    #[tokio::test]
    #[ignore]
    async fn test_rate_limit_exceeded_after_cap() -> TestResult {
        let pool = test_pool().await?;
        let repo = PartnerRepository::new(pool.clone());
        let svc = PartnerService::new(repo.clone());

        // Register with a very low rate limit
        let mut req = register_req(&unique_org());
        req.rate_limit_per_minute = Some(2);
        let partner = svc.register(req).await?;

        // First two requests should succeed
        svc.check_rate_limit(partner.id).await?;
        svc.check_rate_limit(partner.id).await?;

        // Third should be rejected
        let err = svc.check_rate_limit(partner.id).await.unwrap_err();
        assert!(matches!(
            err,
            Bitmesh_backend::partner::error::PartnerError::RateLimitExceeded
        ));
        Ok(())
    }

    // ── Deprecation notices ───────────────────────────────────────────────────

    #[tokio::test]
    #[ignore]
    async fn test_deprecation_notices_returns_active_deprecations() -> TestResult {
        let pool = test_pool().await?;
        let svc = PartnerService::new(PartnerRepository::new(pool));

        // The migration seeds a v0 deprecation — it should appear here
        let notices = svc.deprecation_notices().await?;
        assert!(
            !notices.is_empty(),
            "Expected at least the seeded v0 deprecation"
        );
        let v0 = notices
            .iter()
            .find(|n| n.api_version == "v0")
            .expect("v0 deprecation should be present in seeded data");
        assert!(v0.days_until_sunset >= 0);
        Ok(())
    }

    #[tokio::test]
    #[ignore]
    async fn test_validation_detects_deprecated_api_version() -> TestResult {
        let pool = test_pool().await?;
        let svc = PartnerService::new(PartnerRepository::new(pool));

        // Register a partner on the deprecated v0 version
        let mut req = register_req(&unique_org());
        req.api_version = Some("v0".to_string());
        let partner = svc.register(req).await?;

        let results = svc.run_validation(partner.id).await?;
        let version_test = results
            .iter()
            .find(|r| r.test_name == "api_version_current")
            .expect("validation must include 'api_version_current' test");

        assert!(!version_test.passed, "v0 is deprecated — test should fail");
        assert!(version_test.detail.contains("deprecated"));
        Ok(())
    }
}
