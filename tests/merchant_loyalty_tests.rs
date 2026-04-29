use rust_decimal::Decimal;
use std::str::FromStr;
use Bitmesh_backend::merchant_gateway::loyalty::{
    assess_loyalty_risk, customer_tags_from_metadata, evaluate_cashback_reward,
    validate_campaign_request, CreateLoyaltyCampaignRequest, LoyaltyEvaluationInput,
    LoyaltyRewardTier, LoyaltyRiskAssessment, LoyaltyRiskStatus, LoyaltyRuleConfig,
};

fn dec(value: &str) -> Decimal {
    Decimal::from_str(value).unwrap()
}

fn base_rule() -> LoyaltyRuleConfig {
    LoyaltyRuleConfig {
        trigger_min_amount_cngn: dec("1000"),
        cashback_percent: dec("1"),
        budget_remaining_cngn: dec("500"),
        per_customer_daily_remaining_cngn: None,
        segment_tags: Vec::new(),
        vip_cashback_multiplier: dec("1"),
    }
}

#[test]
fn one_percent_cashback_campaign_qualifies_sale() {
    let decision = evaluate_cashback_reward(
        &base_rule(),
        &LoyaltyEvaluationInput {
            transaction_amount_cngn: dec("2500"),
            customer_tags: Vec::new(),
            risk: LoyaltyRiskAssessment {
                status: LoyaltyRiskStatus::Clear,
                flags: Vec::new(),
            },
        },
    );

    assert!(decision.eligible);
    assert_eq!(decision.reward_amount_cngn, dec("25.0000000"));
    assert_eq!(decision.effective_cashback_percent, dec("1"));
    assert_eq!(decision.customer_tier, LoyaltyRewardTier::Standard);
}

#[test]
fn campaign_budget_cap_blocks_reward_that_would_overspend() {
    let mut rule = base_rule();
    rule.budget_remaining_cngn = dec("10");

    let decision = evaluate_cashback_reward(
        &rule,
        &LoyaltyEvaluationInput {
            transaction_amount_cngn: dec("2500"),
            customer_tags: Vec::new(),
            risk: LoyaltyRiskAssessment {
                status: LoyaltyRiskStatus::Clear,
                flags: Vec::new(),
            },
        },
    );

    assert!(!decision.eligible);
    assert_eq!(
        decision.reason.as_deref(),
        Some("campaign_budget_cap_exhausted")
    );
}

#[test]
fn vip_segment_receives_tier_multiplier() {
    let mut rule = base_rule();
    rule.segment_tags = vec!["vip".to_string()];
    rule.vip_cashback_multiplier = dec("2");

    let decision = evaluate_cashback_reward(
        &rule,
        &LoyaltyEvaluationInput {
            transaction_amount_cngn: dec("3000"),
            customer_tags: vec!["VIP".to_string(), "lagos".to_string()],
            risk: LoyaltyRiskAssessment {
                status: LoyaltyRiskStatus::Clear,
                flags: Vec::new(),
            },
        },
    );

    assert!(decision.eligible);
    assert_eq!(decision.customer_tier, LoyaltyRewardTier::Vip);
    assert_eq!(decision.effective_cashback_percent, dec("2"));
    assert_eq!(decision.reward_amount_cngn, dec("60.0000000"));
}

#[test]
fn high_risk_wallet_is_flagged_and_reward_is_held() {
    let risk = assess_loyalty_risk(
        "GCUSTOMERAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        "GMERCHANTAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        true,
        0,
        0,
    );
    let decision = evaluate_cashback_reward(
        &base_rule(),
        &LoyaltyEvaluationInput {
            transaction_amount_cngn: dec("2000"),
            customer_tags: Vec::new(),
            risk,
        },
    );

    assert!(decision.eligible);
    assert_eq!(decision.risk_status, LoyaltyRiskStatus::HighRisk);
    assert!(decision
        .risk_flags
        .iter()
        .any(|flag| flag == "known_high_risk_wallet"));
    assert!(decision.risk_status.should_hold_reward());
}

#[test]
fn per_customer_daily_cap_blocks_excess_reward() {
    let mut rule = base_rule();
    rule.per_customer_daily_remaining_cngn = Some(dec("5"));

    let decision = evaluate_cashback_reward(
        &rule,
        &LoyaltyEvaluationInput {
            transaction_amount_cngn: dec("1000"),
            customer_tags: Vec::new(),
            risk: LoyaltyRiskAssessment {
                status: LoyaltyRiskStatus::Clear,
                flags: Vec::new(),
            },
        },
    );

    assert!(!decision.eligible);
    assert_eq!(
        decision.reason.as_deref(),
        Some("customer_daily_reward_cap_exhausted")
    );
}

#[test]
fn metadata_customer_tags_support_segments_and_tiers() {
    let metadata = serde_json::json!({
        "customer_tags": ["repeat", "vip"],
        "customer_segment": "weekend_buyers",
        "customer_tier": "vip"
    });

    let tags = customer_tags_from_metadata(&metadata);

    assert!(tags.contains(&"repeat".to_string()));
    assert!(tags.contains(&"vip".to_string()));
    assert!(tags.contains(&"weekend_buyers".to_string()));
}

#[test]
fn campaign_request_validation_rejects_invalid_percent() {
    let req = CreateLoyaltyCampaignRequest {
        name: "Bad cashback".to_string(),
        description: None,
        trigger_min_amount_cngn: dec("1000"),
        cashback_percent: dec("101"),
        budget_cap_cngn: dec("10000"),
        per_customer_daily_cap_cngn: None,
        segment_tags: Vec::new(),
        vip_cashback_multiplier: None,
        stellar_source_account: None,
        atomic_stellar_enabled: Some(true),
        starts_at: None,
        ends_at: None,
        metadata: None,
    };

    assert!(validate_campaign_request(&req).is_err());
}
