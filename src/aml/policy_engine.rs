//! AML Policy Engine - Comprehensive Rule Management & Evaluation System
//!
//! Implements a sophisticated AML policy engine with:
//! - Dynamic rule definition and management
//! - Policy set routing based on transaction context
//! - Real-time rule evaluation with caching
//! - Performance monitoring and backtesting
//! - Event-driven rule updates and invalidation

use super::models::*;
use crate::cache::{AdvancedRedisCache, CacheError};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, error, info, warn};
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct AMLPolicyEngine {
    database: PgPool,
    cache: Arc<AdvancedRedisCache>,
    config: PolicyEngineConfig,
    rule_cache: Arc<std::sync::RwLock<HashMap<Uuid, AMLRule>>>,
    policy_set_cache: Arc<std::sync::RwLock<HashMap<Uuid, PolicySet>>>,
}

#[derive(Debug, Clone)]
pub struct PolicyEngineConfig {
    pub evaluation_timeout_ms: u64,
    pub cache_ttl_ms: u64,
    pub max_rules_per_evaluation: usize,
    pub risk_thresholds: RiskThresholds,
    pub enable_caching: bool,
    pub enable_performance_monitoring: bool,
    pub enable_backtesting: bool,
}

impl Default for PolicyEngineConfig {
    fn default() -> Self {
        Self {
            evaluation_timeout_ms: 5000,
            cache_ttl_ms: 300000, // 5 minutes
            max_rules_per_evaluation: 100,
            risk_thresholds: RiskThresholds {
                low_max: 30.0,
                medium_min: 30.0,
                medium_max: 60.0,
                high_min: 60.0,
                high_max: 80.0,
                critical_min: 80.0,
            },
            enable_caching: true,
            enable_performance_monitoring: true,
            enable_backtesting: true,
        }
    }
}

impl AMLPolicyEngine {
    pub async fn new(database_url: &str, cache: AdvancedRedisCache) -> Result<Self, anyhow::Error> {
        let database = PgPool::connect(database_url).await?;
        let config = PolicyEngineConfig::default();
        
        let engine = Self {
            database,
            cache: Arc::new(cache),
            config,
            rule_cache: Arc::new(std::sync::RwLock::new(HashMap::new())),
            policy_set_cache: Arc::new(std::sync::RwLock::new(HashMap::new())),
        };

        // Initialize rule cache
        engine.initialize_rule_cache().await?;
        
        // Initialize policy set cache
        engine.initialize_policy_set_cache().await?;

        info!("AML Policy Engine initialized successfully");
        Ok(engine)
    }

    async fn initialize_rule_cache(&self) -> Result<(), anyhow::Error> {
        let rules = self.load_all_rules().await?;
        let mut cache = self.rule_cache.write().unwrap();
        
        for rule in rules {
            cache.insert(rule.id, rule);
        }
        
        info!("Loaded {} AML rules into cache", cache.len());
        Ok(())
    }

    async fn initialize_policy_set_cache(&self) -> Result<(), anyhow::Error> {
        let policy_sets = self.load_all_policy_sets().await?;
        let mut cache = self.policy_set_cache.write().unwrap();
        
        for policy_set in policy_sets {
            cache.insert(policy_set.id, policy_set);
        }
        
        info!("Loaded {} AML policy sets into cache", cache.len());
        Ok(())
    }

    /// Evaluate a transaction against applicable AML rules
    pub async fn evaluate_transaction(&self, context: &EvaluationContext) -> Result<PolicyEvaluationResult, anyhow::Error> {
        let start_time = std::time::Instant::now();
        
        debug!("Starting AML evaluation for transaction: {}", context.transaction.id);

        // Determine applicable policy set
        let policy_set = self.get_applicable_policy_set(context).await?;
        
        // Check cache first if enabled
        if self.config.enable_caching {
            let cache_key = format!("aml:evaluation:{}:{}", 
                context.transaction.id, 
                self.calculate_context_hash(context));
            
            if let Some(cached_result) = self.cache.get::<PolicyEvaluationResult>(&cache_key).await? {
                debug!("AML evaluation cache hit for transaction: {}", context.transaction.id);
                return Ok(cached_result);
            }
        }

        // Load applicable rules
        let rules = self.load_applicable_rules(&policy_set).await?;
        
        // Evaluate each rule
        let mut triggered_rules = Vec::new();
        let mut composite_score = 0.0;

        for rule in rules.iter().take(self.config.max_rules_per_evaluation) {
            let rule_start = std::time::Instant::now();
            
            match self.evaluate_rule(rule, context).await {
                Ok(result) => {
                    if result.triggered {
                        composite_score += result.confidence_score * rule.risk_weight;
                        triggered_rules.push(result);
                    }
                }
                Err(e) => {
                    error!("Rule evaluation failed for rule {}: {}", rule.id, e);
                    // Continue with other rules even if one fails
                }
            }

            // Check evaluation timeout
            if rule_start.elapsed().as_millis() > self.config.evaluation_timeout_ms {
                warn!("AML evaluation timeout reached for transaction: {}", context.transaction.id);
                break;
            }
        }

        // Determine risk level and response action
        let risk_level = self.calculate_risk_level(composite_score);
        let response_action = self.determine_response_action(risk_level, &policy_set);

        let evaluation_result = PolicyEvaluationResult {
            policy_set_id: policy_set.id,
            evaluation_context_id: context.transaction.id,
            composite_risk_score: composite_score,
            risk_level,
            triggered_rules,
            recommended_response: response_action,
            evaluation_timestamp: Utc::now(),
            evaluation_duration_ms: start_time.elapsed().as_millis() as u64,
            cache_hit: false,
        };

        // Cache result if enabled
        if self.config.enable_caching {
            let cache_key = format!("aml:evaluation:{}:{}", 
                context.transaction.id, 
                self.calculate_context_hash(context));
            
            let ttl = std::time::Duration::from_millis(self.config.cache_ttl_ms);
            self.cache.set(&cache_key, &evaluation_result, Some(ttl)).await?;
        }

        debug!("AML evaluation completed for transaction: {} (score: {}, level: {:?})", 
            context.transaction.id, composite_score, risk_level);

        Ok(evaluation_result)
    }

    async fn get_applicable_policy_set(&self, context: &EvaluationContext) -> Result<PolicySet, anyhow::Error> {
        let cache = self.policy_set_cache.read().unwrap();
        
        for policy_set in cache.values() {
            if self.is_policy_set_applicable(policy_set, context) {
                return Ok(policy_set.clone());
            }
        }

        // Return default policy set if no specific one matches
        self.get_default_policy_set().await
    }

    fn is_policy_set_applicable(&self, policy_set: &PolicySet, context: &EvaluationContext) -> bool {
        let conditions = &policy_set.applicable_conditions;

        // Check transaction type
        if !conditions.transaction_types.is_empty() && 
           !conditions.transaction_types.contains(&context.transaction.transaction_type) {
            return false;
        }

        // Check KYC tier
        if !conditions.user_kyc_tiers.is_empty() && 
           !conditions.user_kyc_tiers.contains(&context.user_profile.kyc_tier) {
            return false;
        }

        // Check jurisdiction
        if !conditions.jurisdictions.is_empty() && 
           !conditions.jurisdictions.contains(&context.user_profile.jurisdiction) {
            return false;
        }

        // Check amount range
        if !conditions.amount_ranges.is_empty() {
            let amount_matches = conditions.amount_ranges.iter().any(|range| {
                let amount = context.transaction.amount;
                let min_matches = range.min.map_or(true, |min| amount >= min);
                let max_matches = range.max.map_or(true, |max| amount <= max);
                min_matches && max_matches && range.currency == context.transaction.currency
            });

            if !amount_matches {
                return false;
            }
        }

        true
    }

    async fn get_default_policy_set(&self) -> Result<PolicySet, anyhow::Error> {
        // Try to find a default policy set in cache first
        let cache = self.policy_set_cache.read().unwrap();
        
        for policy_set in cache.values() {
            if policy_set.name == "Default" {
                return Ok(policy_set.clone());
            }
        }

        // If no default found, create one
        self.create_default_policy_set().await
    }

    async fn create_default_policy_set(&self) -> Result<PolicySet, anyhow::Error> {
        let default_rules = self.get_default_rule_ids().await?;
        
        let policy_set = PolicySet {
            id: Uuid::new_v4(),
            name: "Default".to_string(),
            description: "Default AML policy set for all transactions".to_string(),
            rule_ids: default_rules,
            applicable_conditions: PolicyConditions {
                transaction_types: vec![], // Apply to all
                user_kyc_tiers: vec![], // Apply to all
                jurisdictions: vec![], // Apply to all
                amount_ranges: vec![], // Apply to all
            },
            status: PolicySetStatus::Active,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            created_by: Uuid::new_v4(),
            updated_by: None,
        };

        Ok(policy_set)
    }

    async fn get_default_rule_ids(&self) -> Result<Vec<Uuid>, anyhow::Error> {
        // Return IDs of core AML rules that should be in default policy
        Ok(vec![
            Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap(), // Structuring rule
            Uuid::parse_str("00000000-0000-0000-0000-000000000002").unwrap(), // Velocity rule
            Uuid::parse_str("00000000-0000-0000-0000-000000000003").unwrap(), // Amount anomaly rule
            Uuid::parse_str("00000000-0000-0000-0000-000000000004").unwrap(), // Geographic risk rule
            Uuid::parse_str("00000000-0000-0000-0000-000000000005").unwrap(), // Counterparty risk rule
        ])
    }

    async fn load_applicable_rules(&self, policy_set: &PolicySet) -> Result<Vec<AMLRule>, anyhow::Error> {
        let cache = self.rule_cache.read().unwrap();
        let mut rules = Vec::new();

        for rule_id in &policy_set.rule_ids {
            if let Some(rule) = cache.get(rule_id) {
                if rule.status == AMLRuleStatus::Active {
                    rules.push(rule.clone());
                }
            }
        }

        // Sort by priority (risk weight descending)
        rules.sort_by(|a, b| b.risk_weight.partial_cmp(&a.risk_weight).unwrap());

        Ok(rules)
    }

    async fn evaluate_rule(&self, rule: &AMLRule, context: &EvaluationContext) -> Result<RuleEvaluationResult, anyhow::Error> {
        let start_time = std::time::Instant::now();
        
        debug!("Evaluating AML rule: {} for transaction: {}", rule.id, context.transaction.id);

        // Check if rule is applicable to this transaction type
        if !rule.applicable_transaction_types.contains(&context.transaction.transaction_type) {
            return Ok(RuleEvaluationResult {
                rule_id: rule.id,
                evaluation_context_id: context.transaction.id,
                triggered: false,
                confidence_score: 0.0,
                contributing_evidence: vec![],
                recommended_response: ResponseAction::Allow,
                evaluation_timestamp: Utc::now(),
                evaluation_duration_ms: start_time.elapsed().as_millis() as u64,
                error: None,
            });
        }

        // Evaluate rule conditions
        let mut triggered = false;
        let mut confidence_score = 0.0;
        let mut evidence = Vec::new();

        for condition in &rule.evaluation_logic.conditions {
            let condition_result = self.evaluate_condition(condition, context).await?;
            
            if condition_result.satisfied {
                triggered = true;
                confidence_score += condition_result.confidence;
                evidence.push(Evidence {
                    field: condition.field.clone(),
                    value: condition_result.actual_value,
                    threshold: condition_result.threshold,
                    actual_value: condition_result.actual_value.clone(),
                    weight: condition.weight.unwrap_or(1.0),
                    description: format!("Condition met: {}", condition_result.description),
                });
            }
        }

        // Normalize confidence score
        if !rule.evaluation_logic.conditions.is_empty() {
            confidence_score /= rule.evaluation_logic.conditions.len() as f64;
        }

        let result = RuleEvaluationResult {
            rule_id: rule.id,
            evaluation_context_id: context.transaction.id,
            triggered,
            confidence_score,
            contributing_evidence: evidence,
            recommended_response: rule.response_action.clone(),
            evaluation_timestamp: Utc::now(),
            evaluation_duration_ms: start_time.elapsed().as_millis() as u64,
            error: None,
        };

        debug!("Rule evaluation completed: {} (triggered: {}, confidence: {})", 
            rule.id, triggered, confidence_score);

        Ok(result)
    }

    async fn evaluate_condition(&self, condition: &Condition, context: &EvaluationContext) -> Result<ConditionEvaluationResult, anyhow::Error> {
        let field_value = self.extract_field_value(&condition.field, context)?;
        let threshold_value = condition.value.clone();
        
        let satisfied = match &condition.operator {
            ComparisonOperator::Equals => self.compare_equals(&field_value, &threshold_value)?,
            ComparisonOperator::NotEquals => !self.compare_equals(&field_value, &threshold_value)?,
            ComparisonOperator::GreaterThan => self.compare_greater_than(&field_value, &threshold_value)?,
            ComparisonOperator::GreaterThanOrEqual => self.compare_greater_than_or_equal(&field_value, &threshold_value)?,
            ComparisonOperator::LessThan => self.compare_less_than(&field_value, &threshold_value)?,
            ComparisonOperator::LessThanOrEqual => self.compare_less_than_or_equal(&field_value, &threshold_value)?,
            ComparisonOperator::Contains => self.compare_contains(&field_value, &threshold_value)?,
            ComparisonOperator::NotContains => !self.compare_contains(&field_value, &threshold_value)?,
            ComparisonOperator::In => self.compare_in(&field_value, &threshold_value)?,
            ComparisonOperator::NotIn => !self.compare_in(&field_value, &threshold_value)?,
            ComparisonOperator::Regex => self.compare_regex(&field_value, &threshold_value)?,
        };

        let confidence = if satisfied { condition.weight.unwrap_or(1.0) } else { 0.0 };

        Ok(ConditionEvaluationResult {
            satisfied,
            confidence,
            actual_value: field_value,
            threshold: threshold_value,
            description: format!("{} {} {:?}", condition.field, condition.operator, condition.value),
        })
    }

    fn extract_field_value(&self, field_path: &str, context: &EvaluationContext) -> Result<serde_json::Value, anyhow::Error> {
        let parts: Vec<&str> = field_path.split('.').collect();
        
        match parts.as_slice() {
            ["transaction", "amount"] => Ok(serde_json::Value::Number(context.transaction.amount.into())),
            ["transaction", "currency"] => Ok(serde_json::Value::String(context.transaction.currency.clone())),
            ["transaction", "transaction_type"] => Ok(serde_json::to_value(&context.transaction.transaction_type)?),
            
            ["user_profile", "kyc_tier"] => Ok(serde_json::to_value(&context.user_profile.kyc_tier)?),
            ["user_profile", "jurisdiction"] => Ok(serde_json::Value::String(context.user_profile.jurisdiction.clone())),
            ["user_profile", "risk_score"] => Ok(serde_json::Value::Number(context.user_profile.risk_score.into())),
            ["user_profile", "is_pep"] => Ok(serde_json::Value::Bool(context.user_profile.is_pep)),
            ["user_profile", "is_sanctioned"] => Ok(serde_json::Value::Bool(context.user_profile.is_sanctioned)),
            
            ["historical_metrics", "transaction_count_24h"] => Ok(serde_json::Value::Number(context.historical_metrics.transaction_count_24h.into())),
            ["historical_metrics", "transaction_count_7d"] => Ok(serde_json::Value::Number(context.historical_metrics.transaction_count_7d.into())),
            ["historical_metrics", "transaction_count_30d"] => Ok(serde_json::Value::Number(context.historical_metrics.transaction_count_30d.into())),
            ["historical_metrics", "average_transaction_size"] => Ok(serde_json::Value::Number(context.historical_metrics.average_transaction_size.into())),
            ["historical_metrics", "max_transaction_size"] => Ok(serde_json::Value::Number(context.historical_metrics.max_transaction_size.into())),
            ["historical_metrics", "unique_counterparties_30d"] => Ok(serde_json::Value::Number(context.historical_metrics.unique_counterparties_30d.into())),
            ["historical_metrics", "account_age_days"] => Ok(serde_json::Value::Number(context.historical_metrics.account_age_days.into())),
            ["historical_metrics", "dormant_days"] => Ok(serde_json::Value::Number(context.historical_metrics.dormant_days.into())),
            
            ["real_time_signals", "ip_risk_score"] => Ok(serde_json::Value::Number(context.real_time_signals.ip_risk_score.into())),
            ["real_time_signals", "device_risk_score"] => Ok(serde_json::Value::Number(context.real_time_signals.device_risk_score.into())),
            ["real_time_signals", "behavioral_anomaly_score"] => Ok(serde_json::Value::Number(context.real_time_signals.behavioral_anomaly_score.into())),
            ["real_time_signals", "velocity_alert_count"] => Ok(serde_json::Value::Number(context.real_time_signals.velocity_alert_count.into())),
            ["real_time_signals", "failed_auth_attempts"] => Ok(serde_json::Value::Number(context.real_time_signals.failed_auth_attempts.into())),
            ["real_time_signals", "concurrent_sessions"] => Ok(serde_json::Value::Number(context.real_time_signals.concurrent_sessions.into())),
            
            ["geolocation", "country"] => Ok(serde_json::Value::String(context.transaction.geolocation.country.clone())),
            ["geolocation", "is_high_risk_country"] => Ok(serde_json::Value::Bool(context.transaction.geolocation.is_high_risk_country)),
            ["geolocation", "is_vpn"] => Ok(serde_json::Value::Bool(context.transaction.geolocation.is_vpn)),
            ["geolocation", "is_tor"] => Ok(serde_json::Value::Bool(context.transaction.geolocation.is_tor)),
            
            _ => Err(anyhow::anyhow!("Unknown field path: {}", field_path)),
        }
    }

    fn compare_equals(&self, actual: &serde_json::Value, expected: &serde_json::Value) -> Result<bool, anyhow::Error> {
        Ok(actual == expected)
    }

    fn compare_greater_than(&self, actual: &serde_json::Value, expected: &serde_json::Value) -> Result<bool, anyhow::Error> {
        match (actual, expected) {
            (serde_json::Value::Number(a), serde_json::Value::Number(e)) => {
                Ok(a.as_f64().unwrap_or(0.0) > e.as_f64().unwrap_or(0.0))
            }
            _ => Err(anyhow::anyhow!("Cannot compare non-numeric values for greater than")),
        }
    }

    fn compare_greater_than_or_equal(&self, actual: &serde_json::Value, expected: &serde_json::Value) -> Result<bool, anyhow::Error> {
        match (actual, expected) {
            (serde_json::Value::Number(a), serde_json::Value::Number(e)) => {
                Ok(a.as_f64().unwrap_or(0.0) >= e.as_f64().unwrap_or(0.0))
            }
            _ => Err(anyhow::anyhow!("Cannot compare non-numeric values for greater than or equal")),
        }
    }

    fn compare_less_than(&self, actual: &serde_json::Value, expected: &serde_json::Value) -> Result<bool, anyhow::Error> {
        match (actual, expected) {
            (serde_json::Value::Number(a), serde_json::Value::Number(e)) => {
                Ok(a.as_f64().unwrap_or(0.0) < e.as_f64().unwrap_or(0.0))
            }
            _ => Err(anyhow::anyhow!("Cannot compare non-numeric values for less than")),
        }
    }

    fn compare_less_than_or_equal(&self, actual: &serde_json::Value, expected: &serde_json::Value) -> Result<bool, anyhow::Error> {
        match (actual, expected) {
            (serde_json::Value::Number(a), serde_json::Value::Number(e)) => {
                Ok(a.as_f64().unwrap_or(0.0) <= e.as_f64().unwrap_or(0.0))
            }
            _ => Err(anyhow::anyhow!("Cannot compare non-numeric values for less than or equal")),
        }
    }

    fn compare_contains(&self, actual: &serde_json::Value, expected: &serde_json::Value) -> Result<bool, anyhow::Error> {
        match (actual, expected) {
            (serde_json::Value::String(a), serde_json::Value::String(e)) => {
                Ok(a.to_lowercase().contains(&e.to_lowercase()))
            }
            _ => Err(anyhow::anyhow!("Cannot compare non-string values for contains")),
        }
    }

    fn compare_in(&self, actual: &serde_json::Value, expected: &serde_json::Value) -> Result<bool, anyhow::Error> {
        match expected {
            serde_json::Value::Array(expected_values) => {
                Ok(expected_values.contains(actual))
            }
            _ => Err(anyhow::anyhow!("Expected value must be an array for 'in' comparison")),
        }
    }

    fn compare_regex(&self, actual: &serde_json::Value, expected: &serde_json::Value) -> Result<bool, anyhow::Error> {
        match (actual, expected) {
            (serde_json::Value::String(a), serde_json::Value::String(pattern)) => {
                let regex = regex::Regex::new(pattern)?;
                Ok(regex.is_match(a))
            }
            _ => Err(anyhow::anyhow!("Cannot compare non-string values for regex")),
        }
    }

    fn calculate_risk_level(&self, score: f64) -> RiskLevel {
        if score >= self.config.risk_thresholds.critical_min {
            RiskLevel::Critical
        } else if score >= self.config.risk_thresholds.high_min {
            RiskLevel::High
        } else if score >= self.config.risk_thresholds.medium_min {
            RiskLevel::Medium
        } else {
            RiskLevel::Low
        }
    }

    fn determine_response_action(&self, risk_level: RiskLevel, policy_set: &PolicySet) -> ResponseAction {
        match risk_level {
            RiskLevel::Low => ResponseAction::Allow,
            RiskLevel::Medium => ResponseAction::Monitor,
            RiskLevel::High => ResponseAction::Flag,
            RiskLevel::Critical => ResponseAction::Hold,
        }
    }

    fn calculate_context_hash(&self, context: &EvaluationContext) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        context.transaction.id.hash(&mut hasher);
        context.transaction.amount.to_bits().hash(&mut hasher);
        context.transaction.transaction_type.hash(&mut hasher);
        context.user_profile.kyc_tier.hash(&mut hasher);
        context.user_profile.jurisdiction.hash(&mut hasher);
        
        format!("{:x}", hasher.finish())
    }

    // Database operations
    async fn load_all_rules(&self) -> Result<Vec<AMLRule>, anyhow::Error> {
        // In a real implementation, this would load from the database
        // For now, return empty vector
        Ok(vec![])
    }

    async fn load_all_policy_sets(&self) -> Result<Vec<PolicySet>, anyhow::Error> {
        // In a real implementation, this would load from the database
        // For now, return empty vector
        Ok(vec![])
    }

    // Rule management operations
    pub async fn create_rule(&self, rule: AMLRule) -> Result<AMLRule, anyhow::Error> {
        rule.validate()?;
        
        // Insert into database
        let inserted_rule = self.insert_rule_into_db(&rule).await?;
        
        // Update cache
        let mut cache = self.rule_cache.write().unwrap();
        cache.insert(inserted_rule.id, inserted_rule.clone());
        
        // Invalidate evaluation cache
        if self.config.enable_caching {
            self.cache.invalidate_pattern("aml:evaluation:*").await?;
        }

        info!("Created new AML rule: {}", inserted_rule.id);
        Ok(inserted_rule)
    }

    pub async fn update_rule(&self, rule_id: Uuid, updates: AMLRule) -> Result<AMLRule, anyhow::Error> {
        updates.validate()?;
        
        // Update in database
        let updated_rule = self.update_rule_in_db(rule_id, &updates).await?;
        
        // Update cache
        let mut cache = self.rule_cache.write().unwrap();
        cache.insert(updated_rule.id, updated_rule.clone());
        
        // Invalidate evaluation cache
        if self.config.enable_caching {
            self.cache.invalidate_pattern("aml:evaluation:*").await?;
        }

        info!("Updated AML rule: {}", updated_rule.id);
        Ok(updated_rule)
    }

    pub async fn activate_rule(&self, rule_id: Uuid) -> Result<AMLRule, anyhow::Error> {
        let mut rule = self.get_rule_by_id(rule_id).await?;
        rule.status = AMLRuleStatus::Active;
        rule.updated_at = Utc::now();
        
        self.update_rule(rule_id, rule).await
    }

    pub async fn suspend_rule(&self, rule_id: Uuid) -> Result<AMLRule, anyhow::Error> {
        let mut rule = self.get_rule_by_id(rule_id).await?;
        rule.status = AMLRuleStatus::Suspended;
        rule.updated_at = Utc::now();
        
        self.update_rule(rule_id, rule).await
    }

    async fn get_rule_by_id(&self, rule_id: Uuid) -> Result<AMLRule, anyhow::Error> {
        let cache = self.rule_cache.read().unwrap();
        cache.get(&rule_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Rule not found: {}", rule_id))
    }

    async fn insert_rule_into_db(&self, rule: &AMLRule) -> Result<AMLRule, anyhow::Error> {
        // In a real implementation, this would insert into the database
        // For now, just return the rule with a new ID
        let mut new_rule = rule.clone();
        new_rule.id = Uuid::new_v4();
        Ok(new_rule)
    }

    async fn update_rule_in_db(&self, rule_id: Uuid, updates: &AMLRule) -> Result<AMLRule, anyhow::Error> {
        // In a real implementation, this would update in the database
        // For now, just return the updates with the original ID
        let mut updated_rule = updates.clone();
        updated_rule.id = rule_id;
        Ok(updated_rule)
    }

    // Backtesting operations
    pub async fn backtest_rules(&self, request: BacktestRequest) -> Result<BacktestResult, anyhow::Error> {
        if !self.config.enable_backtesting {
            return Err(anyhow::anyhow!("Backtesting is disabled"));
        }

        info!("Starting AML rule backtesting for period: {} to {}", 
            request.test_period_start, request.test_period_end);

        // Load historical transactions
        let historical_transactions = self.load_historical_transactions(&request).await?;
        
        let mut total_transactions = 0;
        let mut trigger_count = 0;
        let mut rule_evaluations = HashMap::new();

        for transaction in historical_transactions {
            let context = self.build_evaluation_context_from_transaction(&transaction).await?;
            
            // Evaluate against specified rules or policy set
            let result = if let Some(rule_id) = request.rule_id {
                // Test single rule
                let rule = self.get_rule_by_id(rule_id).await?;
                let rule_result = self.evaluate_rule(&rule, &context).await?;
                
                if rule_result.triggered {
                    trigger_count += 1;
                }
                
                // Track rule performance
                let entry = rule_evaluations.entry(rule_id).or_insert((0, 0));
                entry.0 += 1; // total evaluations
                if rule_result.triggered {
                    entry.1 += 1; // triggers
                }
                
                PolicyEvaluationResult {
                    policy_set_id: Uuid::new_v4(),
                    evaluation_context_id: context.transaction.id,
                    composite_risk_score: if rule_result.triggered { rule_result.confidence_score } else { 0.0 },
                    risk_level: if rule_result.triggered { self.calculate_risk_level(rule_result.confidence_score) } else { RiskLevel::Low },
                    triggered_rules: vec![rule_result],
                    recommended_response: ResponseAction::Allow,
                    evaluation_timestamp: Utc::now(),
                    evaluation_duration_ms: 0,
                    cache_hit: false,
                }
            } else if let Some(policy_set_id) = request.policy_set_id {
                // Test policy set
                let policy_set = self.get_policy_set_by_id(policy_set_id).await?;
                self.evaluate_transaction(&context).await?
            } else {
                // Test all active rules
                self.evaluate_transaction(&context).await?
            };

            total_transactions += 1;
        }

        let trigger_rate = if total_transactions > 0 {
            trigger_count as f64 / total_transactions as f64
        } else {
            0.0
        };

        let backtest_result = BacktestResult {
            rule_id: request.rule_id,
            policy_set_id: request.policy_set_id,
            test_period_start: request.test_period_start,
            test_period_end: request.test_period_end,
            total_transactions_tested: total_transactions,
            trigger_count,
            estimated_false_positive_count: 0, // Would need case outcome data
            estimated_detection_rate: trigger_rate,
            performance_metrics: RulePerformanceMetrics {
                rule_id: request.rule_id.unwrap_or_default(),
                period_start: request.test_period_start,
                period_end: request.test_period_end,
                total_evaluations: total_transactions as u64,
                trigger_count: trigger_count as u64,
                trigger_rate,
                false_positive_count: 0,
                false_positive_rate: 0.0,
                true_positive_count: 0,
                true_positive_rate: 0.0,
                average_confidence_score: 0.0,
                average_evaluation_time_ms: 0.0,
            },
            recommendations: vec![
                format!("Trigger rate: {:.2}%", trigger_rate * 100.0),
                "Review rule parameters if trigger rate is too high".to_string(),
            ],
        };

        info!("Backtesting completed: {} transactions tested, {} triggers ({:.2}% rate)", 
            total_transactions, trigger_count, trigger_rate * 100.0);

        Ok(backtest_result)
    }

    async fn load_historical_transactions(&self, request: &BacktestRequest) -> Result<Vec<TransactionData>, anyhow::Error> {
        // In a real implementation, this would load from the database
        // For now, return empty vector
        Ok(vec![])
    }

    async fn build_evaluation_context_from_transaction(&self, transaction: &TransactionData) -> Result<EvaluationContext, anyhow::Error> {
        // In a real implementation, this would build full context from database
        // For now, return minimal context
        Ok(EvaluationContext {
            transaction: transaction.clone(),
            user_profile: UserProfile {
                id: transaction.user_id,
                kyc_tier: KYCTier::Tier2,
                registration_date: Utc::now(),
                risk_score: 0.5,
                jurisdiction: "NG".to_string(),
                is_pep: false,
                is_sanctioned: false,
                watchlist_flags: vec![],
                verification_documents: vec![],
                account_status: AccountStatus::Active,
            },
            historical_metrics: HistoricalMetrics {
                transaction_count_24h: 5,
                transaction_count_7d: 20,
                transaction_count_30d: 50,
                total_volume_24h: 10000.0,
                total_volume_7d: 50000.0,
                total_volume_30d: 200000.0,
                average_transaction_size: 1000.0,
                max_transaction_size: 5000.0,
                unique_counterparties_30d: 10,
                last_transaction_date: Some(Utc::now()),
                account_age_days: 180,
                dormant_days: 0,
            },
            real_time_signals: RealTimeSignals {
                ip_risk_score: 0.2,
                device_risk_score: 0.1,
                behavioral_anomaly_score: 0.3,
                velocity_alert_count: 0,
                failed_auth_attempts: 0,
                concurrent_sessions: 1,
            },
            counterparty_data: None,
            session_context: SessionContext {
                session_id: "test_session".to_string(),
                login_timestamp: Utc::now(),
                authentication_method: "password".to_string(),
                mfa_enabled: true,
            },
        })
    }

    async fn get_policy_set_by_id(&self, policy_set_id: Uuid) -> Result<PolicySet, anyhow::Error> {
        let cache = self.policy_set_cache.read().unwrap();
        cache.get(&policy_set_id)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Policy set not found: {}", policy_set_id))
    }
}

// Supporting types
#[derive(Debug, Clone)]
struct ConditionEvaluationResult {
    satisfied: bool,
    confidence: f64,
    actual_value: serde_json::Value,
    threshold: serde_json::Value,
    description: String,
}

#[derive(Debug, Clone)]
pub struct BacktestRequest {
    pub rule_id: Option<Uuid>,
    pub policy_set_id: Option<Uuid>,
    pub test_period_start: DateTime<Utc>,
    pub test_period_end: DateTime<Utc>,
    pub sample_size: Option<u32>,
    pub random_sample: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_policy_engine_config_default() {
        let config = PolicyEngineConfig::default();
        assert!(config.enable_caching);
        assert!(config.enable_performance_monitoring);
        assert_eq!(config.max_rules_per_evaluation, 100);
    }

    #[test]
    fn test_risk_level_calculation() {
        let config = PolicyEngineConfig::default();
        let engine = AMLPolicyEngine {
            database: unsafe { std::mem::zeroed() }, // Placeholder for testing
            cache: Arc::new(unsafe { std::mem::zeroed() }), // Placeholder for testing
            config,
            rule_cache: Arc::new(std::sync::RwLock::new(HashMap::new())),
            policy_set_cache: Arc::new(std::sync::RwLock::new(HashMap::new())),
        };

        assert!(matches!(engine.calculate_risk_level(0.1), RiskLevel::Low));
        assert!(matches!(engine.calculate_risk_level(0.4), RiskLevel::Medium));
        assert!(matches!(engine.calculate_risk_level(0.7), RiskLevel::High));
        assert!(matches!(engine.calculate_risk_level(0.9), RiskLevel::Critical));
    }
}
