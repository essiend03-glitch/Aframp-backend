/// Horizon API client for transaction submission and confirmation polling
use crate::stellar::error::{SubmissionError, SubmissionResult, HorizonErrorCode};
use crate::stellar::models::HorizonTransaction;
use serde::Deserialize;
use std::time::Duration;

#[derive(Clone)]
pub struct HorizonClient {
    base_url: String,
    client: reqwest::Client,
    request_timeout: Duration,
}

#[derive(Debug, Deserialize)]
pub struct HorizonErrorResponse {
    pub status: Option<u16>,
    pub type_url: Option<String>,
    pub title: Option<String>,
    pub detail: Option<String>,
    pub instance: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TransactionsResponse {
    pub _links: Option<serde_json::Value>,
    pub records: Vec<HorizonTransaction>,
}

impl HorizonClient {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: reqwest::Client::new(),
            request_timeout: Duration::from_secs(15),
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.request_timeout = timeout;
        self
    }

    /// Submit a transaction to Horizon
    pub async fn submit_transaction(&self, tx_envelope: &str) -> SubmissionResult<HorizonTransaction> {
        let url = format!("{}/transactions", self.base_url);

        let mut params = std::collections::HashMap::new();
        params.insert("tx", tx_envelope);

        let response = self
            .client
            .post(&url)
            .form(&params)
            .timeout(self.request_timeout)
            .send()
            .await
            .map_err(|e| SubmissionError::HorizonApi(format!("POST /transactions failed: {}", e)))?;

        let status = response.status();

        if status.is_success() {
            let tx: HorizonTransaction = response
                .json()
                .await
                .map_err(|e| {
                    SubmissionError::HorizonApi(format!("failed to parse transaction response: {}", e))
                })?;
            Ok(tx)
        } else {
            let error_msg = response
                .text()
                .await
                .unwrap_or_else(|_| "unknown error".to_string());

            // Try to parse as Horizon error
            if let Ok(horizon_err) = serde_json::from_str::<HorizonErrorResponse>(&error_msg) {
                let detail = horizon_err.detail.unwrap_or_default();
                let error_code = HorizonErrorCode::from_str(&detail);
                
                return Err(match error_code {
                    HorizonErrorCode::TxBadSeq => {
                        SubmissionError::BadSequence(detail)
                    }
                    HorizonErrorCode::TxInsufficientFee => {
                        SubmissionError::InsufficientFee {
                            provided: 0,
                            required: 0,
                        }
                    }
                    HorizonErrorCode::TxMalformed => {
                        SubmissionError::MalformedTransaction(detail)
                    }
                    _ if error_code.is_retryable() => {
                        SubmissionError::TransientNetworkError {
                            source: detail,
                            attempt: 1,
                        }
                    }
                    _ => SubmissionError::UnknownHorizonError {
                        code: status.to_string(),
                        message: detail,
                    },
                });
            }

            Err(SubmissionError::HorizonApi(format!(
                "submission failed ({}): {}",
                status, error_msg
            )))
        }
    }

    /// Get transaction by hash from Horizon
    pub async fn get_transaction(&self, tx_hash: &str) -> SubmissionResult<Option<HorizonTransaction>> {
        let url = format!("{}/transactions/{}", self.base_url, tx_hash);

        let response = self
            .client
            .get(&url)
            .timeout(self.request_timeout)
            .send()
            .await
            .map_err(|e| SubmissionError::HorizonApi(format!("GET /transactions/{{}} failed: {}", e)))?;

        if response.status() == 404 {
            return Ok(None);
        }

        if response.status().is_success() {
            let tx: HorizonTransaction = response
                .json()
                .await
                .map_err(|e| {
                    SubmissionError::HorizonApi(format!("failed to parse transaction response: {}", e))
                })?;
            Ok(Some(tx))
        } else {
            Err(SubmissionError::HorizonApi(format!(
                "failed to fetch transaction: {}",
                response.status()
            )))
        }
    }

    /// Get account details including current sequence
    pub async fn get_account_sequence(&self, account_id: &str) -> SubmissionResult<i64> {
        let url = format!("{}/accounts/{}", self.base_url, account_id);

        #[derive(Deserialize)]
        struct AccountResponse {
            sequence: String,
        }

        let response = self
            .client
            .get(&url)
            .timeout(self.request_timeout)
            .send()
            .await
            .map_err(|e| {
                SubmissionError::HorizonApi(format!("failed to fetch account sequence: {}", e))
            })?;

        if response.status().is_success() {
            let account: AccountResponse = response
                .json()
                .await
                .map_err(|e| {
                    SubmissionError::HorizonApi(format!("failed to parse account response: {}", e))
                })?;

            account
                .sequence
                .parse::<i64>()
                .map_err(|_| {
                    SubmissionError::HorizonApi("invalid sequence format".to_string())
                })
        } else if response.status() == 404 {
            Err(SubmissionError::HorizonApi(format!(
                "account {} not found",
                account_id
            )))
        } else {
            Err(SubmissionError::HorizonApi(format!(
                "failed to fetch account: {}",
                response.status()
            )))
        }
    }

    /// Poll for transaction confirmation (exponential backoff)
    pub async fn poll_transaction_confirmation(
        &self,
        tx_hash: &str,
        max_attempts: u32,
    ) -> SubmissionResult<Option<HorizonTransaction>> {
        let mut backoff_ms = 100u64;
        let mut attempt = 0;

        loop {
            attempt += 1;

            match self.get_transaction(tx_hash).await? {
                Some(tx) => return Ok(Some(tx)),
                None if attempt >= max_attempts => return Ok(None),
                None => {
                    tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
                    backoff_ms = (backoff_ms * 2).min(5000); // Cap at 5s
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = HorizonClient::new("https://horizon-testnet.stellar.org".to_string());
        assert_eq!(client.base_url, "https://horizon-testnet.stellar.org");
    }

    #[test]
    fn test_client_with_timeout() {
        let client = HorizonClient::new("https://horizon-testnet.stellar.org".to_string())
            .with_timeout(Duration::from_secs(30));
        assert_eq!(client.request_timeout, Duration::from_secs(30));
    }
}
