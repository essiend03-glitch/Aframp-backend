//! Business logic for Merchant CRM & Customer Insights (Issue #334).

use crate::error::Error;
use crate::merchant_crm::{
    encryption::{decrypt_pii, encrypt_pii, mask_wallet_address},
    models::*,
    repository::CustomerProfileRepository,
};
use std::sync::Arc;
use tracing::{error, info};
use uuid::Uuid;

pub struct MerchantCrmService {
    repo: Arc<CustomerProfileRepository>,
}

impl MerchantCrmService {
    pub fn new(repo: Arc<CustomerProfileRepository>) -> Self {
        Self { repo }
    }

    // -------------------------------------------------------------------------
    // Consent-based opt-in
    // -------------------------------------------------------------------------

    /// Upsert a customer profile with encrypted PII from the checkout opt-in flow.
    pub async fn opt_in_customer(
        &self,
        merchant_id: Uuid,
        req: UpsertCustomerProfileRequest,
    ) -> Result<CustomerProfileResponse, Error> {
        let enc_email = req
            .email
            .as_deref()
            .map(encrypt_pii)
            .transpose()
            .map_err(|e| Error::Internal(format!("Encryption error: {}", e)))?;

        let enc_phone = req
            .phone
            .as_deref()
            .map(encrypt_pii)
            .transpose()
            .map_err(|e| Error::Internal(format!("Encryption error: {}", e)))?;

        let enc_name = req
            .name
            .as_deref()
            .map(encrypt_pii)
            .transpose()
            .map_err(|e| Error::Internal(format!("Encryption error: {}", e)))?;

        let tags = req.tags.unwrap_or_default();

        let profile = self
            .repo
            .upsert_profile(
                merchant_id,
                &req.wallet_address,
                req.display_name.as_deref(),
                enc_email.as_deref(),
                enc_phone.as_deref(),
                enc_name.as_deref(),
                req.consent_given,
                req.consent_ip_address.as_deref(),
                &tags,
            )
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;

        // Refresh lifetime metrics from transactions table
        let _ = self
            .repo
            .refresh_profile_metrics(merchant_id, &req.wallet_address)
            .await;

        info!(
            merchant_id = %merchant_id,
            wallet = %req.wallet_address,
            consent = req.consent_given,
            "Customer profile upserted"
        );

        self.to_response(profile, true)
    }

    // -------------------------------------------------------------------------
    // Profile retrieval
    // -------------------------------------------------------------------------

    pub async fn get_customer(
        &self,
        merchant_id: Uuid,
        wallet_address: &str,
        decrypt: bool,
    ) -> Result<CustomerProfileResponse, Error> {
        let profile = self
            .repo
            .find_profile(merchant_id, wallet_address)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?
            .ok_or_else(|| Error::NotFound("Customer profile not found".into()))?;

        self.to_response(profile, decrypt)
    }

    pub async fn list_customers(
        &self,
        merchant_id: Uuid,
        query: &CustomerListQuery,
    ) -> Result<Vec<CustomerProfileResponse>, Error> {
        let page = query.page.unwrap_or(1).max(1);
        let page_size = query.page_size.unwrap_or(50).min(200);
        let anonymise = query
            .export_mode
            .as_deref()
            .map(|m| m == "anonymised")
            .unwrap_or(false);

        let profiles = self
            .repo
            .list_profiles(
                merchant_id,
                query.min_spent,
                query.active_within_days,
                query.tag.as_deref(),
                query.repeat_only.unwrap_or(false),
                page,
                page_size,
            )
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;

        profiles
            .into_iter()
            .map(|p| self.to_response(p, !anonymise))
            .collect()
    }

    // -------------------------------------------------------------------------
    // Tag management
    // -------------------------------------------------------------------------

    pub async fn update_tags(
        &self,
        merchant_id: Uuid,
        wallet_address: &str,
        tags: Vec<String>,
    ) -> Result<(), Error> {
        self.repo
            .update_tags(merchant_id, wallet_address, &tags)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;

        info!(
            merchant_id = %merchant_id,
            wallet = %wallet_address,
            tags = ?tags,
            "Customer tags updated"
        );
        Ok(())
    }

    // -------------------------------------------------------------------------
    // Retention metrics
    // -------------------------------------------------------------------------

    pub async fn get_retention_metrics(
        &self,
        merchant_id: Uuid,
    ) -> Result<RetentionMetrics, Error> {
        let (total, repeat) = self
            .repo
            .get_retention_metrics(merchant_id)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;

        let retention_rate_pct = if total > 0 {
            (repeat as f64 / total as f64) * 100.0
        } else {
            0.0
        };

        Ok(RetentionMetrics {
            total_customers: total,
            repeat_customers: repeat,
            retention_rate_pct,
            avg_purchase_frequency_days: None,
            avg_days_since_last_purchase: None,
        })
    }

    // -------------------------------------------------------------------------
    // Segments
    // -------------------------------------------------------------------------

    pub async fn upsert_segment(
        &self,
        merchant_id: Uuid,
        req: UpsertSegmentRequest,
    ) -> Result<CustomerSegment, Error> {
        let criteria = serde_json::to_value(&req.filter_criteria)
            .map_err(|e| Error::BadRequest(format!("Invalid filter criteria: {}", e)))?;

        self.repo
            .upsert_segment(merchant_id, &req.name, req.description.as_deref(), &criteria)
            .await
            .map_err(|e| Error::Internal(e.to_string()))
    }

    pub async fn list_segments(
        &self,
        merchant_id: Uuid,
    ) -> Result<Vec<CustomerSegment>, Error> {
        self.repo
            .list_segments(merchant_id)
            .await
            .map_err(|e| Error::Internal(e.to_string()))
    }

    // -------------------------------------------------------------------------
    // Privacy-first export
    // -------------------------------------------------------------------------

    pub async fn export_anonymised(
        &self,
        merchant_id: Uuid,
    ) -> Result<Vec<AnonymisedCustomerExport>, Error> {
        let profiles = self
            .repo
            .list_profiles(merchant_id, None, None, None, false, 1, 10_000)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;

        Ok(profiles
            .into_iter()
            .map(|p| AnonymisedCustomerExport {
                wallet_address_masked: mask_wallet_address(&p.wallet_address),
                total_spent: p.total_spent.to_string(),
                total_transactions: p.total_transactions,
                is_repeat_customer: p.is_repeat_customer,
                tags: p.tags,
                first_transaction_at: p.first_transaction_at,
                last_transaction_at: p.last_transaction_at,
            })
            .collect())
    }

    // -------------------------------------------------------------------------
    // Internal helpers
    // -------------------------------------------------------------------------

    fn to_response(
        &self,
        profile: CustomerProfile,
        decrypt: bool,
    ) -> Result<CustomerProfileResponse, Error> {
        let (email, phone, name) = if decrypt && profile.consent_given {
            let e = profile
                .encrypted_email
                .as_deref()
                .map(decrypt_pii)
                .transpose()
                .map_err(|e| Error::Internal(format!("Decryption error: {}", e)))?;
            let p = profile
                .encrypted_phone
                .as_deref()
                .map(decrypt_pii)
                .transpose()
                .map_err(|e| Error::Internal(format!("Decryption error: {}", e)))?;
            let n = profile
                .encrypted_name
                .as_deref()
                .map(decrypt_pii)
                .transpose()
                .map_err(|e| Error::Internal(format!("Decryption error: {}", e)))?;
            (e, p, n)
        } else {
            (None, None, None)
        };

        Ok(CustomerProfileResponse {
            id: profile.id,
            wallet_address: profile.wallet_address,
            display_name: profile.display_name,
            email,
            phone,
            name,
            consent_given: profile.consent_given,
            consent_given_at: profile.consent_given_at,
            total_spent: profile.total_spent.to_string(),
            total_transactions: profile.total_transactions,
            first_transaction_at: profile.first_transaction_at,
            last_transaction_at: profile.last_transaction_at,
            is_repeat_customer: profile.is_repeat_customer,
            tags: profile.tags,
            created_at: profile.created_at,
        })
    }
}
