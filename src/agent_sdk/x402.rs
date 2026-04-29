use crate::agent_sdk::error::{AgentError, AgentResult};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, info, warn};

/// x402 payment negotiation client.
///
/// Implements the x402 protocol: when an API returns HTTP 402 Payment Required,
/// the agent automatically pays the requested micro-transaction in cNGN and
/// retries the original request.
pub struct X402Client {
    http: Client,
    /// Stellar public key of the paying agent.
    pub agent_address: String,
    /// Horizon URL for submitting payment transactions.
    horizon_url: String,
    /// cNGN asset issuer address.
    cngn_issuer: String,
}

/// Parsed 402 challenge from an API server.
#[derive(Debug, Deserialize)]
pub struct X402Challenge {
    /// Amount of cNGN required.
    pub amount: String,
    /// Recipient address for the payment.
    pub recipient: String,
    /// Unique nonce to include in the payment memo.
    pub nonce: String,
    /// Human-readable description.
    #[serde(default)]
    pub description: String,
}

/// Result of a successful x402 payment.
#[derive(Debug, Serialize)]
pub struct X402PaymentProof {
    pub tx_hash: String,
    pub amount: String,
    pub nonce: String,
}

impl X402Client {
    pub fn new(
        agent_address: String,
        horizon_url: String,
        cngn_issuer: String,
    ) -> Self {
        Self {
            http: Client::new(),
            agent_address,
            horizon_url,
            cngn_issuer,
        }
    }

    /// Perform a GET request to `url`, automatically handling HTTP 402 by
    /// paying the requested amount and retrying once.
    ///
    /// `pay_fn` is a callback that submits the actual Stellar payment and
    /// returns the transaction hash. This keeps the x402 client decoupled from
    /// the Stellar signing logic.
    pub async fn get_with_payment<F, Fut>(
        &self,
        url: &str,
        pay_fn: F,
    ) -> AgentResult<serde_json::Value>
    where
        F: Fn(String, String, String) -> Fut, // (amount, recipient, memo) -> tx_hash
        Fut: std::future::Future<Output = AgentResult<String>>,
    {
        debug!(url, "x402: sending initial request");
        let resp = self
            .http
            .get(url)
            .header("X-Agent-Address", &self.agent_address)
            .send()
            .await
            .map_err(|e| AgentError::Network(e.to_string()))?;

        if resp.status() != reqwest::StatusCode::PAYMENT_REQUIRED {
            let body = resp
                .json::<serde_json::Value>()
                .await
                .map_err(|e| AgentError::Network(e.to_string()))?;
            return Ok(body);
        }

        // Parse the 402 challenge.
        let challenge: X402Challenge = resp
            .json()
            .await
            .map_err(|e| AgentError::X402PaymentRequired(format!("Invalid 402 body: {e}")))?;

        info!(
            amount = %challenge.amount,
            recipient = %challenge.recipient,
            nonce = %challenge.nonce,
            "x402: paying for API access"
        );

        let tx_hash = pay_fn(
            challenge.amount.clone(),
            challenge.recipient.clone(),
            challenge.nonce.clone(),
        )
        .await?;

        let proof = X402PaymentProof {
            tx_hash,
            amount: challenge.amount,
            nonce: challenge.nonce,
        };

        // Retry with payment proof in header.
        let proof_json = serde_json::to_string(&proof)
            .map_err(|e| AgentError::X402PaymentRequired(e.to_string()))?;

        debug!(url, "x402: retrying request with payment proof");
        let retry_resp = self
            .http
            .get(url)
            .header("X-Agent-Address", &self.agent_address)
            .header("X-402-Payment", proof_json)
            .send()
            .await
            .map_err(|e| AgentError::Network(e.to_string()))?;

        if !retry_resp.status().is_success() {
            warn!(status = %retry_resp.status(), "x402: retry failed after payment");
            return Err(AgentError::X402PaymentRequired(format!(
                "API rejected payment proof, status: {}",
                retry_resp.status()
            )));
        }

        retry_resp
            .json::<serde_json::Value>()
            .await
            .map_err(|e| AgentError::Network(e.to_string()))
    }
}
