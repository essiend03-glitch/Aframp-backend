use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Which LLM tier the agent is currently allowed to use.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "llm_tier", rename_all = "snake_case")]
pub enum LlmTier {
    /// Full-capability model — used when budget is healthy.
    Advanced,
    /// Cheaper model — activated when spend reaches 80 % of daily limit.
    Efficient,
}

/// Lifecycle state of an agent's signing keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "agent_key_state", rename_all = "snake_case")]
pub enum AgentKeyState {
    Active,
    /// Frozen by the watchdog due to a runaway burn-rate violation.
    Frozen,
}

/// Budget policy attached to a single agent.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct BudgetPolicy {
    pub id: Uuid,
    pub agent_id: Uuid,
    /// Maximum cNGN spend per calendar day.
    pub daily_limit_cngn: String,
    /// Maximum cNGN spend per individual task.
    pub task_limit_cngn: String,
    /// Wallet balance below which an automatic refill is triggered.
    pub safety_buffer_cngn: String,
    /// Amount transferred from the funding account on each refill.
    pub refill_amount_cngn: String,
    /// Stellar address of the human owner's funding account.
    pub funding_account: String,
    /// Current LLM tier (switches to Efficient at 80 % daily spend).
    pub llm_tier: LlmTier,
    /// Current key state (frozen when watchdog fires).
    pub key_state: AgentKeyState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// A single inference / API call cost event written to the expenditure ledger.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct InferenceEvent {
    pub id: Uuid,
    pub agent_id: Uuid,
    pub task_id: Option<Uuid>,
    /// "llm_call" | "api_request" | "data_query"
    pub event_type: String,
    pub model_used: Option<String>,
    pub cost_cngn: String,
    pub cumulative_daily_spend: String,
    pub metadata: serde_json::Value,
    pub recorded_at: DateTime<Utc>,
}

/// Projected cost check before a task is executed.
#[derive(Debug, Deserialize)]
pub struct CostProjectionRequest {
    pub agent_id: Uuid,
    pub task_id: Option<Uuid>,
    /// Estimated number of LLM calls.
    pub estimated_llm_calls: u32,
    /// Estimated number of external API calls.
    pub estimated_api_calls: u32,
    /// Cost per LLM call in cNGN.
    pub cost_per_llm_call: String,
    /// Cost per API call in cNGN.
    pub cost_per_api_call: String,
}

#[derive(Debug, Serialize)]
pub struct CostProjectionResponse {
    pub agent_id: Uuid,
    pub projected_cost_cngn: String,
    /// Whether the projected cost fits within the task budget.
    pub within_task_budget: bool,
    /// Whether the projected cost fits within today's remaining daily budget.
    pub within_daily_budget: bool,
    /// Current LLM tier the agent should use.
    pub llm_tier: LlmTier,
    /// Human approval required before execution.
    pub requires_approval: bool,
}

/// Record an inference event (called by the agent after each LLM/API call).
#[derive(Debug, Deserialize)]
pub struct RecordInferenceRequest {
    pub agent_id: Uuid,
    pub task_id: Option<Uuid>,
    pub event_type: String,
    pub model_used: Option<String>,
    pub cost_cngn: String,
    pub metadata: Option<serde_json::Value>,
}

/// Budget status report sent to the owner at 50 % threshold.
#[derive(Debug, Serialize)]
pub struct BudgetUpdateReport {
    pub agent_id: Uuid,
    pub daily_limit_cngn: String,
    pub spent_today_cngn: String,
    pub spend_percent: f64,
    pub llm_tier: LlmTier,
    pub key_state: AgentKeyState,
    pub wallet_balance_cngn: String,
    pub reported_at: DateTime<Utc>,
}

/// Query params for listing inference events.
#[derive(Debug, Deserialize)]
pub struct LedgerQuery {
    pub agent_id: Option<Uuid>,
    pub task_id: Option<Uuid>,
    pub page: Option<i64>,
    pub page_size: Option<i64>,
}

impl LedgerQuery {
    pub fn page(&self) -> i64 { self.page.unwrap_or(1).max(1) }
    pub fn page_size(&self) -> i64 { self.page_size.unwrap_or(50).clamp(1, 200) }
    pub fn offset(&self) -> i64 { (self.page() - 1) * self.page_size() }
}
