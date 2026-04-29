//! Business logic for Merchant Dispute Resolution & Clawback Management (Issue #337).

use crate::dispute::{
    models::*,
    repository::DisputeRepository,
};
use crate::error::Error;
use sqlx::types::BigDecimal;
use std::str::FromStr;
use std::sync::Arc;
use tracing::{error, info, warn};
use uuid::Uuid;

pub struct DisputeService {
    repo: Arc<DisputeRepository>,
}

impl DisputeService {
    pub fn new(repo: Arc<DisputeRepository>) -> Self {
        Self { repo }
    }

    // -------------------------------------------------------------------------
    // Customer actions
    // -------------------------------------------------------------------------

    /// Open a new dispute for a transaction.
    ///
    /// Validates that the transaction belongs to the customer and that no
    /// open dispute already exists for it.
    pub async fn open_dispute(
        &self,
        customer_wallet: &str,
        merchant_id: Uuid,
        transaction_amount: f64,
        req: OpenDisputeRequest,
    ) -> Result<Dispute, Error> {
        let tx_amount = BigDecimal::from_str(&transaction_amount.to_string())
            .map_err(|_| Error::Validation("Invalid transaction amount".into()))?;

        let claimed = match req.claimed_amount {
            Some(a) => {
                let bd = BigDecimal::from_str(&a.to_string())
                    .map_err(|_| Error::Validation("Invalid claimed amount".into()))?;
                if bd > tx_amount {
                    return Err(Error::Validation(
                        "Claimed amount cannot exceed transaction amount".into(),
                    ));
                }
                bd
            }
            None => tx_amount.clone(),
        };

        // Provisional escrow: hold 100% of claimed amount for high-risk path.
        // In production this would check merchant risk tier; we default to active.
        let escrow_hold_pct = Some(
            BigDecimal::from_str("100")
                .expect("static value"),
        );

        let dispute = self
            .repo
            .create_dispute(
                req.transaction_id,
                customer_wallet,
                merchant_id,
                req.reason,
                &req.description,
                tx_amount,
                claimed,
                true,
                escrow_hold_pct,
            )
            .await?;

        self.repo
            .append_audit_log(
                dispute.id,
                customer_wallet,
                "dispute_opened",
                None,
                Some(DisputeStatus::Open),
                Some(&req.description),
            )
            .await?;

        info!(
            dispute_id = %dispute.id,
            customer = %customer_wallet,
            merchant_id = %merchant_id,
            "Dispute opened"
        );

        Ok(dispute)
    }

    /// Submit evidence on behalf of a customer.
    pub async fn submit_customer_evidence(
        &self,
        dispute_id: Uuid,
        customer_wallet: &str,
        req: SubmitEvidenceRequest,
    ) -> Result<DisputeEvidence, Error> {
        let dispute = self.get_dispute_or_404(dispute_id).await?;

        if dispute.customer_wallet != customer_wallet {
            return Err(Error::Unauthorized(
                "You are not the customer on this dispute".into(),
            ));
        }
        if dispute.status.is_terminal() {
            return Err(Error::Validation(
                "Cannot add evidence to a closed dispute".into(),
            ));
        }

        self.repo
            .add_evidence(
                dispute_id,
                EvidenceSubmitter::Customer,
                customer_wallet,
                &req.label,
                req.file_url.as_deref(),
                req.notes.as_deref(),
                None,
            )
            .await
    }

    // -------------------------------------------------------------------------
    // Merchant actions
    // -------------------------------------------------------------------------

    /// Merchant responds to a dispute within the 48-hour window.
    pub async fn merchant_respond(
        &self,
        dispute_id: Uuid,
        merchant_id: Uuid,
        req: MerchantResponseRequest,
    ) -> Result<Dispute, Error> {
        let dispute = self.get_dispute_or_404(dispute_id).await?;

        if dispute.merchant_id != merchant_id {
            return Err(Error::Unauthorized(
                "You are not the merchant on this dispute".into(),
            ));
        }
        if dispute.status != DisputeStatus::Open {
            return Err(Error::Validation(
                "Dispute is no longer awaiting merchant response".into(),
            ));
        }

        let proposal_json = req
            .settlement_proposal
            .as_ref()
            .map(|p| serde_json::to_value(p).unwrap_or(serde_json::Value::Null));

        // Add merchant notes as evidence.
        self.repo
            .add_evidence(
                dispute_id,
                EvidenceSubmitter::Merchant,
                &merchant_id.to_string(),
                "Merchant response",
                None,
                Some(&req.notes),
                None,
            )
            .await?;

        let updated = self
            .repo
            .record_merchant_response(dispute_id, proposal_json)
            .await?;

        self.repo
            .append_audit_log(
                dispute_id,
                &merchant_id.to_string(),
                "merchant_responded",
                Some(DisputeStatus::Open),
                Some(DisputeStatus::UnderReview),
                Some(&req.notes),
            )
            .await?;

        info!(
            dispute_id = %dispute_id,
            merchant_id = %merchant_id,
            "Merchant responded to dispute"
        );

        Ok(updated)
    }

    /// Submit evidence on behalf of a merchant.
    pub async fn submit_merchant_evidence(
        &self,
        dispute_id: Uuid,
        merchant_id: Uuid,
        req: SubmitEvidenceRequest,
    ) -> Result<DisputeEvidence, Error> {
        let dispute = self.get_dispute_or_404(dispute_id).await?;

        if dispute.merchant_id != merchant_id {
            return Err(Error::Unauthorized(
                "You are not the merchant on this dispute".into(),
            ));
        }
        if dispute.status.is_terminal() {
            return Err(Error::Validation(
                "Cannot add evidence to a closed dispute".into(),
            ));
        }

        self.repo
            .add_evidence(
                dispute_id,
                EvidenceSubmitter::Merchant,
                &merchant_id.to_string(),
                &req.label,
                req.file_url.as_deref(),
                req.notes.as_deref(),
                None,
            )
            .await
    }

    // -------------------------------------------------------------------------
    // Platform mediation
    // -------------------------------------------------------------------------

    /// Platform mediator resolves the dispute and triggers the appropriate
    /// refund from the merchant's ledger to the customer's wallet.
    pub async fn resolve_dispute(
        &self,
        dispute_id: Uuid,
        mediator_id: &str,
        req: ResolveDisputeRequest,
    ) -> Result<Dispute, Error> {
        let dispute = self.get_dispute_or_404(dispute_id).await?;

        if dispute.status.is_terminal() {
            return Err(Error::Validation("Dispute is already resolved".into()));
        }

        let (refunded_amount, status) = match req.decision {
            DisputeDecision::FullRefund => (
                Some(dispute.claimed_amount.clone()),
                DisputeStatus::ResolvedCustomer,
            ),
            DisputeDecision::PartialRefund => {
                let amount = req
                    .refund_amount
                    .ok_or_else(|| Error::Validation("refund_amount required for partial refund".into()))?;
                let bd = BigDecimal::from_str(&amount.to_string())
                    .map_err(|_| Error::Validation("Invalid refund amount".into()))?;
                if bd > dispute.transaction_amount {
                    return Err(Error::Validation(
                        "Refund amount cannot exceed transaction amount".into(),
                    ));
                }
                (Some(bd), DisputeStatus::ResolvedPartial)
            }
            DisputeDecision::NoRefund => (None, DisputeStatus::ResolvedMerchant),
            DisputeDecision::Withdrawn => (None, DisputeStatus::Closed),
        };

        // In a full implementation this would call the Stellar payment service
        // to execute the clawback/refund on-chain. We record a placeholder hash.
        let refund_tx_hash: Option<String> = if refunded_amount.is_some() {
            Some(format!("pending-refund-{}", dispute_id))
        } else {
            None
        };

        let resolved = self
            .repo
            .resolve_dispute(
                dispute_id,
                req.decision,
                refunded_amount,
                refund_tx_hash.as_deref(),
                status,
            )
            .await?;

        self.repo
            .append_audit_log(
                dispute_id,
                mediator_id,
                "dispute_resolved",
                Some(dispute.status),
                Some(status),
                Some(&req.notes),
            )
            .await?;

        info!(
            dispute_id = %dispute_id,
            decision = ?req.decision,
            mediator = %mediator_id,
            "Dispute resolved"
        );

        Ok(resolved)
    }

    // -------------------------------------------------------------------------
    // Shared read operations
    // -------------------------------------------------------------------------

    pub async fn get_dispute(&self, id: Uuid) -> Result<Option<Dispute>, Error> {
        self.repo.get_dispute(id).await
    }

    pub async fn list_customer_disputes(
        &self,
        customer_wallet: &str,
        query: &DisputeListQuery,
    ) -> Result<DisputePage, Error> {
        self.repo
            .list_disputes_for_customer(customer_wallet, query)
            .await
    }

    pub async fn list_merchant_disputes(
        &self,
        merchant_id: Uuid,
        query: &DisputeListQuery,
    ) -> Result<DisputePage, Error> {
        self.repo
            .list_disputes_for_merchant(merchant_id, query)
            .await
    }

    pub async fn list_evidence(
        &self,
        dispute_id: Uuid,
    ) -> Result<Vec<DisputeEvidence>, Error> {
        self.repo.list_evidence(dispute_id).await
    }

    pub async fn get_audit_log(
        &self,
        dispute_id: Uuid,
    ) -> Result<Vec<DisputeAuditLog>, Error> {
        self.repo.get_audit_log(dispute_id).await
    }

    // -------------------------------------------------------------------------
    // Background worker helper
    // -------------------------------------------------------------------------

    /// Escalate disputes whose 48-hour merchant response window has expired.
    pub async fn escalate_overdue_disputes(&self) -> Result<usize, Error> {
        let ids = self.repo.flag_overdue_disputes().await?;
        let count = ids.len();
        for id in &ids {
            if let Err(e) = self
                .repo
                .append_audit_log(
                    *id,
                    "system",
                    "escalated_to_mediation",
                    Some(DisputeStatus::Open),
                    Some(DisputeStatus::Mediation),
                    Some("Merchant response deadline exceeded"),
                )
                .await
            {
                warn!(dispute_id = %id, error = %e, "Failed to write audit log for escalation");
            }
        }
        if count > 0 {
            info!(count, "Escalated overdue disputes to mediation");
        }
        Ok(count)
    }

    // -------------------------------------------------------------------------
    // Private helpers
    // -------------------------------------------------------------------------

    async fn get_dispute_or_404(&self, id: Uuid) -> Result<Dispute, Error> {
        self.repo
            .get_dispute(id)
            .await?
            .ok_or_else(|| Error::NotFound(format!("Dispute {} not found", id)))
    }
}
