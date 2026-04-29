use crate::travel_rule::models::*;
use anyhow::{anyhow, Result};
use chrono::{Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;
use tracing::{info, warn};

/// Jurisdictional threshold above which Travel Rule applies (USD equivalent)
const TRAVEL_RULE_THRESHOLD_USD: f64 = 1000.0;
/// Timeout for counterparty acknowledgement
const HANDSHAKE_TIMEOUT_SECS: i64 = 300;

pub struct TravelRuleService {
    pool: PgPool,
}

impl TravelRuleService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Initiate a Travel Rule exchange for an outbound transfer.
    /// The transaction remains in "pending-travel-rule" state until acknowledged.
    pub async fn initiate_exchange(&self, req: InitiateTravelRuleRequest) -> Result<TravelRuleExchange> {
        // Look up counterparty VASP capabilities
        let vasp = self.lookup_vasp(&req.beneficiary_vasp_id).await?;
        let protocol = self.select_protocol(&vasp);

        let exchange_id = Uuid::new_v4();
        let now = Utc::now();
        let timeout_at = now + Duration::seconds(HANDSHAKE_TIMEOUT_SECS);

        let originator_json = serde_json::to_value(&req.originator)?;
        let beneficiary_json = serde_json::to_value(&req.beneficiary)?;

        let exchange = sqlx::query_as::<_, TravelRuleExchange>(
            r#"INSERT INTO travel_rule_exchanges (
                exchange_id, transaction_id, originator_vasp_id, beneficiary_vasp_id,
                protocol_used, status, originator_ivms101, beneficiary_ivms101,
                transfer_amount, asset_code, handshake_initiated_at, timeout_at,
                created_at, updated_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)
            RETURNING *"#,
        )
        .bind(exchange_id)
        .bind(&req.transaction_id)
        .bind("self") // originator is always this VASP
        .bind(&req.beneficiary_vasp_id)
        .bind(&protocol)
        .bind(TravelRuleStatus::Pending)
        .bind(&originator_json)
        .bind(&beneficiary_json)
        .bind(&req.transfer_amount)
        .bind(&req.asset_code)
        .bind(now)
        .bind(timeout_at)
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;

        info!(
            exchange_id = %exchange_id,
            transaction_id = %req.transaction_id,
            protocol = ?protocol,
            "Travel Rule exchange initiated"
        );

        Ok(exchange)
    }

    /// Acknowledge receipt of inbound Travel Rule data from another VASP.
    /// Verifies beneficiary identity before crediting.
    pub async fn acknowledge_inbound(&self, data: InboundTravelRuleData) -> Result<TravelRuleExchange> {
        // Verify beneficiary against internal profile (stub — real impl queries KYC)
        self.verify_beneficiary_identity(&data).await?;

        let exchange = sqlx::query_as::<_, TravelRuleExchange>(
            r#"INSERT INTO travel_rule_exchanges (
                exchange_id, transaction_id, originator_vasp_id, beneficiary_vasp_id,
                protocol_used, status, originator_ivms101, beneficiary_ivms101,
                transfer_amount, asset_code, handshake_initiated_at, acknowledged_at,
                timeout_at, created_at, updated_at
            ) VALUES ($1,$2,$3,'self',$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)
            RETURNING *"#,
        )
        .bind(data.exchange_id)
        .bind(&data.transaction_id)
        .bind(&data.originator_vasp_id)
        .bind(&data.protocol_used)
        .bind(TravelRuleStatus::Acknowledged)
        .bind(serde_json::to_value(&data.originator)?)
        .bind(serde_json::to_value(&data.beneficiary)?)
        .bind(&data.transfer_amount)
        .bind(&data.asset_code)
        .bind(Utc::now())
        .bind(Utc::now())
        .bind(Utc::now() + Duration::seconds(HANDSHAKE_TIMEOUT_SECS))
        .bind(Utc::now())
        .bind(Utc::now())
        .fetch_one(&self.pool)
        .await?;

        info!(exchange_id = %data.exchange_id, "Inbound Travel Rule data acknowledged");
        Ok(exchange)
    }

    /// Mark an exchange as acknowledged by the counterparty VASP
    pub async fn mark_acknowledged(&self, exchange_id: Uuid) -> Result<()> {
        sqlx::query(
            "UPDATE travel_rule_exchanges SET status = $1, acknowledged_at = $2, updated_at = $3 WHERE exchange_id = $4"
        )
        .bind(TravelRuleStatus::Acknowledged)
        .bind(Utc::now())
        .bind(Utc::now())
        .bind(exchange_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Expire timed-out pending exchanges and route to manual review
    pub async fn expire_timed_out_exchanges(&self) -> Result<u64> {
        let result = sqlx::query(
            r#"UPDATE travel_rule_exchanges
               SET status = 'timed_out', updated_at = NOW()
               WHERE status = 'pending' AND timeout_at < NOW()"#,
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Check if a transfer amount exceeds the jurisdictional Travel Rule threshold
    pub fn requires_travel_rule(&self, amount_usd: f64) -> bool {
        amount_usd >= TRAVEL_RULE_THRESHOLD_USD
    }

    /// Look up a VASP from the registry
    pub async fn lookup_vasp(&self, vasp_id: &str) -> Result<VaspRegistryEntry> {
        let vasp = sqlx::query_as::<_, VaspRegistryEntry>(
            "SELECT * FROM vasp_registry WHERE vasp_id = $1"
        )
        .bind(vasp_id)
        .fetch_optional(&self.pool)
        .await?;

        match vasp {
            Some(v) => Ok(v),
            None => {
                warn!(vasp_id = vasp_id, "Unknown VASP — routing to manual review");
                Err(anyhow!("VASP not found in registry: {}", vasp_id))
            }
        }
    }

    /// Select the best supported protocol for a VASP
    fn select_protocol(&self, vasp: &VaspRegistryEntry) -> TravelRuleProtocol {
        if vasp.supported_protocols.iter().any(|p| p == "trisa") {
            TravelRuleProtocol::Trisa
        } else if vasp.supported_protocols.iter().any(|p| p == "trust") {
            TravelRuleProtocol::Trust
        } else if vasp.supported_protocols.iter().any(|p| p == "openvasp") {
            TravelRuleProtocol::OpenVasp
        } else {
            TravelRuleProtocol::Ivms101Direct
        }
    }

    /// Verify inbound beneficiary identity against internal KYC profile
    async fn verify_beneficiary_identity(&self, data: &InboundTravelRuleData) -> Result<()> {
        // In production: query KYC service and compare name/DOB against received IVMS101 data
        // For now: accept if beneficiary data is present
        match &data.beneficiary {
            Ivms101Person::Natural(p) if p.first_name.is_empty() || p.last_name.is_empty() => {
                Err(anyhow!("Beneficiary identity verification failed: missing required fields"))
            }
            Ivms101Person::Legal(p) if p.legal_name.is_empty() => {
                Err(anyhow!("Beneficiary identity verification failed: missing legal name"))
            }
            _ => Ok(()),
        }
    }

    pub async fn get_exchange(&self, exchange_id: Uuid) -> Result<TravelRuleExchange> {
        let exchange = sqlx::query_as::<_, TravelRuleExchange>(
            "SELECT * FROM travel_rule_exchanges WHERE exchange_id = $1"
        )
        .bind(exchange_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(exchange)
    }
}
