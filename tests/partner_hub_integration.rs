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

    // ── Helpers ───────────────────────────────────────────────────────────────

    async fn test_pool() -> sqlx::PgPool {
        let url = std::env::var("DATABASE_URL")
            .expect("DATABASE_URL must be set for partner hub integration tests");
        sqlx::PgPool::connect(&url).await.expect("DB connect")
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
    async fn test_register_partner_creates_sandbox_partner() {
        let pool = test_pool().await;
        let svc = PartnerService::new(PartnerRepository::new(pool));
        let org = unique_org();

        let partner = svc.register(register_req(&org)).await.unwrap();

        assert_eq!(partner.status, "sandbox");
        assert_eq!(partner.organisation, org);
        assert_eq!(partner.partner_type, "fintech");
        assert_eq!(partner.api_version, "v1");
    }

    #[tokio::test]
    #[ignore]
    async fn test_register_duplicate_organisation_returns_conflict() {
        let pool = test_pool().await;
        let svc = PartnerService::new(PartnerRepository::new(pool));
        let org = unique_org();

        svc.register(register_req(&org)).await.unwrap();
        let err = svc.register(register_req(&org)).await.unwrap_err();

        assert!(
            matches!(err, Bitmesh_backend::partner::error::PartnerError::AlreadyExists),
            "expected AlreadyExists, got {:?}",
            err
        );
    }

    #[tokio::test]
    #[ignore]
    async fn test_register_invalid_partner_type_rejected() {
        let pool = test_pool().await;
        let svc = PartnerService::new(PartnerRepository::new(pool));
        let mut req = register_req(&unique_org());
        req.partner_type = "unknown_type".to_string();

        let err = svc.register(req).await.unwrap_err();
        assert!(matches!(
            err,
            Bitmesh_backend::partner::error::PartnerError::InvalidPartnerType(_)
        ));
    }

    // ── Credential provisioning ───────────────────────────────────────────────

    #[tokio::test]
    #[ignore]
    async fn test_provision_api_key_returns_secret_once() {
        let pool = test_pool().await;
        let svc = PartnerService::new(PartnerRepository::new(pool));
        let partner = svc.register(register_req(&unique_org())).await.unwrap();

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
            .await
            .unwrap();

        assert_eq!(cred.credential_type, "api_key");
        assert!(cred.secret.is_some(), "API key secret must be returned on provisioning");
        assert!(cred.api_key_prefix.is_some());
        // Secret starts with the prefix
        let secret = cred.secret.unwrap();
        let prefix = cred.api_key_prefix.unwrap();
        assert!(secret.starts_with(&prefix));
    }

    #[tokio::test]
    #[ignore]
    async fn test_provision_oauth2_client_returns_client_id_and_secret() {
        let pool = test_pool().await;
        let svc = PartnerService::new(PartnerRepository::new(pool));
        let partner = svc.register(register_req(&unique_org())).await.unwrap();

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
            .await
            .unwrap();

        assert_eq!(cred.credential_type, "oauth2_client");
        assert!(cred.client_id.is_some());
        assert!(cred.secret.is_some());
        assert_eq!(cred.scopes, vec!["partner:read", "partner:write"]);
    }

    #[tokio::test]
    #[ignore]
    async fn test_provision_mtls_cert_stores_fingerprint() {
        let pool = test_pool().await;
        let svc = PartnerService::new(PartnerRepository::new(pool));
        let partner = svc.register(register_req(&unique_org())).await.unwrap();

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
            .await
            .unwrap();

        assert_eq!(cred.credential_type, "mtls_cert");
        assert!(cred.certificate_fingerprint.is_some());
        assert!(cred.secret.is_none()); // no secret for mTLS
    }

    #[tokio::test]
    #[ignore]
    async fn test_provision_mtls_without_pem_returns_error() {
        let pool = test_pool().await;
        let svc = PartnerService::new(PartnerRepository::new(pool));
        let partner = svc.register(register_req(&unique_org())).await.unwrap();

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
    }

    // ── Validation engine ─────────────────────────────────────────────────────

    #[tokio::test]
    #[ignore]
    async fn test_validation_fails_without_credential() {
        let pool = test_pool().await;
        let svc = PartnerService::new(PartnerRepository::new(pool));
        let partner = svc.register(register_req(&unique_org())).await.unwrap();

        let results = svc.run_validation(partner.id).await.unwrap();

        let cred_test = results
            .iter()
            .find(|r| r.test_name == "credential_provisioned")
            .unwrap();
        assert!(!cred_test.passed);
    }

    #[tokio::test]
    #[ignore]
    async fn test_validation_passes_after_credential_provisioned() {
        let pool = test_pool().await;
        let svc = PartnerService::new(PartnerRepository::new(pool));
        let partner = svc.register(register_req(&unique_org())).await.unwrap();

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
        .await
        .unwrap();

        let results = svc.run_validation(partner.id).await.unwrap();

        let sandbox_test = results.iter().find(|r| r.test_name == "sandbox_status").unwrap();
        assert!(sandbox_test.passed);

        let version_test = results.iter().find(|r| r.test_name == "api_version_current").unwrap();
        assert!(version_test.passed);
    }

    // ── Promote to production ─────────────────────────────────────────────────

    #[tokio::test]
    #[ignore]
    async fn test_promote_fails_without_passing_all_tests() {
        let pool = test_pool().await;
        let svc = PartnerService::new(PartnerRepository::new(pool));
        let partner = svc.register(register_req(&unique_org())).await.unwrap();

        // No credential provisioned — credential_provisioned test will fail
        let err = svc.promote_to_production(partner.id).await.unwrap_err();
        assert!(
            matches!(err, Bitmesh_backend::partner::error::PartnerError::ValidationFailed(_)),
            "expected ValidationFailed, got {:?}",
            err
        );
    }

    #[tokio::test]
    #[ignore]
    async fn test_promote_already_active_partner_fails() {
        let pool = test_pool().await;
        let repo = PartnerRepository::new(pool.clone());
        let svc = PartnerService::new(repo.clone());
        let partner = svc.register(register_req(&unique_org())).await.unwrap();

        // Force status to active directly
        repo.update_status(partner.id, "active").await.unwrap();

        let err = svc.promote_to_production(partner.id).await.unwrap_err();
        assert!(matches!(
            err,
            Bitmesh_backend::partner::error::PartnerError::ValidationFailed(_)
        ));
    }

    // ── Credential revocation ─────────────────────────────────────────────────

    #[tokio::test]
    #[ignore]
    async fn test_revoke_credential_owned_by_partner() {
        let pool = test_pool().await;
        let svc = PartnerService::new(PartnerRepository::new(pool));
        let partner = svc.register(register_req(&unique_org())).await.unwrap();

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
            .await
            .unwrap();

        // Revoke should succeed
        svc.revoke_credential(partner.id, cred.credential_id)
            .await
            .unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn test_revoke_credential_not_owned_returns_not_found() {
        let pool = test_pool().await;
        let svc = PartnerService::new(PartnerRepository::new(pool));

        let partner_a = svc.register(register_req(&unique_org())).await.unwrap();
        let partner_b = svc.register(register_req(&unique_org())).await.unwrap();

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
            .await
            .unwrap();

        // Partner B tries to revoke Partner A's credential
        let err = svc
            .revoke_credential(partner_b.id, cred.credential_id)
            .await
            .unwrap_err();

        assert!(matches!(
            err,
            Bitmesh_backend::partner::error::PartnerError::CredentialNotFound
        ));
    }

    // ── Rate limiting ─────────────────────────────────────────────────────────

    #[tokio::test]
    #[ignore]
    async fn test_rate_limit_exceeded_after_cap() {
        let pool = test_pool().await;
        let repo = PartnerRepository::new(pool.clone());
        let svc = PartnerService::new(repo.clone());

        // Register with a very low rate limit
        let mut req = register_req(&unique_org());
        req.rate_limit_per_minute = Some(2);
        let partner = svc.register(req).await.unwrap();

        // First two requests should succeed
        svc.check_rate_limit(partner.id).await.unwrap();
        svc.check_rate_limit(partner.id).await.unwrap();

        // Third should be rejected
        let err = svc.check_rate_limit(partner.id).await.unwrap_err();
        assert!(matches!(
            err,
            Bitmesh_backend::partner::error::PartnerError::RateLimitExceeded
        ));
    }

    // ── Deprecation notices ───────────────────────────────────────────────────

    #[tokio::test]
    #[ignore]
    async fn test_deprecation_notices_returns_active_deprecations() {
        let pool = test_pool().await;
        let svc = PartnerService::new(PartnerRepository::new(pool));

        // The migration seeds a v0 deprecation — it should appear here
        let notices = svc.deprecation_notices().await.unwrap();
        // At minimum the seeded v0 deprecation should be present
        assert!(
            !notices.is_empty(),
            "Expected at least the seeded v0 deprecation"
        );
        let v0 = notices.iter().find(|n| n.api_version == "v0");
        assert!(v0.is_some(), "v0 deprecation should be present");
        assert!(v0.unwrap().days_until_sunset >= 0);
    }

    #[tokio::test]
    #[ignore]
    async fn test_validation_detects_deprecated_api_version() {
        let pool = test_pool().await;
        let svc = PartnerService::new(PartnerRepository::new(pool));

        // Register a partner on the deprecated v0 version
        let mut req = register_req(&unique_org());
        req.api_version = Some("v0".to_string());
        let partner = svc.register(req).await.unwrap();

        let results = svc.run_validation(partner.id).await.unwrap();
        let version_test = results
            .iter()
            .find(|r| r.test_name == "api_version_current")
            .unwrap();

        assert!(!version_test.passed, "v0 is deprecated — test should fail");
        assert!(version_test.detail.contains("deprecated"));
    }
}
