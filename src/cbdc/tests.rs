#[cfg(feature = "database")]
#[cfg(test)]
mod tests {
    use crate::cbdc::models::*;
    use crate::cbdc::validator::{ScreeningResult, SwapValidator};
    use crate::cbdc::two_pc::TwoPhaseLockState;

    // ── Model Tests ─────────────────────────────────────────────────────────

    #[test]
    fn test_dlt_system_as_str() {
        assert_eq!(DltSystem::HyperledgerBesu.as_str(), "Hyperledger Besu");
        assert_eq!(DltSystem::Corda.as_str(), "Corda");
        assert_eq!(DltSystem::Quorum.as_str(), "Quorum");
        assert_eq!(DltSystem::HyperledgerFabric.as_str(), "Hyperledger Fabric");
    }

    #[test]
    fn test_swap_type_as_str() {
        assert_eq!(SwapType::Mint.as_str(), "mint");
        assert_eq!(SwapType::Burn.as_str(), "burn");
        assert_eq!(SwapType::CrossRailSettlement.as_str(), "cross_rail_settlement");
    }

    #[test]
    fn test_two_phase_lock_state_as_str() {
        assert_eq!(TwoPhaseLockState::None.as_str(), "none");
        assert_eq!(TwoPhaseLockState::Preparing.as_str(), "preparing");
        assert_eq!(TwoPhaseLockState::Prepared.as_str(), "prepared");
        assert_eq!(TwoPhaseLockState::Committing.as_str(), "committing");
        assert_eq!(TwoPhaseLockState::Committed.as_str(), "committed");
        assert_eq!(TwoPhaseLockState::RollingBack.as_str(), "rolling_back");
        assert_eq!(TwoPhaseLockState::RolledBack.as_str(), "rolled_back");
    }

    // ── Validator Tests ────────────────────────────────────────────────────

    #[tokio::test]
    async fn test_swap_validator_valid_payload() {
        let validator = SwapValidator::new();
        let payload = serde_json::json!({
            "amount": 1000.0,
            "sender": "GABCDEF123456789",
            "recipient": "CBN_NIGERIA_WALLET",
            "jurisdiction": "ng",
            "compliance_metadata": {
                "purpose": "trade_settlement",
                "source_of_funds": "export_receivables",
            }
        });

        let report = validator.validate(&payload).await;
        assert!(report.is_valid);
        assert!(report.violations.is_empty());
    }

    #[tokio::test]
    async fn test_swap_validator_missing_fields() {
        let validator = SwapValidator::new();
        let payload = serde_json::json!({
            "amount": 1000.0,
        });

        let report = validator.validate(&payload).await;
        assert!(!report.is_valid);
        assert!(report.violations.iter().any(|v| v.contains("Sender")));
        assert!(report.violations.iter().any(|v| v.contains("Recipient")));
    }

    #[tokio::test]
    async fn test_swap_validator_negative_amount() {
        let validator = SwapValidator::new();
        let payload = serde_json::json!({
            "amount": -100.0,
            "sender": "GABCDEF123456789",
            "recipient": "CBN_NIGERIA_WALLET",
        });

        let report = validator.validate(&payload).await;
        assert!(!report.is_valid);
        assert!(report.violations.iter().any(|v| v.contains("positive")));
    }

    #[tokio::test]
    async fn test_swap_validator_zero_amount() {
        let validator = SwapValidator::new();
        let payload = serde_json::json!({
            "amount": 0.0,
            "sender": "GABCDEF123456789",
            "recipient": "CBN_NIGERIA_WALLET",
        });

        let report = validator.validate(&payload).await;
        assert!(!report.is_valid);
        assert!(report.violations.iter().any(|v| v.contains("positive")));
    }

    #[tokio::test]
    async fn test_swap_validator_warnings_for_missing_compliance_tags() {
        let validator = SwapValidator::new();
        let payload = serde_json::json!({
            "amount": 500.0,
            "sender": "GABCDEF123456789",
            "recipient": "CBN_NIGERIA_WALLET",
            "jurisdiction": "ng",
        });

        let report = validator.validate(&payload).await;
        assert!(report.is_valid); // Still valid with warnings
        assert!(report.warnings.iter().any(|w| w.contains("purpose")));
        assert!(report.warnings.iter().any(|w| w.contains("source_of_funds")));
    }

    #[tokio::test]
    async fn test_swap_validator_screening_result_default() {
        let validator = SwapValidator::new();
        let payload = serde_json::json!({
            "amount": 100.0,
            "sender": "GABCDEF123456789",
            "recipient": "CBN_NIGERIA_WALLET",
        });

        let report = validator.validate(&payload).await;
        assert_eq!(report.screening_result, ScreeningResult::Pending);
        assert_eq!(report.screening_id, "no-aml-service");
    }

    // ── HSM Signing Algorithm Tests ────────────────────────────────────────

    #[test]
    fn test_hsm_algorithm_serde() {
        let alg = crate::cbdc::hsm::HsmSigningAlgorithm::EcdsaP256;
        let serialized = serde_json::to_string(&alg).unwrap();
        assert_eq!(serialized, "\"ECDSA-P256\"");

        let deserialized: crate::cbdc::hsm::HsmSigningAlgorithm =
            serde_json::from_str("\"PKCS11-HSM\"").unwrap();
        assert_eq!(deserialized, crate::cbdc::hsm::HsmSigningAlgorithm::Pkcs11Hsm);
    }

    // ── Worker Config Tests ────────────────────────────────────────────────

    #[test]
    fn test_worker_config_defaults() {
        let config = CbdcWorkerConfig::default();
        assert_eq!(config.settlement_poll_interval_secs, 10);
        assert_eq!(config.settlement_batch_size, 50);
        assert_eq!(config.reversal_retry_interval_secs, 30);
        assert_eq!(config.gateway_health_interval_secs, 60);
        assert_eq!(config.two_phase_lock_ttl_secs, 300);
        assert_eq!(config.two_phase_heartbeat_interval_secs, 15);
        assert_eq!(config.max_reversal_attempts, 5);
    }

    // ── Swap Initiation Request Validation ─────────────────────────────────

    #[test]
    fn test_initiate_swap_request_serde() {
        let req = InitiateSwapRequest {
            swap_type: SwapType::Mint,
            stellar_asset_code: "cNGN".to_string(),
            stellar_asset_issuer: Some("GABCDEF123456789".to_string()),
            stellar_amount: "1000.000000000000000000".parse().unwrap(),
            stellar_destination_account: "GXYZ123456789".to_string(),
            cbdc_gateway_id: uuid::Uuid::new_v4(),
            cbdc_recipient: "CBN_RESERVE_WALLET".to_string(),
            cbdc_currency: "NGN".to_string(),
            cbdc_amount: "1000.000000000000000000".parse().unwrap(),
            idempotency_key: "test-idempotency-key-001".to_string(),
            compliance_metadata: None,
            required_approvals: Some(2),
        };

        let serialized = serde_json::to_string(&req).unwrap();
        let deserialized: InitiateSwapRequest = serde_json::from_str(&serialized).unwrap();
        assert_eq!(req.swap_type, deserialized.swap_type);
        assert_eq!(req.stellar_asset_code, deserialized.stellar_asset_code);
        assert_eq!(req.idempotency_key, deserialized.idempotency_key);
        assert_eq!(req.required_approvals, deserialized.required_approvals);
    }
}
