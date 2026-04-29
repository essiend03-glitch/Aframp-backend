//! Merchant loyalty and cashback rewards engine.
//!
//! The pure rule helpers in this module are intentionally separate from the
//! repository methods so campaign math, budget caps, tiering, and risk behavior
//! can be tested without a database or Stellar network.

use crate::database::error::DatabaseError;
use crate::merchant_gateway::models::MerchantPaymentIntent;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use std::collections::HashSet;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "text")]
#[serde(rename_all = "snake_case")]
pub enum LoyaltyCampaignStatus {
    #[sqlx(rename = "draft")]
    Draft,
    #[sqlx(rename = "active")]
    Active,
    #[sqlx(rename = "paused")]
    Paused,
    #[sqlx(rename = "deactivated")]
    Deactivated,
    #[sqlx(rename = "exhausted")]
    Exhausted,
}

impl LoyaltyCampaignStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Deactivated => "deactivated",
            Self::Exhausted => "exhausted",
        }
    }
}

impl std::fmt::Display for LoyaltyCampaignStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "text")]
#[serde(rename_all = "snake_case")]
pub enum LoyaltyRewardStatus {
    #[sqlx(rename = "queued")]
    Queued,
    #[sqlx(rename = "submitted")]
    Submitted,
    #[sqlx(rename = "paid")]
    Paid,
    #[sqlx(rename = "held")]
    Held,
    #[sqlx(rename = "failed")]
    Failed,
}

impl LoyaltyRewardStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Submitted => "submitted",
            Self::Paid => "paid",
            Self::Held => "held",
            Self::Failed => "failed",
        }
    }
}

impl std::fmt::Display for LoyaltyRewardStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "text")]
#[serde(rename_all = "snake_case")]
pub enum LoyaltyRiskStatus {
    #[sqlx(rename = "clear")]
    Clear,
    #[sqlx(rename = "flagged")]
    Flagged,
    #[sqlx(rename = "high_risk")]
    HighRisk,
}

impl LoyaltyRiskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Clear => "clear",
            Self::Flagged => "flagged",
            Self::HighRisk => "high_risk",
        }
    }

    pub fn should_hold_reward(&self) -> bool {
        matches!(self, Self::Flagged | Self::HighRisk)
    }
}

impl std::fmt::Display for LoyaltyRiskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LoyaltyRewardTier {
    Standard,
    Vip,
}

impl LoyaltyRewardTier {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Vip => "vip",
        }
    }
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct LoyaltyCampaign {
    pub id: Uuid,
    pub merchant_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub status: LoyaltyCampaignStatus,
    pub trigger_min_amount_cngn: Decimal,
    pub cashback_percent: Decimal,
    pub budget_cap_cngn: Decimal,
    pub budget_spent_cngn: Decimal,
    pub per_customer_daily_cap_cngn: Option<Decimal>,
    pub segment_tags: Vec<String>,
    pub vip_cashback_multiplier: Decimal,
    pub stellar_source_account: Option<String>,
    pub atomic_stellar_enabled: bool,
    pub starts_at: DateTime<Utc>,
    pub ends_at: Option<DateTime<Utc>>,
    pub metadata: serde_json::Value,
    pub activated_at: Option<DateTime<Utc>>,
    pub deactivated_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl LoyaltyCampaign {
    pub fn budget_remaining(&self) -> Decimal {
        (self.budget_cap_cngn - self.budget_spent_cngn).max(Decimal::ZERO)
    }
}

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
pub struct LoyaltyReward {
    pub id: Uuid,
    pub campaign_id: Uuid,
    pub merchant_id: Uuid,
    pub payment_intent_id: Uuid,
    pub customer_address: String,
    pub transaction_amount_cngn: Decimal,
    pub reward_amount_cngn: Decimal,
    pub cashback_percent: Decimal,
    pub customer_tier: String,
    pub risk_status: LoyaltyRiskStatus,
    pub risk_flags: Vec<String>,
    pub status: LoyaltyRewardStatus,
    pub stellar_tx_hash: Option<String>,
    pub stellar_source_account: Option<String>,
    pub idempotency_key: String,
    pub atomicity_mode: String,
    pub notification_status: String,
    pub failure_code: Option<String>,
    pub paid_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateLoyaltyCampaignRequest {
    pub name: String,
    pub description: Option<String>,
    pub trigger_min_amount_cngn: Decimal,
    pub cashback_percent: Decimal,
    pub budget_cap_cngn: Decimal,
    pub per_customer_daily_cap_cngn: Option<Decimal>,
    #[serde(default)]
    pub segment_tags: Vec<String>,
    pub vip_cashback_multiplier: Option<Decimal>,
    pub stellar_source_account: Option<String>,
    pub atomic_stellar_enabled: Option<bool>,
    pub starts_at: Option<DateTime<Utc>>,
    pub ends_at: Option<DateTime<Utc>>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize)]
pub struct LoyaltyRewardExecution {
    pub campaign_id: Uuid,
    pub reward_id: Uuid,
    pub reward_amount_cngn: Decimal,
    pub status: LoyaltyRewardStatus,
    pub risk_status: LoyaltyRiskStatus,
    pub atomicity_mode: String,
    pub notification_status: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoyaltySpendReportQuery {
    pub start_at: Option<DateTime<Utc>>,
    pub end_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct LoyaltyMarketingSpendReport {
    pub merchant_id: Uuid,
    pub reward_count: i64,
    pub total_reward_amount_cngn: Decimal,
    pub paid_reward_amount_cngn: Decimal,
    pub held_reward_count: i64,
    pub risk_flagged_count: i64,
    pub first_reward_at: Option<DateTime<Utc>>,
    pub last_reward_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct LoyaltyCampaignSpendSummary {
    pub campaign_id: Uuid,
    pub campaign_name: String,
    pub reward_count: i64,
    pub total_reward_amount_cngn: Decimal,
    pub paid_reward_amount_cngn: Decimal,
    pub budget_cap_cngn: Decimal,
    pub budget_spent_cngn: Decimal,
}

#[derive(Debug, Clone, Serialize)]
pub struct LoyaltyMarketingSpendResponse {
    pub summary: LoyaltyMarketingSpendReport,
    pub campaigns: Vec<LoyaltyCampaignSpendSummary>,
}

#[derive(Debug, Clone)]
pub struct LoyaltyRuleConfig {
    pub trigger_min_amount_cngn: Decimal,
    pub cashback_percent: Decimal,
    pub budget_remaining_cngn: Decimal,
    pub per_customer_daily_remaining_cngn: Option<Decimal>,
    pub segment_tags: Vec<String>,
    pub vip_cashback_multiplier: Decimal,
}

impl From<&LoyaltyCampaign> for LoyaltyRuleConfig {
    fn from(campaign: &LoyaltyCampaign) -> Self {
        Self {
            trigger_min_amount_cngn: campaign.trigger_min_amount_cngn,
            cashback_percent: campaign.cashback_percent,
            budget_remaining_cngn: campaign.budget_remaining(),
            per_customer_daily_remaining_cngn: campaign.per_customer_daily_cap_cngn,
            segment_tags: campaign.segment_tags.clone(),
            vip_cashback_multiplier: campaign.vip_cashback_multiplier,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LoyaltyEvaluationInput {
    pub transaction_amount_cngn: Decimal,
    pub customer_tags: Vec<String>,
    pub risk: LoyaltyRiskAssessment,
}

#[derive(Debug, Clone)]
pub struct LoyaltyRiskAssessment {
    pub status: LoyaltyRiskStatus,
    pub flags: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct LoyaltyRewardDecision {
    pub eligible: bool,
    pub reason: Option<String>,
    pub reward_amount_cngn: Decimal,
    pub effective_cashback_percent: Decimal,
    pub customer_tier: LoyaltyRewardTier,
    pub risk_status: LoyaltyRiskStatus,
    pub risk_flags: Vec<String>,
}

impl LoyaltyRewardDecision {
    pub fn not_eligible(reason: impl Into<String>) -> Self {
        Self {
            eligible: false,
            reason: Some(reason.into()),
            reward_amount_cngn: Decimal::ZERO,
            effective_cashback_percent: Decimal::ZERO,
            customer_tier: LoyaltyRewardTier::Standard,
            risk_status: LoyaltyRiskStatus::Clear,
            risk_flags: Vec::new(),
        }
    }
}

pub fn validate_campaign_request(req: &CreateLoyaltyCampaignRequest) -> Result<(), String> {
    if req.name.trim().is_empty() {
        return Err("Campaign name is required".to_string());
    }
    if req.trigger_min_amount_cngn <= Decimal::ZERO {
        return Err("Trigger amount must be positive".to_string());
    }
    if req.cashback_percent <= Decimal::ZERO || req.cashback_percent > Decimal::new(100, 0) {
        return Err("Cashback percent must be between 0 and 100".to_string());
    }
    if req.budget_cap_cngn <= Decimal::ZERO {
        return Err("Budget cap must be positive".to_string());
    }
    if let Some(cap) = req.per_customer_daily_cap_cngn {
        if cap <= Decimal::ZERO {
            return Err("Per-customer daily cap must be positive".to_string());
        }
    }
    if let Some(multiplier) = req.vip_cashback_multiplier {
        if multiplier < Decimal::ONE || multiplier > Decimal::new(5, 0) {
            return Err("VIP multiplier must be between 1 and 5".to_string());
        }
    }
    if matches!((req.starts_at, req.ends_at), (Some(start), Some(end)) if end <= start) {
        return Err("Campaign end time must be after start time".to_string());
    }
    Ok(())
}

pub fn calculate_cashback_amount(
    transaction_amount_cngn: Decimal,
    cashback_percent: Decimal,
) -> Decimal {
    ((transaction_amount_cngn * cashback_percent) / Decimal::new(100, 0)).round_dp(7)
}

pub fn derive_customer_tier(customer_tags: &[String]) -> LoyaltyRewardTier {
    if customer_tags
        .iter()
        .any(|tag| matches!(tag.to_ascii_lowercase().as_str(), "vip" | "loyalty_vip"))
    {
        LoyaltyRewardTier::Vip
    } else {
        LoyaltyRewardTier::Standard
    }
}

pub fn segment_matches(campaign_segment_tags: &[String], customer_tags: &[String]) -> bool {
    if campaign_segment_tags.is_empty() {
        return true;
    }

    let normalized_customer_tags: HashSet<String> = customer_tags
        .iter()
        .map(|tag| tag.trim().to_ascii_lowercase())
        .filter(|tag| !tag.is_empty())
        .collect();

    campaign_segment_tags
        .iter()
        .any(|tag| normalized_customer_tags.contains(&tag.trim().to_ascii_lowercase()))
}

pub fn assess_loyalty_risk(
    customer_address: &str,
    merchant_stellar_address: &str,
    known_high_risk_wallet: bool,
    reward_count_last_hour: i64,
    reward_count_today: i64,
) -> LoyaltyRiskAssessment {
    let mut flags = Vec::new();

    if known_high_risk_wallet {
        flags.push("known_high_risk_wallet".to_string());
    }
    if !merchant_stellar_address.is_empty() && customer_address == merchant_stellar_address {
        flags.push("self_rewarding_wallet".to_string());
    }
    if reward_count_last_hour >= 5 {
        flags.push("rapid_repeat_rewards".to_string());
    }
    if reward_count_today >= 20 {
        flags.push("daily_reward_velocity".to_string());
    }

    let status = if flags
        .iter()
        .any(|flag| flag == "known_high_risk_wallet" || flag == "self_rewarding_wallet")
    {
        LoyaltyRiskStatus::HighRisk
    } else if flags.is_empty() {
        LoyaltyRiskStatus::Clear
    } else {
        LoyaltyRiskStatus::Flagged
    };

    LoyaltyRiskAssessment { status, flags }
}

pub fn evaluate_cashback_reward(
    config: &LoyaltyRuleConfig,
    input: &LoyaltyEvaluationInput,
) -> LoyaltyRewardDecision {
    if input.transaction_amount_cngn < config.trigger_min_amount_cngn {
        return LoyaltyRewardDecision::not_eligible("transaction_below_threshold");
    }

    if !segment_matches(&config.segment_tags, &input.customer_tags) {
        return LoyaltyRewardDecision::not_eligible("customer_segment_not_matched");
    }

    let customer_tier = derive_customer_tier(&input.customer_tags);
    let effective_cashback_percent = match customer_tier {
        LoyaltyRewardTier::Standard => config.cashback_percent,
        LoyaltyRewardTier::Vip => config.cashback_percent * config.vip_cashback_multiplier,
    };
    let reward_amount =
        calculate_cashback_amount(input.transaction_amount_cngn, effective_cashback_percent);

    if reward_amount <= Decimal::ZERO {
        return LoyaltyRewardDecision::not_eligible("reward_amount_zero");
    }
    if reward_amount > config.budget_remaining_cngn {
        return LoyaltyRewardDecision::not_eligible("campaign_budget_cap_exhausted");
    }
    if matches!(config.per_customer_daily_remaining_cngn, Some(remaining) if reward_amount > remaining)
    {
        return LoyaltyRewardDecision::not_eligible("customer_daily_reward_cap_exhausted");
    }

    LoyaltyRewardDecision {
        eligible: true,
        reason: None,
        reward_amount_cngn: reward_amount,
        effective_cashback_percent,
        customer_tier,
        risk_status: input.risk.status.clone(),
        risk_flags: input.risk.flags.clone(),
    }
}

pub fn customer_tags_from_metadata(metadata: &serde_json::Value) -> Vec<String> {
    let mut tags = Vec::new();

    if let Some(values) = metadata
        .get("customer_tags")
        .and_then(|value| value.as_array())
    {
        tags.extend(
            values
                .iter()
                .filter_map(|value| value.as_str())
                .map(|value| value.to_string()),
        );
    }

    for key in ["customer_segment", "customer_tier"] {
        if let Some(value) = metadata.get(key).and_then(|value| value.as_str()) {
            tags.push(value.to_string());
        }
    }

    tags.sort();
    tags.dedup();
    tags
}

pub struct LoyaltyRepository {
    pool: PgPool,
}

impl LoyaltyRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create_campaign(
        &self,
        merchant_id: Uuid,
        req: CreateLoyaltyCampaignRequest,
    ) -> Result<LoyaltyCampaign, DatabaseError> {
        sqlx::query_as::<_, LoyaltyCampaign>(
            r#"
            INSERT INTO merchant_loyalty_campaigns (
                merchant_id, name, description, trigger_min_amount_cngn,
                cashback_percent, budget_cap_cngn, per_customer_daily_cap_cngn,
                segment_tags, vip_cashback_multiplier, stellar_source_account,
                atomic_stellar_enabled, starts_at, ends_at, metadata
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, COALESCE($12, NOW()), $13, $14)
            RETURNING *
            "#,
        )
        .bind(merchant_id)
        .bind(req.name.trim())
        .bind(req.description)
        .bind(req.trigger_min_amount_cngn)
        .bind(req.cashback_percent)
        .bind(req.budget_cap_cngn)
        .bind(req.per_customer_daily_cap_cngn)
        .bind(req.segment_tags)
        .bind(req.vip_cashback_multiplier.unwrap_or(Decimal::ONE))
        .bind(req.stellar_source_account)
        .bind(req.atomic_stellar_enabled.unwrap_or(true))
        .bind(req.starts_at)
        .bind(req.ends_at)
        .bind(req.metadata.unwrap_or_else(|| serde_json::json!({})))
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    pub async fn list_campaigns(
        &self,
        merchant_id: Uuid,
    ) -> Result<Vec<LoyaltyCampaign>, DatabaseError> {
        sqlx::query_as::<_, LoyaltyCampaign>(
            r#"
            SELECT *
            FROM merchant_loyalty_campaigns
            WHERE merchant_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(merchant_id)
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    pub async fn set_campaign_status(
        &self,
        merchant_id: Uuid,
        campaign_id: Uuid,
        status: LoyaltyCampaignStatus,
    ) -> Result<LoyaltyCampaign, DatabaseError> {
        sqlx::query_as::<_, LoyaltyCampaign>(
            r#"
            UPDATE merchant_loyalty_campaigns
            SET status = $3,
                activated_at = CASE WHEN $3 = 'active' THEN NOW() ELSE activated_at END,
                deactivated_at = CASE WHEN $3 = 'deactivated' THEN NOW() ELSE deactivated_at END
            WHERE id = $1 AND merchant_id = $2
            RETURNING *
            "#,
        )
        .bind(campaign_id)
        .bind(merchant_id)
        .bind(status.as_str())
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    pub async fn active_campaigns_for_payment(
        &self,
        merchant_id: Uuid,
        amount_cngn: Decimal,
    ) -> Result<Vec<LoyaltyCampaign>, DatabaseError> {
        sqlx::query_as::<_, LoyaltyCampaign>(
            r#"
            SELECT *
            FROM merchant_loyalty_campaigns
            WHERE merchant_id = $1
              AND status = 'active'
              AND trigger_min_amount_cngn <= $2
              AND starts_at <= NOW()
              AND (ends_at IS NULL OR ends_at > NOW())
              AND budget_spent_cngn < budget_cap_cngn
            ORDER BY created_at ASC
            "#,
        )
        .bind(merchant_id)
        .bind(amount_cngn)
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    pub async fn customer_tags_for_wallet(
        &self,
        merchant_id: Uuid,
        wallet_address: &str,
    ) -> Result<Vec<String>, DatabaseError> {
        let row: Option<(Vec<String>,)> = sqlx::query_as(
            r#"
            SELECT tags
            FROM merchant_customer_profiles
            WHERE merchant_id = $1 AND wallet_address = $2
            "#,
        )
        .bind(merchant_id)
        .bind(wallet_address)
        .fetch_optional(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;

        Ok(row.map(|(tags,)| tags).unwrap_or_default())
    }

    pub async fn high_risk_wallet_active(
        &self,
        wallet_address: &str,
    ) -> Result<bool, DatabaseError> {
        let active: (bool,) = sqlx::query_as(
            r#"
            SELECT EXISTS (
                SELECT 1
                FROM merchant_loyalty_risk_wallets
                WHERE wallet_address = $1 AND is_active = true
            )
            "#,
        )
        .bind(wallet_address)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;

        Ok(active.0)
    }

    pub async fn reward_count_since(
        &self,
        merchant_id: Uuid,
        wallet_address: &str,
        since: DateTime<Utc>,
    ) -> Result<i64, DatabaseError> {
        let count: (i64,) = sqlx::query_as(
            r#"
            SELECT COUNT(*)
            FROM merchant_loyalty_rewards
            WHERE merchant_id = $1
              AND customer_address = $2
              AND created_at >= $3
            "#,
        )
        .bind(merchant_id)
        .bind(wallet_address)
        .bind(since)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;

        Ok(count.0)
    }

    pub async fn reward_amount_since(
        &self,
        merchant_id: Uuid,
        wallet_address: &str,
        campaign_id: Uuid,
        since: DateTime<Utc>,
    ) -> Result<Decimal, DatabaseError> {
        let total: (Decimal,) = sqlx::query_as(
            r#"
            SELECT COALESCE(SUM(reward_amount_cngn), 0)
            FROM merchant_loyalty_rewards
            WHERE merchant_id = $1
              AND customer_address = $2
              AND campaign_id = $3
              AND created_at >= $4
              AND status IN ('queued', 'submitted', 'paid', 'held')
            "#,
        )
        .bind(merchant_id)
        .bind(wallet_address)
        .bind(campaign_id)
        .bind(since)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;

        Ok(total.0)
    }

    pub async fn reserve_reward(
        &self,
        campaign: &LoyaltyCampaign,
        payment_intent: &MerchantPaymentIntent,
        decision: &LoyaltyRewardDecision,
        customer_address: &str,
        stellar_source_account: Option<&str>,
    ) -> Result<Option<LoyaltyReward>, DatabaseError> {
        let mut tx = self.pool.begin().await.map_err(DatabaseError::from_sqlx)?;

        let existing = sqlx::query_as::<_, LoyaltyReward>(
            r#"
            SELECT *
            FROM merchant_loyalty_rewards
            WHERE campaign_id = $1 AND payment_intent_id = $2
            "#,
        )
        .bind(campaign.id)
        .bind(payment_intent.id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(DatabaseError::from_sqlx)?;

        if existing.is_some() {
            tx.commit().await.map_err(DatabaseError::from_sqlx)?;
            return Ok(existing);
        }

        let updated_campaign = sqlx::query_as::<_, LoyaltyCampaign>(
            r#"
            UPDATE merchant_loyalty_campaigns
            SET budget_spent_cngn = budget_spent_cngn + $2,
                status = CASE
                    WHEN budget_spent_cngn + $2 >= budget_cap_cngn THEN 'exhausted'
                    ELSE status
                END
            WHERE id = $1
              AND status = 'active'
              AND budget_spent_cngn + $2 <= budget_cap_cngn
            RETURNING *
            "#,
        )
        .bind(campaign.id)
        .bind(decision.reward_amount_cngn)
        .fetch_optional(&mut *tx)
        .await
        .map_err(DatabaseError::from_sqlx)?;

        if updated_campaign.is_none() {
            tx.commit().await.map_err(DatabaseError::from_sqlx)?;
            return Ok(None);
        }

        let reward_status = if decision.risk_status.should_hold_reward() {
            LoyaltyRewardStatus::Held
        } else {
            LoyaltyRewardStatus::Queued
        };
        let idempotency_key = format!("loyalty:{}:{}", campaign.id, payment_intent.id);
        let atomicity_mode = if campaign.atomic_stellar_enabled && stellar_source_account.is_some()
        {
            "stellar_payment_channel"
        } else {
            "post_receipt_queue"
        };

        let reward = sqlx::query_as::<_, LoyaltyReward>(
            r#"
            INSERT INTO merchant_loyalty_rewards (
                campaign_id, merchant_id, payment_intent_id, customer_address,
                transaction_amount_cngn, reward_amount_cngn, cashback_percent,
                customer_tier, risk_status, risk_flags, status, stellar_source_account,
                idempotency_key, atomicity_mode, notification_status
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, 'queued')
            RETURNING *
            "#,
        )
        .bind(campaign.id)
        .bind(payment_intent.merchant_id)
        .bind(payment_intent.id)
        .bind(customer_address)
        .bind(payment_intent.amount_cngn)
        .bind(decision.reward_amount_cngn)
        .bind(decision.effective_cashback_percent)
        .bind(decision.customer_tier.as_str())
        .bind(decision.risk_status.as_str())
        .bind(&decision.risk_flags)
        .bind(reward_status.as_str())
        .bind(stellar_source_account)
        .bind(idempotency_key)
        .bind(atomicity_mode)
        .fetch_one(&mut *tx)
        .await
        .map_err(DatabaseError::from_sqlx)?;

        tx.commit().await.map_err(DatabaseError::from_sqlx)?;
        Ok(Some(reward))
    }

    pub async fn mark_reward_submitted(
        &self,
        reward_id: Uuid,
        stellar_tx_hash: &str,
    ) -> Result<LoyaltyReward, DatabaseError> {
        sqlx::query_as::<_, LoyaltyReward>(
            r#"
            UPDATE merchant_loyalty_rewards
            SET status = 'paid',
                stellar_tx_hash = $2,
                paid_at = NOW()
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(reward_id)
        .bind(stellar_tx_hash)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    pub async fn mark_reward_failed(
        &self,
        reward_id: Uuid,
        failure_code: &str,
    ) -> Result<LoyaltyReward, DatabaseError> {
        sqlx::query_as::<_, LoyaltyReward>(
            r#"
            UPDATE merchant_loyalty_rewards
            SET status = 'failed',
                failure_code = $2
            WHERE id = $1
            RETURNING *
            "#,
        )
        .bind(reward_id)
        .bind(failure_code)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)
    }

    pub async fn queue_reward_notification(
        &self,
        reward: &LoyaltyReward,
        event_type: &str,
    ) -> Result<(), DatabaseError> {
        sqlx::query(
            r#"
            INSERT INTO merchant_loyalty_reward_notifications (
                reward_id, merchant_id, customer_address, event_type, payload, status
            )
            VALUES ($1, $2, $3, $4, $5, 'queued')
            ON CONFLICT (reward_id, event_type) DO NOTHING
            "#,
        )
        .bind(reward.id)
        .bind(reward.merchant_id)
        .bind(&reward.customer_address)
        .bind(event_type)
        .bind(serde_json::json!({
            "reward_id": reward.id,
            "campaign_id": reward.campaign_id,
            "amount_cngn": reward.reward_amount_cngn,
            "status": reward.status,
        }))
        .execute(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;

        Ok(())
    }

    pub async fn spend_report(
        &self,
        merchant_id: Uuid,
        query: LoyaltySpendReportQuery,
    ) -> Result<LoyaltyMarketingSpendResponse, DatabaseError> {
        let summary = sqlx::query_as::<_, LoyaltyMarketingSpendReport>(
            r#"
            SELECT
                $1::uuid AS merchant_id,
                COUNT(r.id)::bigint AS reward_count,
                COALESCE(SUM(r.reward_amount_cngn), 0) AS total_reward_amount_cngn,
                COALESCE(SUM(r.reward_amount_cngn) FILTER (WHERE r.status = 'paid'), 0) AS paid_reward_amount_cngn,
                (COUNT(r.id) FILTER (WHERE r.status = 'held'))::bigint AS held_reward_count,
                (COUNT(r.id) FILTER (WHERE r.risk_status <> 'clear'))::bigint AS risk_flagged_count,
                MIN(r.created_at) AS first_reward_at,
                MAX(r.created_at) AS last_reward_at
            FROM merchant_loyalty_rewards r
            WHERE r.merchant_id = $1
              AND ($2::timestamptz IS NULL OR r.created_at >= $2)
              AND ($3::timestamptz IS NULL OR r.created_at <= $3)
            "#,
        )
        .bind(merchant_id)
        .bind(query.start_at)
        .bind(query.end_at)
        .fetch_one(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;

        let campaigns = sqlx::query_as::<_, LoyaltyCampaignSpendSummary>(
            r#"
            SELECT
                c.id AS campaign_id,
                c.name AS campaign_name,
                COUNT(r.id)::bigint AS reward_count,
                COALESCE(SUM(r.reward_amount_cngn), 0) AS total_reward_amount_cngn,
                COALESCE(SUM(r.reward_amount_cngn) FILTER (WHERE r.status = 'paid'), 0) AS paid_reward_amount_cngn,
                c.budget_cap_cngn,
                c.budget_spent_cngn
            FROM merchant_loyalty_campaigns c
            LEFT JOIN merchant_loyalty_rewards r
              ON r.campaign_id = c.id
             AND ($2::timestamptz IS NULL OR r.created_at >= $2)
             AND ($3::timestamptz IS NULL OR r.created_at <= $3)
            WHERE c.merchant_id = $1
            GROUP BY c.id, c.name, c.budget_cap_cngn, c.budget_spent_cngn
            ORDER BY c.created_at DESC
            "#,
        )
        .bind(merchant_id)
        .bind(query.start_at)
        .bind(query.end_at)
        .fetch_all(&self.pool)
        .await
        .map_err(DatabaseError::from_sqlx)?;

        Ok(LoyaltyMarketingSpendResponse { summary, campaigns })
    }
}
