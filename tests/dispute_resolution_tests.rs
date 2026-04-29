//! Unit tests for Merchant Dispute Resolution & Clawback Management (Issue #337).

#[cfg(test)]
mod tests {
    use Bitmesh_backend::dispute::models::*;

    // -------------------------------------------------------------------------
    // DisputeStatus helpers
    // -------------------------------------------------------------------------

    #[test]
    fn terminal_statuses_are_correctly_identified() {
        assert!(DisputeStatus::ResolvedCustomer.is_terminal());
        assert!(DisputeStatus::ResolvedMerchant.is_terminal());
        assert!(DisputeStatus::ResolvedPartial.is_terminal());
        assert!(DisputeStatus::Closed.is_terminal());

        assert!(!DisputeStatus::Open.is_terminal());
        assert!(!DisputeStatus::UnderReview.is_terminal());
        assert!(!DisputeStatus::Mediation.is_terminal());
    }

    #[test]
    fn dispute_status_as_str_matches_db_enum() {
        assert_eq!(DisputeStatus::Open.as_str(), "open");
        assert_eq!(DisputeStatus::UnderReview.as_str(), "under_review");
        assert_eq!(DisputeStatus::Mediation.as_str(), "mediation");
        assert_eq!(DisputeStatus::ResolvedCustomer.as_str(), "resolved_customer");
        assert_eq!(DisputeStatus::ResolvedMerchant.as_str(), "resolved_merchant");
        assert_eq!(DisputeStatus::ResolvedPartial.as_str(), "resolved_partial");
        assert_eq!(DisputeStatus::Closed.as_str(), "closed");
    }

    // -------------------------------------------------------------------------
    // DisputeListQuery pagination
    // -------------------------------------------------------------------------

    #[test]
    fn dispute_list_query_defaults_to_page_1_size_20() {
        let q = DisputeListQuery {
            status: None,
            page: None,
            page_size: None,
        };
        assert_eq!(q.page(), 1);
        assert_eq!(q.page_size(), 20);
        assert_eq!(q.offset(), 0);
    }

    #[test]
    fn dispute_list_query_clamps_page_size() {
        let q = DisputeListQuery {
            status: None,
            page: Some(1),
            page_size: Some(9999),
        };
        assert_eq!(q.page_size(), 100);
    }

    #[test]
    fn dispute_list_query_page_cannot_be_zero() {
        let q = DisputeListQuery {
            status: None,
            page: Some(0),
            page_size: Some(10),
        };
        assert_eq!(q.page(), 1);
        assert_eq!(q.offset(), 0);
    }

    #[test]
    fn dispute_list_query_offset_is_correct() {
        let q = DisputeListQuery {
            status: None,
            page: Some(3),
            page_size: Some(10),
        };
        assert_eq!(q.offset(), 20);
    }

    // -------------------------------------------------------------------------
    // SettlementProposal serialisation round-trip
    // -------------------------------------------------------------------------

    #[test]
    fn settlement_proposal_serialises_and_deserialises() {
        let proposal = SettlementProposal {
            proposal_type: "partial_refund".to_string(),
            refund_amount: Some(500.0),
            message: Some("We offer 50% refund".to_string()),
        };

        let json = serde_json::to_string(&proposal).expect("serialise");
        let back: SettlementProposal = serde_json::from_str(&json).expect("deserialise");

        assert_eq!(back.proposal_type, "partial_refund");
        assert_eq!(back.refund_amount, Some(500.0));
        assert_eq!(back.message.as_deref(), Some("We offer 50% refund"));
    }

    // -------------------------------------------------------------------------
    // DisputeDecision variants
    // -------------------------------------------------------------------------

    #[test]
    fn dispute_decision_variants_are_distinct() {
        let decisions = [
            DisputeDecision::FullRefund,
            DisputeDecision::PartialRefund,
            DisputeDecision::NoRefund,
            DisputeDecision::Withdrawn,
        ];
        // Ensure all four variants exist and are distinct.
        assert_eq!(decisions.len(), 4);
        assert_ne!(decisions[0], decisions[1]);
        assert_ne!(decisions[1], decisions[2]);
        assert_ne!(decisions[2], decisions[3]);
    }
}
