//! Unit tests for Merchant Multi-Sig models and business logic.

#[cfg(test)]
mod tests {
    use crate::merchant_multisig::models::*;

    #[test]
    fn action_type_round_trips() {
        for (s, expected) in [
            ("payout", ActionType::Payout),
            ("api_key_update", ActionType::ApiKeyUpdate),
            ("tax_config_update", ActionType::TaxConfigUpdate),
            ("any", ActionType::Any),
            ("unknown", ActionType::Any),
        ] {
            assert_eq!(ActionType::from_str(s), expected);
            if s != "unknown" {
                assert_eq!(expected.as_str(), s);
            }
        }
    }

    #[test]
    fn proposal_status_as_str() {
        assert_eq!(ProposalStatus::Pending.as_str(), "pending");
        assert_eq!(ProposalStatus::Approved.as_str(), "approved");
        assert_eq!(ProposalStatus::Rejected.as_str(), "rejected");
        assert_eq!(ProposalStatus::Expired.as_str(), "expired");
        assert_eq!(ProposalStatus::Executed.as_str(), "executed");
    }

    #[test]
    fn signer_decision_as_str() {
        assert_eq!(SignerDecision::Approved.as_str(), "approved");
        assert_eq!(SignerDecision::Rejected.as_str(), "rejected");
    }

    #[test]
    fn freeze_error_is_account_frozen() {
        let err = MultisigError::AccountFrozen;
        assert!(err.to_string().contains("frozen"));
    }

    #[test]
    fn duplicate_signature_error_contains_signer() {
        let err = MultisigError::DuplicateSignature("user-42".to_string());
        assert!(err.to_string().contains("user-42"));
    }

    #[test]
    fn no_policy_error_contains_action_and_merchant() {
        let err = MultisigError::NoPolicyApplicable("payout".to_string(), "merchant-1".to_string());
        let msg = err.to_string();
        assert!(msg.contains("payout"));
        assert!(msg.contains("merchant-1"));
    }
}
