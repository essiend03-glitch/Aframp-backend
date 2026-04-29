use crate::agent_cfo::{
    ledger::ExpenditureLedger,
    types::{
        AgentKeyState, BudgetPolicy, BudgetUpdateReport, CostProjectionRequest,
        CostProjectionResponse, LlmTier, RecordInferenceRequest,
    },
};
use chrono::Utc;
use rust_decimal::Decimal;
use sqlx::PgPool;
use std::str::FromStr;
use std::sync::Arc;
use tracing::{info, warn};
use uuid::Uuid;

/// Fraction of daily budget at which the agent degrades to LlmTier::Efficient.
const DEGRADATION_THRESHOLD: f64 = 0.80;
/// Fraction at which a budget-update report is sent to the owner.
const REPORT_THRESHOLD: f64 = 0.50;

pub struct AgentCfoEngine {
    db: PgPool,
    ledger: Arc<ExpenditureLedger>,
}

impl AgentCfoEngine {
    pub fn new(db: PgPool) -> Self {
        let ledger = Arc::new(ExpenditureLedger::new(db.clone()));
        Self { db, ledger }
    }

    // ── Policy helpers ────────────────────────────────────────────────────────

    pub async fn get_policy(&self, agent_id: Uuid) -> Result<BudgetPolicy, String> {
        sqlx::query_as!(
            BudgetPolicy,
            r#"
            SELECT id, agent_id,
                   daily_limit_cngn, task_limit_cngn,
                   safety_buffer_cngn, refill_amount_cngn,
                   funding_account,
                   llm_tier AS "llm_tier: LlmTier",
                   key_state AS "key_state: AgentKeyState",
                   created_at, updated_at
            FROM agent_budget_policies WHERE agent_id = $1
            "#,
            agent_id,
        )
        .fetch_one(&self.db)
        .await
        .map_err(|e| format!("policy not found: {e}"))
    }

    // ── Pre-execution cost projection ─────────────────────────────────────────

    pub async fn project_cost(
        &self,
        req: CostProjectionRequest,
    ) -> Result<CostProjectionResponse, String> {
        let policy = self.get_policy(req.agent_id).await?;

        if policy.key_state == AgentKeyState::Frozen {
            return Err("Agent signing keys are frozen — task execution blocked".to_string());
        }

        let cost_per_llm = Decimal::from_str(&req.cost_per_llm_call)
            .map_err(|e| format!("cost_per_llm_call parse: {e}"))?;
        let cost_per_api = Decimal::from_str(&req.cost_per_api_call)
            .map_err(|e| format!("cost_per_api_call parse: {e}"))?;
        let projected = cost_per_llm * Decimal::from(req.estimated_llm_calls)
            + cost_per_api * Decimal::from(req.estimated_api_calls);

        let task_limit = Decimal::from_str(&policy.task_limit_cngn)
            .map_err(|e| format!("task_limit parse: {e}"))?;
        let daily_limit = Decimal::from_str(&policy.daily_limit_cngn)
            .map_err(|e| format!("daily_limit parse: {e}"))?;
        let spent_today = self.ledger.daily_spend(req.agent_id).await?;
        let remaining_daily = (daily_limit - spent_today).max(Decimal::ZERO);

        let within_task = projected <= task_limit;
        let within_daily = projected <= remaining_daily;
        let requires_approval = !within_task || !within_daily;

        Ok(CostProjectionResponse {
            agent_id: req.agent_id,
            projected_cost_cngn: projected.to_string(),
            within_task_budget: within_task,
            within_daily_budget: within_daily,
            llm_tier: policy.llm_tier,
            requires_approval,
        })
    }

    // ── Inference event recording ─────────────────────────────────────────────

    pub async fn record_inference(
        &self,
        req: RecordInferenceRequest,
    ) -> Result<serde_json::Value, String> {
        let policy = self.get_policy(req.agent_id).await?;

        if policy.key_state == AgentKeyState::Frozen {
            return Err("Agent keys are frozen — inference recording blocked".to_string());
        }

        let event = self
            .ledger
            .record(
                req.agent_id,
                req.task_id,
                &req.event_type,
                req.model_used.as_deref(),
                &req.cost_cngn,
                req.metadata.unwrap_or(serde_json::json!({})),
            )
            .await?;

        // ── Post-record checks ────────────────────────────────────────────────
        let daily_limit = Decimal::from_str(&policy.daily_limit_cngn)
            .map_err(|e| format!("daily_limit parse: {e}"))?;
        let spent = Decimal::from_str(&event.cumulative_daily_spend)
            .map_err(|e| format!("cumulative parse: {e}"))?;
        let ratio = if daily_limit.is_zero() {
            0.0
        } else {
            (spent / daily_limit).to_string().parse::<f64>().unwrap_or(0.0)
        };

        // Graceful degradation at 80 %
        if ratio >= DEGRADATION_THRESHOLD && policy.llm_tier == LlmTier::Advanced {
            warn!(
                agent_id = %req.agent_id,
                spend_pct = ratio * 100.0,
                "Budget 80% consumed — switching to LlmTier::Efficient"
            );
            let _ = sqlx::query!(
                "UPDATE agent_budget_policies SET llm_tier = 'efficient', updated_at = NOW() \
                 WHERE agent_id = $1",
                req.agent_id,
            )
            .execute(&self.db)
            .await;
        }

        // Budget update report at 50 %
        if ratio >= REPORT_THRESHOLD && ratio < DEGRADATION_THRESHOLD {
            self.emit_budget_report(&policy, spent, ratio).await;
        }

        // Trigger wallet refill if balance is low (non-blocking)
        {
            let db = self.db.clone();
            let agent_id = req.agent_id;
            let safety_buffer = policy.safety_buffer_cngn.clone();
            let refill_amount = policy.refill_amount_cngn.clone();
            let funding_account = policy.funding_account.clone();
            tokio::spawn(async move {
                let _ = maybe_refill(&db, agent_id, &safety_buffer, &refill_amount, &funding_account).await;
            });
        }

        Ok(serde_json::json!({
            "event_id": event.id,
            "cumulative_daily_spend": event.cumulative_daily_spend,
            "llm_tier": policy.llm_tier,
            "spend_percent": ratio * 100.0,
        }))
    }

    // ── Budget report ─────────────────────────────────────────────────────────

    async fn emit_budget_report(&self, policy: &BudgetPolicy, spent: Decimal, ratio: f64) {
        let report = BudgetUpdateReport {
            agent_id: policy.agent_id,
            daily_limit_cngn: policy.daily_limit_cngn.clone(),
            spent_today_cngn: spent.to_string(),
            spend_percent: ratio * 100.0,
            llm_tier: policy.llm_tier,
            key_state: policy.key_state,
            wallet_balance_cngn: "0".to_string(), // populated by caller if needed
            reported_at: Utc::now(),
        };

        info!(
            agent_id = %report.agent_id,
            spend_pct = report.spend_percent,
            "📊 Budget Update Report: agent has consumed {:.1}% of daily limit",
            report.spend_percent
        );

        // Persist the report so the Admin Dashboard can query it.
        let _ = sqlx::query!(
            r#"
            INSERT INTO agent_budget_reports
                (id, agent_id, daily_limit_cngn, spent_today_cngn, spend_percent,
                 llm_tier, key_state, reported_at)
            VALUES (gen_random_uuid(), $1, $2, $3, $4, $5::llm_tier, $6::agent_key_state, NOW())
            "#,
            report.agent_id,
            report.daily_limit_cngn,
            report.spent_today_cngn,
            report.spend_percent,
            report.llm_tier as LlmTier,
            report.key_state as AgentKeyState,
        )
        .execute(&self.db)
        .await;

        // Fire-and-forget webhook notification if configured.
        if let Ok(url) = std::env::var("AGENT_CFO_REPORT_WEBHOOK_URL") {
            let payload = serde_json::to_value(&report).unwrap_or_default();
            tokio::spawn(async move {
                let _ = reqwest::Client::new().post(&url).json(&payload).send().await;
            });
        }
    }

    pub fn ledger(&self) -> Arc<ExpenditureLedger> {
        Arc::clone(&self.ledger)
    }
}

// ── Wallet refill helper ──────────────────────────────────────────────────────

async fn maybe_refill(
    db: &PgPool,
    agent_id: Uuid,
    safety_buffer: &str,
    refill_amount: &str,
    funding_account: &str,
) -> Result<(), String> {
    // Read current wallet balance from the agent_wallets table.
    let balance_row = sqlx::query!(
        "SELECT balance_cngn FROM agent_wallets WHERE agent_id = $1",
        agent_id,
    )
    .fetch_optional(db)
    .await
    .map_err(|e| format!("wallet query: {e}"))?;

    let balance = match balance_row {
        Some(r) => Decimal::from_str(&r.balance_cngn).unwrap_or(Decimal::ZERO),
        None => return Ok(()),
    };

    let buffer = Decimal::from_str(safety_buffer).unwrap_or(Decimal::ZERO);
    if balance >= buffer {
        return Ok(());
    }

    // Record the refill intent — actual Stellar transfer is handled by the
    // treasury / payment layer that watches this table.
    sqlx::query!(
        r#"
        INSERT INTO agent_refill_requests
            (id, agent_id, refill_amount_cngn, funding_account, status, requested_at)
        VALUES (gen_random_uuid(), $1, $2, $3, 'pending', NOW())
        ON CONFLICT DO NOTHING
        "#,
        agent_id,
        refill_amount,
        funding_account,
    )
    .execute(db)
    .await
    .map(|_| ())
    .map_err(|e| format!("refill insert: {e}"))?;

    info!(
        agent_id = %agent_id,
        balance = %balance,
        buffer = %buffer,
        refill = %refill_amount,
        "💰 Wallet below safety buffer — refill request queued"
    );
    Ok(())
}
