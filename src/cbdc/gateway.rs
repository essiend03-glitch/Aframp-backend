use crate::cbdc::models::*;
use async_trait::async_trait;
use chrono::Utc;
use reqwest::Client as HttpClient;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{error, info, instrument, warn};

#[derive(Debug, Clone)]
pub struct DltGatewayConfig {
    pub connection_timeout_ms: u64,
    pub max_retries: u32,
    pub retry_backoff_ms: u64,
    pub rate_limit_rps: u32,
}

impl Default for DltGatewayConfig {
    fn default() -> Self {
        Self {
            connection_timeout_ms: 5000,
            max_retries: 3,
            retry_backoff_ms: 1000,
            rate_limit_rps: 10,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GatewayConnectionStatus {
    Healthy,
    Degraded,
    Unreachable,
    Unknown,
}

impl GatewayConnectionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            GatewayConnectionStatus::Healthy => "healthy",
            GatewayConnectionStatus::Degraded => "degraded",
            GatewayConnectionStatus::Unreachable => "unreachable",
            GatewayConnectionStatus::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone)]
pub struct DltGatewayMetrics {
    pub rpc_latency_ms: f64,
    pub block_height: Option<i64>,
    pub peer_count: Option<i32>,
    pub is_syncing: Option<bool>,
}

/// Enterprise DLT Gateway Client supporting Hyperledger Besu, Corda, and Quorum.
pub struct DltGatewayClient {
    gateway: CbdcGateway,
    http_client: HttpClient,
    config: DltGatewayConfig,
    status: Arc<RwLock<GatewayConnectionStatus>>,
    metrics: Arc<RwLock<Option<DltGatewayMetrics>>>,
}

impl DltGatewayClient {
    pub fn new(gateway: CbdcGateway, config: DltGatewayConfig) -> Self {
        let timeout = Duration::from_millis(gateway.connection_timeout_ms as u64);
        let http_client = HttpClient::builder()
            .timeout(timeout)
            .danger_accept_invalid_certs(false)
            .build()
            .unwrap_or_else(|_| HttpClient::new());

        Self {
            gateway,
            http_client,
            config,
            status: Arc::new(RwLock::new(GatewayConnectionStatus::Unknown)),
            metrics: Arc::new(RwLock::new(None)),
        }
    }

    pub fn gateway_id(&self) -> Uuid {
        self.gateway.id
    }

    pub fn gateway_name(&self) -> &str {
        &self.gateway.name
    }

    pub async fn current_status(&self) -> GatewayConnectionStatus {
        self.status.read().await.clone()
    }

    pub async fn current_metrics(&self) -> Option<DltGatewayMetrics> {
        self.metrics.read().await.clone()
    }

    /// Performs a health check against the DLT node's RPC endpoint.
    #[instrument(skip(self))]
    pub async fn health_check(&self) -> Result<GatewayConnectionStatus, String> {
        let start = std::time::Instant::now();

        let result = match self.gateway.dlt_system.as_str() {
            "Hyperledger Besu" | "Quorum" => {
                self.eth_json_rpc("eth_blockNumber", &[] as &[serde_json::Value]).await
            }
            "Corda" => {
                self.corda_rpc("net.healthCheck", &[] as &[serde_json::Value]).await
            }
            "Hyperledger Fabric" => {
                self.fabric_health_check().await
            }
            other => {
                return Err(format!("Unsupported DLT system: {}", other));
            }
        };

        let elapsed_ms = start.elapsed().as_secs_f64() * 1000.0;

        match result {
            Ok(resp) => {
                info!(
                    gateway = %self.gateway.name,
                    latency_ms = elapsed_ms,
                    "CBDC gateway health check succeeded"
                );

                let mut metrics = self.metrics.write().await;
                *metrics = Some(DltGatewayMetrics {
                    rpc_latency_ms: elapsed_ms,
                    block_height: None,
                    peer_count: None,
                    is_syncing: None,
                });

                if elapsed_ms > 2000.0 {
                    *self.status.write().await = GatewayConnectionStatus::Degraded;
                    Ok(GatewayConnectionStatus::Degraded)
                } else {
                    *self.status.write().await = GatewayConnectionStatus::Healthy;
                    Ok(GatewayConnectionStatus::Healthy)
                }
            }
            Err(e) => {
                warn!(
                    gateway = %self.gateway.name,
                    error = %e,
                    "CBDC gateway health check failed"
                );
                *self.status.write().await = GatewayConnectionStatus::Unreachable;
                Err(e)
            }
        }
    }

    /// Executes a JSON-RPC call against an Ethereum-compatible DLT node (Besu/Quorum).
    #[instrument(skip(self, params))]
    async fn eth_json_rpc(
        &self,
        method: &str,
        params: &[serde_json::Value],
    ) -> Result<serde_json::Value, String> {
        let body = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": 1,
        });

        let response = self
            .http_client
            .post(&self.gateway.rpc_endpoint)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("RPC request failed: {}", e))?;

        let json: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("RPC response parse failed: {}", e))?;

        if let Some(err) = json.get("error") {
            return Err(format!("RPC error: {}", err));
        }

        Ok(json)
    }

    /// Executes a Corda RPC call via REST proxy.
    #[instrument(skip(self, _params))]
    async fn corda_rpc(
        &self,
        _method: &str,
        _params: &[serde_json::Value],
    ) -> Result<serde_json::Value, String> {
        let response = self
            .http_client
            .post(&format!("{}/api/v1/health", self.gateway.rpc_endpoint))
            .send()
            .await
            .map_err(|e| format!("Corda RPC request failed: {}", e))?;

        response
            .json::<serde_json::Value>()
            .await
            .map_err(|e| format!("Corda response parse failed: {}", e))
    }

    /// Hyperledger Fabric health check via operations endpoint.
    async fn fabric_health_check(&self) -> Result<serde_json::Value, String> {
        let response = self
            .http_client
            .get(&format!("{}/healthz", self.gateway.rpc_endpoint))
            .send()
            .await
            .map_err(|e| format!("Fabric health check failed: {}", e))?;

        response
            .json::<serde_json::Value>()
            .await
            .map_err(|e| format!("Fabric response parse failed: {}", e))
    }

    /// Submits a signed transaction to the DLT network.
    #[instrument(skip(self, signed_tx))]
    pub async fn submit_transaction(&self, signed_tx: &[u8]) -> Result<String, String> {
        let tx_hex = hex::encode(signed_tx);
        let params = vec![serde_json::Value::String(format!("0x{}", tx_hex))];

        let response = self.eth_json_rpc("eth_sendRawTransaction", &params).await?;

        let tx_hash = response["result"]
            .as_str()
            .ok_or_else(|| "Missing transaction hash in RPC response".to_string())?
            .to_string();

        info!(
            gateway = %self.gateway.name,
            tx_hash = %tx_hash,
            "Transaction submitted to CBDC gateway"
        );

        Ok(tx_hash)
    }

    /// Gets the transaction receipt/record from the DLT network.
    #[instrument(skip(self))]
    pub async fn get_transaction_receipt(&self, tx_hash: &str) -> Result<serde_json::Value, String> {
        let params = vec![serde_json::Value::String(tx_hash.to_string())];
        self.eth_json_rpc("eth_getTransactionReceipt", &params).await
    }

    /// Gets the current block number from the DLT network.
    #[instrument(skip(self))]
    pub async fn get_block_number(&self) -> Result<i64, String> {
        let params: Vec<serde_json::Value> = vec![serde_json::Value::String("latest".to_string())];
        let response = self.eth_json_rpc("eth_blockNumber", &params).await?;

        let block_hex = response["result"]
            .as_str()
            .ok_or_else(|| "Missing block number".to_string())?;

        i64::from_str_radix(block_hex.trim_start_matches("0x"), 16)
            .map_err(|e| format!("Failed to parse block number: {}", e))
    }

    /// Waits for a transaction to achieve the required number of confirmations.
    #[instrument(skip(self))]
    pub async fn wait_for_confirmations(
        &self,
        tx_hash: &str,
        required_confirmations: u32,
        poll_interval_ms: u64,
        timeout_secs: u64,
    ) -> Result<(String, i64, i32), String> {
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(timeout_secs);

        loop {
            if start.elapsed() > timeout {
                return Err(format!(
                    "Timeout waiting for {} confirmations on tx {}",
                    required_confirmations, tx_hash
                ));
            }

            let receipt = self.get_transaction_receipt(tx_hash).await?;
            let block_hex = receipt["result"]["blockNumber"]
                .as_str()
                .ok_or_else(|| "Transaction not yet mined".to_string())?;
            let block_number = i64::from_str_radix(block_hex.trim_start_matches("0x"), 16)
                .map_err(|e| format!("Invalid block number: {}", e))?;

            let current_block = self.get_block_number().await?;
            let confirmations = (current_block - block_number + 1) as i32;

            if confirmations >= required_confirmations as i32 {
                let block_id = receipt["result"]["blockHash"]
                    .as_str()
                    .unwrap_or("unknown")
                    .to_string();
                return Ok((block_id, block_number, confirmations));
            }

            tokio::time::sleep(Duration::from_millis(poll_interval_ms)).await;
        }
    }
}

#[async_trait]
pub trait DltGateway: Send + Sync {
    async fn submit_swap(&self, payload: &serde_json::Value) -> Result<String, String>;
    async fn check_status(&self, tx_id: &str) -> Result<serde_json::Value, String>;
}

pub struct GatewayPool {
    clients: Arc<RwLock<Vec<Arc<DltGatewayClient>>>>,
}

impl GatewayPool {
    pub fn new() -> Self {
        Self {
            clients: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn add_client(&self, client: Arc<DltGatewayClient>) {
        self.clients.write().await.push(client);
    }

    pub async fn get_clients(&self) -> Vec<Arc<DltGatewayClient>> {
        self.clients.read().await.clone()
    }

    pub async fn get_active_clients(&self) -> Vec<Arc<DltGatewayClient>> {
        let clients = self.clients.read().await;
        let mut active = Vec::new();
        for client in clients.iter() {
            if client.current_status().await == GatewayConnectionStatus::Healthy {
                active.push(client.clone());
            }
        }
        active
    }
}
