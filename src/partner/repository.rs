use sqlx::PgPool;
use uuid::Uuid;

use super::error::PartnerError;
use super::models::{
    ApiVersionDeprecation, Partner, PartnerApiCredential, PartnerCredential, PartnerEntity,
    PartnerProfile,
};

#[derive(Clone)]
pub struct PartnerRepository {
    pool: PgPool,
}

impl PartnerRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, p: &Partner) -> Result<Partner, PartnerError> {
        let row = sqlx::query_as!(
            Partner,
            r#"INSERT INTO integration_partners
               (id, name, organisation, partner_type, status, contact_email,
                ip_whitelist, rate_limit_per_minute, api_version, created_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
               RETURNING *"#,
            p.id,
            p.name,
            p.organisation,
            p.partner_type,
            p.status,
            p.contact_email,
            &p.ip_whitelist,
            p.rate_limit_per_minute,
            p.api_version,
            p.created_at,
            p.updated_at
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn find_by_id(&self, id: Uuid) -> Result<Partner, PartnerError> {
        sqlx::query_as!(
            Partner,
            "SELECT * FROM integration_partners WHERE id = $1",
            id
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or(PartnerError::NotFound)
    }

    pub async fn find_by_organisation(&self, org: &str) -> Result<Option<Partner>, PartnerError> {
        Ok(sqlx::query_as!(
            Partner,
            "SELECT * FROM integration_partners WHERE organisation = $1",
            org
        )
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn update_status(&self, id: Uuid, status: &str) -> Result<(), PartnerError> {
        sqlx::query!(
            "UPDATE integration_partners SET status = $1, updated_at = now() WHERE id = $2",
            status,
            id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // ── Credentials ───────────────────────────────────────────────────────────

    pub async fn create_credential(
        &self,
        c: &PartnerCredential,
    ) -> Result<PartnerCredential, PartnerError> {
        let row = sqlx::query_as!(
            PartnerCredential,
            r#"INSERT INTO partner_credentials
               (id, partner_id, credential_type, client_id, client_secret_hash,
                certificate_fingerprint, api_key_hash, api_key_prefix, scopes,
                environment, expires_at, revoked_at, created_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)
               RETURNING *"#,
            c.id,
            c.partner_id,
            c.credential_type,
            c.client_id,
            c.client_secret_hash,
            c.certificate_fingerprint,
            c.api_key_hash,
            c.api_key_prefix,
            &c.scopes,
            c.environment,
            c.expires_at,
            c.revoked_at,
            c.created_at
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn find_credential_by_id(&self, id: Uuid) -> Result<PartnerCredential, PartnerError> {
        sqlx::query_as!(
            PartnerCredential,
            "SELECT * FROM partner_credentials WHERE id = $1",
            id
        )
        .fetch_optional(&self.pool)
        .await?
        .ok_or(PartnerError::CredentialNotFound)
    }

    /// Returns true if the partner has at least one non-revoked credential.
    pub async fn has_active_credential(&self, partner_id: Uuid) -> Result<bool, PartnerError> {
        let row = sqlx::query!(
            "SELECT EXISTS(SELECT 1 FROM partner_credentials WHERE partner_id = $1 AND revoked_at IS NULL) as exists",
            partner_id
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(row.exists.unwrap_or(false))
    }

    pub async fn find_credential_by_client_id(
        &self,
        client_id: &str,
    ) -> Result<Option<PartnerCredential>, PartnerError> {
        Ok(sqlx::query_as!(
            PartnerCredential,
            "SELECT * FROM partner_credentials WHERE client_id = $1 AND revoked_at IS NULL",
            client_id
        )
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn find_credential_by_api_key_prefix(
        &self,
        prefix: &str,
    ) -> Result<Option<PartnerCredential>, PartnerError> {
        Ok(sqlx::query_as!(
            PartnerCredential,
            "SELECT * FROM partner_credentials WHERE api_key_prefix = $1 AND revoked_at IS NULL",
            prefix
        )
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn find_credential_by_cert_fingerprint(
        &self,
        fp: &str,
    ) -> Result<Option<PartnerCredential>, PartnerError> {
        Ok(sqlx::query_as!(
            PartnerCredential,
            "SELECT * FROM partner_credentials WHERE certificate_fingerprint = $1 AND revoked_at IS NULL",
            fp
        )
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn revoke_credential(&self, id: Uuid) -> Result<(), PartnerError> {
        sqlx::query!(
            "UPDATE partner_credentials SET revoked_at = now() WHERE id = $1",
            id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // ── Rate limit tracking ───────────────────────────────────────────────────

    /// Increment the per-partner per-minute counter; returns current count.
    pub async fn increment_rate_counter(&self, partner_id: Uuid) -> Result<i64, PartnerError> {
        let row = sqlx::query!(
            r#"INSERT INTO partner_rate_counters (partner_id, window_start, request_count)
               VALUES ($1, date_trunc('minute', now()), 1)
               ON CONFLICT (partner_id, window_start)
               DO UPDATE SET request_count = partner_rate_counters.request_count + 1
               RETURNING request_count"#,
            partner_id
        )
        .fetch_one(&self.pool)
        .await?;
        Ok(row.request_count)
    }

    // ── Deprecation notices ───────────────────────────────────────────────────

    pub async fn active_deprecations(&self) -> Result<Vec<ApiVersionDeprecation>, PartnerError> {
        Ok(sqlx::query_as!(
            ApiVersionDeprecation,
            "SELECT * FROM api_version_deprecations WHERE sunset_at > now() ORDER BY sunset_at"
        )
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn deprecation_for_version(
        &self,
        version: &str,
    ) -> Result<Option<ApiVersionDeprecation>, PartnerError> {
        Ok(sqlx::query_as!(
            ApiVersionDeprecation,
            "SELECT * FROM api_version_deprecations WHERE api_version = $1",
            version
        )
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn mark_deprecation_notified(&self, id: Uuid) -> Result<(), PartnerError> {
        sqlx::query!(
            "UPDATE api_version_deprecations SET notified_at = now() WHERE id = $1",
            id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Deprecations whose sunset is within `days_ahead` days and have not yet
    /// been notified (notified_at IS NULL).
    pub async fn deprecations_due_for_notification(
        &self,
        days_ahead: i64,
    ) -> Result<Vec<ApiVersionDeprecation>, sqlx::Error> {
        sqlx::query_as!(
            ApiVersionDeprecation,
            r#"SELECT * FROM api_version_deprecations
               WHERE notified_at IS NULL
                 AND sunset_at <= now() + make_interval(days => $1::int)
               ORDER BY sunset_at"#,
            days_ahead as i32
        )
        .fetch_all(&self.pool)
        .await
    }

    /// All active (non-suspended, non-deprecated) partners using a given API version.
    pub async fn find_partners_by_api_version(
        &self,
        version: &str,
    ) -> Result<Vec<Partner>, sqlx::Error> {
        sqlx::query_as!(
            Partner,
            "SELECT * FROM integration_partners WHERE api_version = $1 AND status NOT IN ('suspended', 'deprecated')",
            version
        )
        .fetch_all(&self.pool)
        .await
    }
}

// ── Issue #466: partners / partner_profiles / partner_api_credentials ─────────

impl PartnerRepository {
    // ── partners ──────────────────────────────────────────────────────────────

    pub async fn create_partner_entity(
        &self,
        p: &PartnerEntity,
    ) -> Result<PartnerEntity, PartnerError> {
        Ok(sqlx::query_as!(
            PartnerEntity,
            r#"INSERT INTO partners
               (id, legal_name, trading_name, organisation_type, registration_number,
                jurisdiction, onboarding_state, compliance_tier, tenant_id, created_at, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
               RETURNING *"#,
            p.id,
            p.legal_name,
            p.trading_name,
            p.organisation_type,
            p.registration_number,
            p.jurisdiction,
            p.onboarding_state,
            p.compliance_tier,
            p.tenant_id,
            p.created_at,
            p.updated_at
        )
        .fetch_one(&self.pool)
        .await?)
    }

    pub async fn find_partner_entity_by_id(
        &self,
        id: Uuid,
    ) -> Result<PartnerEntity, PartnerError> {
        sqlx::query_as!(PartnerEntity, "SELECT * FROM partners WHERE id = $1", id)
            .fetch_optional(&self.pool)
            .await?
            .ok_or(PartnerError::NotFound)
    }

    pub async fn update_onboarding_state(
        &self,
        id: Uuid,
        state: &str,
    ) -> Result<(), PartnerError> {
        sqlx::query!(
            "UPDATE partners SET onboarding_state = $1, updated_at = now() WHERE id = $2",
            state,
            id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_partners_by_state(
        &self,
        state: &str,
    ) -> Result<Vec<PartnerEntity>, PartnerError> {
        Ok(sqlx::query_as!(
            PartnerEntity,
            "SELECT * FROM partners WHERE onboarding_state = $1 ORDER BY created_at DESC",
            state
        )
        .fetch_all(&self.pool)
        .await?)
    }

    // ── partner_profiles ──────────────────────────────────────────────────────

    pub async fn upsert_partner_profile(
        &self,
        p: &PartnerProfile,
    ) -> Result<PartnerProfile, PartnerError> {
        Ok(sqlx::query_as!(
            PartnerProfile,
            r#"INSERT INTO partner_profiles
               (partner_id, primary_contact_name, primary_contact_email, primary_contact_phone,
                technical_contact_email, compliance_contact_email, website_url, support_url,
                logo_url, regulatory_licence_ref, regulated_by, notes, updated_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,now())
               ON CONFLICT (partner_id) DO UPDATE SET
                   primary_contact_name     = EXCLUDED.primary_contact_name,
                   primary_contact_email    = EXCLUDED.primary_contact_email,
                   primary_contact_phone    = EXCLUDED.primary_contact_phone,
                   technical_contact_email  = EXCLUDED.technical_contact_email,
                   compliance_contact_email = EXCLUDED.compliance_contact_email,
                   website_url              = EXCLUDED.website_url,
                   support_url              = EXCLUDED.support_url,
                   logo_url                 = EXCLUDED.logo_url,
                   regulatory_licence_ref   = EXCLUDED.regulatory_licence_ref,
                   regulated_by             = EXCLUDED.regulated_by,
                   notes                    = EXCLUDED.notes,
                   updated_at               = now()
               RETURNING *"#,
            p.partner_id,
            p.primary_contact_name,
            p.primary_contact_email,
            p.primary_contact_phone,
            p.technical_contact_email,
            p.compliance_contact_email,
            p.website_url,
            p.support_url,
            p.logo_url,
            p.regulatory_licence_ref,
            p.regulated_by,
            p.notes
        )
        .fetch_one(&self.pool)
        .await?)
    }

    pub async fn find_partner_profile(
        &self,
        partner_id: Uuid,
    ) -> Result<Option<PartnerProfile>, PartnerError> {
        Ok(sqlx::query_as!(
            PartnerProfile,
            "SELECT * FROM partner_profiles WHERE partner_id = $1",
            partner_id
        )
        .fetch_optional(&self.pool)
        .await?)
    }

    // ── partner_api_credentials ───────────────────────────────────────────────

    pub async fn create_api_credential(
        &self,
        c: &PartnerApiCredential,
    ) -> Result<PartnerApiCredential, PartnerError> {
        Ok(sqlx::query_as!(
            PartnerApiCredential,
            r#"INSERT INTO partner_api_credentials
               (id, partner_id, api_key_hash, api_key_salt, api_key_prefix,
                public_signing_key, signing_algorithm, ip_whitelist, webhook_url,
                webhook_secret_hash, environment, expires_at, revoked_at, last_used_at, created_at)
               VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)
               RETURNING *"#,
            c.id,
            c.partner_id,
            c.api_key_hash,
            c.api_key_salt,
            c.api_key_prefix,
            c.public_signing_key,
            c.signing_algorithm,
            &c.ip_whitelist,
            c.webhook_url,
            c.webhook_secret_hash,
            c.environment,
            c.expires_at,
            c.revoked_at,
            c.last_used_at,
            c.created_at
        )
        .fetch_one(&self.pool)
        .await?)
    }

    pub async fn find_api_credential_by_prefix(
        &self,
        prefix: &str,
    ) -> Result<Option<PartnerApiCredential>, PartnerError> {
        Ok(sqlx::query_as!(
            PartnerApiCredential,
            r#"SELECT * FROM partner_api_credentials
               WHERE api_key_prefix = $1
                 AND revoked_at IS NULL
                 AND (expires_at IS NULL OR expires_at > now())"#,
            prefix
        )
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn revoke_api_credential(&self, id: Uuid) -> Result<(), PartnerError> {
        sqlx::query!(
            "UPDATE partner_api_credentials SET revoked_at = now() WHERE id = $1",
            id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn touch_api_credential_last_used(&self, id: Uuid) -> Result<(), PartnerError> {
        sqlx::query!(
            "UPDATE partner_api_credentials SET last_used_at = now() WHERE id = $1",
            id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_active_api_credentials(
        &self,
        partner_id: Uuid,
    ) -> Result<Vec<PartnerApiCredential>, PartnerError> {
        Ok(sqlx::query_as!(
            PartnerApiCredential,
            r#"SELECT * FROM partner_api_credentials
               WHERE partner_id = $1
                 AND revoked_at IS NULL
                 AND (expires_at IS NULL OR expires_at > now())
               ORDER BY created_at DESC"#,
            partner_id
        )
        .fetch_all(&self.pool)
        .await?)
    }
}
