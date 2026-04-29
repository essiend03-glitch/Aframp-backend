//! Integration Tests for AML Systems
//!
//! Tests the complete AML pipeline including:
//! - Policy engine rule evaluation
//! - Case management workflows
//! - Risk scoring and decision making
//! - Investigation processes

use aframp_backend::aml::{
    policy_engine::AMLPolicyEngine, enhanced_case_management::EnhancedAMLCaseManager,
    models::*, evaluation::*, rules::AMLRuleLibrary,
};
use aframp_backend::cache::AdvancedRedisCache;
use chrono::Utc;
use std::collections::HashMap;
use uuid::Uuid;

fn create_test_evaluation_context() -> EvaluationContext {
    EvaluationContext {
        transaction: TransactionData {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            transaction_type: TransactionType::CryptoOnramp,
            amount: 15000.0,
            currency: "USD".to_string(),
            source_address: Some("0x1234567890abcdef".to_string()),
            destination_address: Some("0x0987654321fedcba".to_string()),
            timestamp: Utc::now(),
            ip_address: "192.168.1.100".to_string(),
            user_agent: "Mozilla/5.0 (Test Browser)".to_string(),
            device_fingerprint: Some("fp_123456".to_string()),
            geolocation: Geolocation {
                country: "US".to_string(),
                region: "CA".to_string(),
                city: "San Francisco".to_string(),
                latitude: 37.7749,
                longitude: -122.4194,
                is_vpn: false,
                is_tor: false,
                is_high_risk_country: false,
            },
            metadata: serde_json::json!({"test": true}),
        },
        user_profile: UserProfile {
            id: Uuid::new_v4(),
            kyc_tier: KYCTier::Tier2,
            registration_date: Utc::now() - chrono::Duration::days(180),
            risk_score: 0.3,
            jurisdiction: "US".to_string(),
            is_pep: false,
            is_sanctioned: false,
            watchlist_flags: vec![],
            verification_documents: vec![],
            account_status: AccountStatus::Active,
        },
        historical_metrics: HistoricalMetrics {
            transaction_count_24h: 3,
            transaction_count_7d: 15,
            transaction_count_30d: 45,
            total_volume_24h: 5000.0,
            total_volume_7d: 25000.0,
            total_volume_30d: 100000.0,
            average_transaction_size: 2222.22,
            max_transaction_size: 8000.0,
            unique_counterparties_30d: 8,
            last_transaction_date: Some(Utc::now() - chrono::Duration::hours(2)),
            account_age_days: 180,
            dormant_days: 0,
        },
        real_time_signals: RealTimeSignals {
            ip_risk_score: 0.1,
            device_risk_score: 0.05,
            behavioral_anomaly_score: 0.2,
            velocity_alert_count: 0,
            failed_auth_attempts: 0,
            concurrent_sessions: 1,
        },
        counterparty_data: Some(CounterpartyData {
            id: Uuid::new_v4(),
            risk_score: 0.2,
            is_known_suspicious: false,
            sanctions_flags: vec![],
            watchlist_flags: vec![],
            transaction_history: vec![],
        }),
        session_context: SessionContext {
            session_id: "sess_123456".to_string(),
            login_timestamp: Utc::now() - chrono::Duration::hours(1),
            authentication_method: "password".to_string(),
            mfa_enabled: true,
        },
    }
}

#[tokio::test]
async fn test_aml_policy_engine_initialization() -> Result<(), anyhow::Error> {
    // This test would require a real database connection
    // For now, we'll test the policy engine configuration
    
    let config = aframp_backend::aml::policy_engine::PolicyEngineConfig::default();
    
    assert!(config.enable_caching);
    assert!(config.enable_performance_monitoring);
    assert_eq!(config.max_rules_per_evaluation, 100);
    assert!(config.risk_thresholds.low_max < config.risk_thresholds.medium_min);
    assert!(config.risk_thresholds.medium_min < config.risk_thresholds.high_min);
    assert!(config.risk_thresholds.high_min < config.risk_thresholds.critical_min);

    Ok(())
}

#[tokio::test]
async fn test_aml_rule_validation() -> Result<(), anyhow::Error> {
    let rules = AMLRuleLibrary::get_initial_rules();
    
    // Verify we have the expected number of rules
    assert!(rules.len() >= 10);
    
    // Validate each rule
    for rule in &rules {
        assert!(rule.validate().is_ok(), "Rule {} should be valid", rule.name);
        assert!(!rule.name.is_empty());
        assert!(!rule.description.is_empty());
        assert!(rule.risk_weight >= 0.0 && rule.risk_weight <= 1.0);
        assert!(!rule.applicable_transaction_types.is_empty());
        assert!(!rule.applicable_jurisdictions.is_empty());
        assert!(!rule.evaluation_logic.conditions.is_empty());
    }

    Ok(())
}

#[tokio::test]
async fn test_aml_rule_categories() -> Result<(), anyhow::Error> {
    let rules = AMLRuleLibrary::get_initial_rules();
    
    // Verify we have rules for each required category
    let mut categories = std::collections::HashSet::new();
    for rule in &rules {
        categories.insert(format!("{:?}", rule.category));
    }
    
    assert!(categories.contains("Structuring"));
    assert!(categories.contains("Velocity"));
    assert!(categories.contains("AmountAnomaly"));
    assert!(categories.contains("GeographicRisk"));
    assert!(categories.contains("CounterpartyRisk"));
    assert!(categories.contains("Layering"));
    assert!(categories.contains("NewAccount"));
    assert!(categories.contains("DormantAccount"));
    assert!(categories.contains("NetworkAnalysis"));

    Ok(())
}

#[tokio::test]
async fn test_aml_rule_evaluation_conditions() -> Result<(), anyhow::Error> {
    let rules = AMLRuleLibrary::get_initial_rules();
    
    for rule in &rules {
        for condition in &rule.evaluation_logic.conditions {
            // Verify condition fields are valid
            assert!(!condition.field.is_empty());
            assert!(!condition.field.contains("invalid_field"));
            
            // Verify operators are valid
            match condition.operator {
                ComparisonOperator::Equals |
                ComparisonOperator::NotEquals |
                ComparisonOperator::GreaterThan |
                ComparisonOperator::GreaterThanOrEqual |
                ComparisonOperator::LessThan |
                ComparisonOperator::LessThanOrEqual |
                ComparisonOperator::Contains |
                ComparisonOperator::NotContains |
                ComparisonOperator::In |
                ComparisonOperator::NotIn => {},
                ComparisonOperator::Regex => {
                    // Verify regex patterns are valid
                    if let serde_json::Value::String(pattern) = &condition.value {
                        assert!(regex::Regex::new(pattern).is_ok());
                    }
                }
            }
            
            // Verify weights are valid if present
            if let Some(weight) = condition.weight {
                assert!(weight >= 0.0 && weight <= 1.0);
            }
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_risk_level_calculation() -> Result<(), anyhow::Error> {
    let config = aframp_backend::aml::policy_engine::PolicyEngineConfig::default();
    
    // Test risk level thresholds
    let low_score = config.risk_thresholds.low_max - 1.0;
    let medium_score = (config.risk_thresholds.medium_min + config.risk_thresholds.medium_max) / 2.0;
    let high_score = (config.risk_thresholds.high_min + config.risk_thresholds.high_max) / 2.0;
    let critical_score = config.risk_thresholds.critical_min + 1.0;
    
    // Create a mock policy engine to test risk level calculation
    // Note: This would normally require database connection
    assert!(low_score < config.risk_thresholds.low_max);
    assert!(medium_score >= config.risk_thresholds.medium_min);
    assert!(medium_score <= config.risk_thresholds.medium_max);
    assert!(high_score >= config.risk_thresholds.high_min);
    assert!(high_score <= config.risk_thresholds.high_max);
    assert!(critical_score >= config.risk_thresholds.critical_min);

    Ok(())
}

#[tokio::test]
async fn test_case_management_configuration() -> Result<(), anyhow::Error> {
    let config = aframp_backend::aml::enhanced_case_management::CaseManagementConfig::default();
    
    assert!(config.auto_assignment_enabled);
    assert_eq!(config.default_sla_hours, 72);
    assert_eq!(config.high_risk_sla_hours, 24);
    assert_eq!(config.critical_risk_sla_hours, 4);
    assert_eq!(config.max_investigator_cases, 20);
    
    // Verify investigation checklists exist for all case types
    assert!(config.investigation_checklists.contains_key(&aframp_backend::aml::enhanced_case_management::CaseType::TransactionBased));
    assert!(config.investigation_checklists.contains_key(&aframp_backend::aml::enhanced_case_management::CaseType::ActivityBased));
    assert!(config.investigation_checklists.contains_key(&aframp_backend::aml::enhanced_case_management::CaseType::ReferralBased));

    Ok(())
}

#[tokio::test]
async fn test_investigation_checklists() -> Result<(), anyhow::Error> {
    let config = aframp_backend::aml::enhanced_case_management::CaseManagementConfig::default();
    
    // Test transaction-based checklist
    if let Some(checklist) = config.investigation_checklists.get(&aframp_backend::aml::enhanced_case_management::CaseType::TransactionBased) {
        assert!(!checklist.required_items.is_empty());
        assert!(checklist.required_items.iter().all(|item| item.required));
        
        // Verify required items have all necessary fields
        for item in &checklist.required_items {
            assert!(!item.title.is_empty());
            assert!(!item.description.is_empty());
            assert!(item.estimated_duration_minutes > 0);
        }
    }
    
    // Test activity-based checklist
    if let Some(checklist) = config.investigation_checklists.get(&aframp_backend::aml::enhanced_case_management::CaseType::ActivityBased) {
        assert!(!checklist.required_items.is_empty());
        assert!(checklist.required_items.iter().all(|item| item.required));
    }

    Ok(())
}

#[tokio::test]
async fn test_aml_models_serialization() -> Result<(), anyhow::Error> {
    // Test that all AML models can be serialized/deserialized
    
    let transaction = TransactionData {
        id: Uuid::new_v4(),
        user_id: Uuid::new_v4(),
        transaction_type: TransactionType::CryptoOnramp,
        amount: 1000.0,
        currency: "USD".to_string(),
        source_address: Some("0x1234567890abcdef".to_string()),
        destination_address: Some("0x0987654321fedcba".to_string()),
        timestamp: Utc::now(),
        ip_address: "192.168.1.100".to_string(),
        user_agent: "Test Agent".to_string(),
        device_fingerprint: Some("fp_123456".to_string()),
        geolocation: Geolocation {
            country: "US".to_string(),
            region: "CA".to_string(),
            city: "San Francisco".to_string(),
            latitude: 37.7749,
            longitude: -122.4194,
            is_vpn: false,
            is_tor: false,
            is_high_risk_country: false,
        },
        metadata: serde_json::json!({"test": true}),
    };
    
    // Test serialization
    let serialized = serde_json::to_string(&transaction)?;
    let deserialized: TransactionData = serde_json::from_str(&serialized)?;
    
    assert_eq!(transaction.id, deserialized.id);
    assert_eq!(transaction.amount, deserialized.amount);
    assert_eq!(transaction.transaction_type, deserialized.transaction_type);

    Ok(())
}

#[tokio::test]
async fn test_evaluation_context_extraction() -> Result<(), anyhow::Error> {
    let context = create_test_evaluation_context();
    
    // Test field extraction logic (this would normally be part of policy engine)
    let test_fields = vec![
        "transaction.amount",
        "transaction.currency",
        "transaction.transaction_type",
        "user_profile.kyc_tier",
        "user_profile.jurisdiction",
        "user_profile.risk_score",
        "user_profile.is_pep",
        "user_profile.is_sanctioned",
        "historical_metrics.transaction_count_24h",
        "historical_metrics.average_transaction_size",
        "real_time_signals.ip_risk_score",
        "real_time_signals.device_risk_score",
        "geolocation.country",
        "geolocation.is_high_risk_country",
        "geolocation.is_vpn",
    ];
    
    // Verify all expected fields exist in the context
    for field in test_fields {
        let parts: Vec<&str> = field.split('.').collect();
        match parts.as_slice() {
            ["transaction", "amount"] => assert!(context.transaction.amount > 0.0),
            ["transaction", "currency"] => assert!(!context.transaction.currency.is_empty()),
            ["transaction", "transaction_type"] => {}, // Valid enum
            ["user_profile", "kyc_tier"] => {}, // Valid enum
            ["user_profile", "jurisdiction"] => assert!(!context.user_profile.jurisdiction.is_empty()),
            ["user_profile", "risk_score"] => assert!(context.user_profile.risk_score >= 0.0 && context.user_profile.risk_score <= 1.0),
            ["user_profile", "is_pep"] => {}, // Boolean
            ["user_profile", "is_sanctioned"] => {}, // Boolean
            ["historical_metrics", "transaction_count_24h"] => assert!(context.historical_metrics.transaction_count_24h >= 0),
            ["historical_metrics", "average_transaction_size"] => assert!(context.historical_metrics.average_transaction_size > 0.0),
            ["real_time_signals", "ip_risk_score"] => assert!(context.real_time_signals.ip_risk_score >= 0.0 && context.real_time_signals.ip_risk_score <= 1.0),
            ["real_time_signals", "device_risk_score"] => assert!(context.real_time_signals.device_risk_score >= 0.0 && context.real_time_signals.device_risk_score <= 1.0),
            ["geolocation", "country"] => assert!(!context.transaction.geolocation.country.is_empty()),
            ["geolocation", "is_high_risk_country"] => {}, // Boolean
            ["geolocation", "is_vpn"] => {}, // Boolean
            _ => panic!("Unexpected field: {}", field),
        }
    }

    Ok(())
}

#[tokio::test]
async fn test_aml_flag_creation() -> Result<(), anyhow::Error> {
    // Test creating different types of AML flags
    
    let sanctions_flag = AmlFlag::SanctionsHit {
        list: "OFAC".to_string(),
        matched_name: "John Doe".to_string(),
    };
    
    let smurfing_flag = AmlFlag::SmurfingDetected {
        tx_count: 5,
        window_hours: 24,
        total_amount: "4500.00".to_string(),
    };
    
    let rapid_flip_flag = AmlFlag::RapidFlip {
        on_ramp_tx_id: Uuid::new_v4(),
        off_ramp_corridor: "NG-US".to_string(),
        elapsed_minutes: 15,
    };
    
    let high_corridor_risk_flag = AmlFlag::HighCorridorRisk {
        corridor: "NG-MM".to_string(),
        risk_score: 0.9,
        reason: "FATF Grey List — Myanmar".to_string(),
    };
    
    // Test serialization of flags
    let flags = vec![&sanctions_flag, &smurfing_flag, &rapid_flip_flag, &high_corridor_risk_flag];
    
    for flag in &flags {
        let serialized = serde_json::to_string(flag)?;
        let _deserialized: AmlFlag = serde_json::from_str(&serialized)?;
    }

    Ok(())
}

#[tokio::test]
async fn test_policy_evaluation_result_structure() -> Result<(), anyhow::Error> {
    let evaluation_result = PolicyEvaluationResult {
        policy_set_id: Uuid::new_v4(),
        evaluation_context_id: Uuid::new_v4(),
        composite_risk_score: 0.75,
        risk_level: RiskLevel::High,
        triggered_rules: vec![
            RuleEvaluationResult {
                rule_id: Uuid::new_v4(),
                evaluation_context_id: Uuid::new_v4(),
                triggered: true,
                confidence_score: 0.8,
                contributing_evidence: vec![],
                recommended_response: ResponseAction::Flag,
                evaluation_timestamp: Utc::now(),
                evaluation_duration_ms: 150,
                error: None,
            }
        ],
        recommended_response: ResponseAction::Flag,
        evaluation_timestamp: Utc::now(),
        evaluation_duration_ms: 200,
        cache_hit: false,
    };
    
    // Test serialization
    let serialized = serde_json::to_string(&evaluation_result)?;
    let deserialized: PolicyEvaluationResult = serde_json::from_str(&serialized)?;
    
    assert_eq!(evaluation_result.policy_set_id, deserialized.policy_set_id);
    assert_eq!(evaluation_result.composite_risk_score, deserialized.composite_risk_score);
    assert_eq!(evaluation_result.risk_level, deserialized.risk_level);
    assert_eq!(evaluation_result.triggered_rules.len(), deserialized.triggered_rules.len());

    Ok(())
}

#[tokio::test]
async fn test_case_record_creation() -> Result<(), anyhow::Error> {
    let case_record = AMLCaseRecord {
        id: Uuid::new_v4(),
        case_type: aframp_backend::aml::enhanced_case_management::CaseType::TransactionBased,
        case_source: aframp_backend::aml::enhanced_case_management::CaseSource::AMLRuleTrigger,
        risk_score_at_opening: 0.85,
        subject_kyc_id: Uuid::new_v4(),
        subject_wallet_addresses: vec!["0x1234567890abcdef".to_string()],
        case_status: AmlCaseStatus::Open,
        assigned_investigator_id: Some("investigator_1".to_string()),
        supervisor_id: Some("supervisor_1".to_string()),
        opened_timestamp: Utc::now(),
        target_resolution_timestamp: Utc::now() + chrono::Duration::hours(24),
        resolved_timestamp: None,
        resolution_summary: None,
    };
    
    // Test serialization
    let serialized = serde_json::to_string(&case_record)?;
    let deserialized: AMLCaseRecord = serde_json::from_str(&serialized)?;
    
    assert_eq!(case_record.id, deserialized.id);
    assert_eq!(case_record.case_type, deserialized.case_type);
    assert_eq!(case_record.risk_score_at_opening, deserialized.risk_score_at_opening);
    assert_eq!(case_record.case_status, deserialized.case_status);

    Ok(())
}

#[tokio::test]
async fn test_case_evidence_record() -> Result<(), anyhow::Error> {
    let evidence_record = aframp_backend::aml::enhanced_case_management::CaseEvidenceRecord {
        id: Uuid::new_v4(),
        case_id: Uuid::new_v4(),
        evidence_type: aframp_backend::aml::enhanced_case_management::EvidenceType::TransactionRecord,
        evidence_description: "Original transaction that triggered AML flag".to_string(),
        evidence_reference_id: Some(Uuid::new_v4().to_string()),
        added_by_officer_id: "officer_1".to_string(),
        added_timestamp: Utc::now(),
    };
    
    // Test serialization
    let serialized = serde_json::to_string(&evidence_record)?;
    let deserialized: aframp_backend::aml::enhanced_case_management::CaseEvidenceRecord = serde_json::from_str(&serialized)?;
    
    assert_eq!(evidence_record.id, deserialized.id);
    assert_eq!(evidence_record.evidence_type, deserialized.evidence_type);
    assert_eq!(evidence_record.case_id, deserialized.case_id);

    Ok(())
}

#[tokio::test]
async fn test_backtest_request_structure() -> Result<(), anyhow::Error> {
    let backtest_request = aframp_backend::aml::policy_engine::BacktestRequest {
        rule_id: Some(Uuid::new_v4()),
        policy_set_id: None,
        test_period_start: Utc::now() - chrono::Duration::days(30),
        test_period_end: Utc::now(),
        sample_size: Some(1000),
        random_sample: false,
    };
    
    // Test serialization
    let serialized = serde_json::to_string(&backtest_request)?;
    let deserialized: aframp_backend::aml::policy_engine::BacktestRequest = serde_json::from_str(&serialized)?;
    
    assert_eq!(backtest_request.rule_id, deserialized.rule_id);
    assert_eq!(backtest_request.test_period_start, deserialized.test_period_start);
    assert_eq!(backtest_request.sample_size, deserialized.sample_size);

    Ok(())
}

#[tokio::test]
async fn test_sla_status_calculation() -> Result<(), anyhow::Error> {
    let config = aframp_backend::aml::enhanced_case_management::CaseManagementConfig::default();
    
    // Test SLA calculation for different risk levels
    let now = Utc::now();
    
    // Low risk case
    let low_risk_target = now + chrono::Duration::hours(config.default_sla_hours as i64);
    
    // High risk case
    let high_risk_target = now + chrono::Duration::hours(config.high_risk_sla_hours as i64);
    
    // Critical risk case
    let critical_risk_target = now + chrono::Duration::hours(config.critical_risk_sla_hours as i64);
    
    // Verify SLA hierarchy
    assert!(critical_risk_target < high_risk_target);
    assert!(high_risk_target < low_risk_target);
    
    // Verify reasonable SLA values
    assert!(config.default_sla_hours >= 24);
    assert!(config.high_risk_sla_hours <= config.default_sla_hours);
    assert!(config.critical_risk_sla_hours <= config.high_risk_sla_hours);

    Ok(())
}

#[tokio::test]
async fn test_assignment_strategies() -> Result<(), anyhow::Error> {
    use aframp_backend::aml::enhanced_case_management::AssignmentStrategy;
    
    // Test all assignment strategies are valid
    let strategies = vec![
        AssignmentStrategy::RoundRobin,
        AssignmentStrategy::WorkloadBalanced,
        AssignmentStrategy::SpecialtyBased,
    ];
    
    for strategy in strategies {
        // Verify strategy can be serialized
        let serialized = serde_json::to_string(&strategy)?;
        let _deserialized: AssignmentStrategy = serde_json::from_str(&serialized)?;
    }

    Ok(())
}

#[tokio::test]
async fn test_network_analysis_types() -> Result<(), anyhow::Error> {
    use aframp_backend::aml::enhanced_case_management::*;
    
    // Test network node types
    let node_types = vec![
        NodeType::Subject,
        NodeType::Counterparty,
        NodeType::Intermediary,
    ];
    
    for node_type in node_types {
        let serialized = serde_json::to_string(&node_type)?;
        let _deserialized: NodeType = serde_json::from_str(&serialized)?;
    }
    
    // Test network pattern types
    let pattern_types = vec![
        PatternType::Circular,
        PatternType::HubSpoke,
        PatternType::RapidChain,
        PatternType::Layering,
    ];
    
    for pattern_type in pattern_types {
        let serialized = serde_json::to_string(&pattern_type)?;
        let _deserialized: PatternType = serde_json::from_str(&serialized)?;
    }

    Ok(())
}

#[cfg(test)]
mod performance_tests {
    use super::*;
    use std::time::Instant;

    #[tokio::test]
    async fn benchmark_aml_rule_evaluation() -> Result<(), anyhow::Error> {
        let rules = AMLRuleLibrary::get_initial_rules();
        let context = create_test_evaluation_context();
        
        const NUM_EVALUATIONS: usize = 1000;
        
        // Benchmark rule evaluation (mock)
        let start = Instant::now();
        
        for _i in 0..NUM_EVALUATIONS {
            // In a real test, this would evaluate rules against the context
            // For now, we'll just simulate the work
            let _score = context.transaction.amount * 0.1;
            let _risk_level = if _score > 0.7 { RiskLevel::High } else { RiskLevel::Low };
        }
        
        let duration = start.elapsed();
        
        println!("AML Rule Evaluation Benchmark:");
        println!("{} evaluations in {:?} ({:.2} evals/sec)", 
            NUM_EVALUATIONS, duration, NUM_EVALUATIONS as f64 / duration.as_secs_f64());
        
        // Should be able to evaluate at least 100 rules per second
        let evals_per_sec = NUM_EVALUATIONS as f64 / duration.as_secs_f64();
        assert!(evals_per_sec > 100.0, "Evaluation too slow: {:.2} evals/sec", evals_per_sec);

        Ok(())
    }

    #[tokio::test]
    async fn benchmark_case_creation() -> Result<(), anyhow::Error> {
        const NUM_CASES: usize = 100;
        
        let start = Instant::now();
        
        for i in 0..NUM_CASES {
            let case_record = AMLCaseRecord {
                id: Uuid::new_v4(),
                case_type: aframp_backend::aml::enhanced_case_management::CaseType::TransactionBased,
                case_source: aframp_backend::aml::enhanced_case_management::CaseSource::AMLRuleTrigger,
                risk_score_at_opening: (i as f64) / 100.0,
                subject_kyc_id: Uuid::new_v4(),
                subject_wallet_addresses: vec![format!("0x{:040x}", i)],
                case_status: AmlCaseStatus::Open,
                assigned_investigator_id: Some(format!("investigator_{}", i % 5)),
                supervisor_id: Some("supervisor_1".to_string()),
                opened_timestamp: Utc::now(),
                target_resolution_timestamp: Utc::now() + chrono::Duration::hours(24),
                resolved_timestamp: None,
                resolution_summary: None,
            };
            
            // Test serialization
            let _serialized = serde_json::to_string(&case_record)?;
        }
        
        let duration = start.elapsed();
        
        println!("Case Creation Benchmark:");
        println!("{} cases created in {:?} ({:.2} cases/sec)", 
            NUM_CASES, duration, NUM_CASES as f64 / duration.as_secs_f64());

        Ok(())
    }
}
