//! Bank Account Verification Service
//! Validates external account numbers against bank codes using banking partner APIs

use crate::cache::RedisCache;
use chrono::Utc;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tracing::{error, info, instrument, warn};
use uuid::Uuid;

/// Bank Account Verification Configuration
#[derive(Debug, Clone)]
pub struct BankVerificationConfig {
    /// Timeout for name enquiry requests (seconds)
    pub request_timeout_seconds: u64,
    /// Maximum retries for failed requests
    pub max_retries: u32,
    /// Cache TTL for verification results (seconds)
    pub cache_ttl_seconds: u64,
}

impl Default for BankVerificationConfig {
    fn default() -> Self {
        Self {
            request_timeout_seconds: 3,
            max_retries: 2,
            cache_ttl_seconds: 3600,
        }
    }
}

/// Account verification result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AccountVerificationResult {
    pub account_number: String,
    pub bank_code: String,
    pub account_name: String,
    pub bank_name: String,
    pub verified: bool,
    pub response_code: Option<String>,
    pub request_id: String,
    pub verified_at: DateTime<Utc>,
}

/// Bank name enquiry API request
#[derive(Debug, Serialize)]
struct NameEnquiryRequest {
    account_number: String,
    bank_code: String,
}

/// Bank name enquiry API response (Flutterwave format)
#[derive(Debug, Deserialize)]
struct NameEnquiryResponse {
    pub status: String,
    pub message: String,
    pub data: Option<NameEnquiryData>,
    #[serde(rename = "response_code")]
    pub response_code: Option<String>,
}

#[derive(Debug, Deserialize)]
struct NameEnquiryData {
    #[serde(rename = "account_number")]
    pub account_number: String,
    #[serde(rename = "account_status")]
    pub account_status: Option<String>,
    #[serde(rename = "account_name")]
    pub account_name: String,
    #[serde(rename = "bank_name")]
    pub bank_name: String,
    #[serde(rename = "bank_code")]
    pub bank_code: String,
}

/// Bank name enquiry response (Paystack format)
#[derive(Debug, Deserialize)]
struct PaystackNameEnquiryResponse {
    pub status: bool,
    pub message: String,
    pub data: Option<PaystackNameEnquiryData>,
}

#[derive(Debug, Deserialize)]
struct PaystackNameEnquiryResponseData {
    pub account_number: String,
    pub account_name: String,
    pub bank_id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BankProvider {
    Flutterwave,
    Paystack,
    #[serde(other)]
    Unknown,
}

/// Bank Account Verification Service
pub struct BankVerificationService {
    config: BankVerificationConfig,
    http: Client,
    cache: Arc<RedisCache>,
}

impl BankVerificationService {
    pub fn new(config: BankVerificationConfig, cache: Arc<RedisCache>) -> Self {
        Self {
            config,
            http: Client::builder()
                .timeout(Duration::from_secs(config.request_timeout_seconds))
                .build()
                .expect("Failed to build HTTP client"),
            cache,
        }
    }

    /// Verify account number against bank code via name enquiry
    /// Returns result within 3 seconds or times out
    #[instrument(skip(self), fields(bank_code = %bank_code, account_number = %account_number))]
    pub async fn verify_account(
        &self,
        bank_code: &str,
        account_number: &str,
    ) -> Result<AccountVerificationResult, VerificationError> {
        // Check cache first
        let cache_key = format!("bank:verify:{}:{}", bank_code, account_number);
        if let Ok(Some(cached)) = self.cache.get::<AccountVerificationResult>(&cache_key).await {
            info!(cached = true, "Using cached verification result");
            return Ok(cached);
        }

        // Attempt verification with retries
        let mut last_error = None;
        for attempt in 0..self.config.max_retries {
            match self.perform_name_enquiry(bank_code, account_number).await {
                Ok(result) => {
                    // Cache successful result
                    let _ = self
                        .cache
                        .set(
                            &cache_key,
                            &result,
                            Some(Duration::from_secs(self.config.cache_ttl_seconds)),
                        )
                        .await;
                    return Ok(result);
                }
                Err(e) => {
                    warn!(attempt = attempt + 1, error = %e, "Name enquiry failed");
                    last_error = Some(e);
                    if attempt < self.config.max_retries - 1 {
                        tokio::time::sleep(Duration::from_millis(100 * (attempt + 1) as u64)).await;
                    }
                }
            }
        }

        Err(last_error.unwrap_or(VerificationError::Timeout))
    }

    /// Perform actual name enquiry API call
    async fn perform_name_enquiry(
        &self,
        bank_code: &str,
        account_number: &str,
    ) -> Result<AccountVerificationResult, VerificationError> {
        let start = Instant::now();

        // Try Flutterwave first (primary provider)
        let result = self
            .verify_via_flutterwave(bank_code, account_number)
            .await;

        let latency_ms = start.elapsed().as_millis() as i64;

        // Record metrics (would be done via metrics service in production)
        info!(latency_ms = latency_ms, provider = "flutterwave", "Bank verification completed");

        result
    }

    /// Verify via Flutterwave API
    async fn verify_via_flutterwave(
        &self,
        bank_code: &str,
        account_number: &str,
    ) -> Result<AccountVerificationResult, VerificationError> {
        let api_key = std::env::var("FLUTTERWAVE_SECRET_KEY")
            .map_err(|_| VerificationError::Configuration("FLUTTERWAVE_SECRET_KEY not set"))?;

        let url = format!(
            "https://api.flutterwave.com/v3/accounts/resolve/{}/{}",
            bank_code, account_number
        );

        let response = self
            .http
            .get(&url)
            .bearer_auth(&api_key)
            .timeout(Duration::from_secs(self.config.request_timeout_seconds))
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    VerificationError::Timeout
                } else {
                    VerificationError::Network(e.to_string())
                }
            })?;

        if !response.status().is_success() {
            let status = response.status();
            if status.as_u16() == 404 {
                return Err(VerificationError::AccountNotFound);
            }
            return Err(VerificationError::ProviderError(format!(
                "Status: {}",
                status
            )));
        }

        let body = response
            .text()
            .await
            .map_err(|e| VerificationError::Parse(e.to_string()))?;

        let api_response: NameEnquiryResponse =
            serde_json::from_str(&body).map_err(|e| VerificationError::Parse(e.to_string()))?;

        if api_response.status != "success" {
            return Err(VerificationError::ProviderError(
                api_response.message.clone(),
            ));
        }

        let data = api_response
            .data
            .ok_or(VerificationError::ProviderError("No data in response".into()))?;

        Ok(AccountVerificationResult {
            account_number: data.account_number,
            bank_code: data.bank_code,
            account_name: data.account_name,
            bank_name: data.bank_name,
            verified: true,
            response_code: api_response.response_code,
            request_id: Uuid::new_v4().to_string(),
            verified_at: Utc::now(),
        })
    }

    /// Verify via Paystack API (fallback)
    async fn verify_via_paystack(
        &self,
        bank_code: &str,
        account_number: &str,
    ) -> Result<AccountVerificationResult, VerificationError> {
        let api_key = std::env::var("PAYSTACK_SECRET_KEY")
            .map_err(|_| VerificationError::Configuration("PAYSTACK_SECRET_KEY not set"))?;

        let url = format!(
            "https://api.paystack.co/bank/resolve?account_number={}&bank_code={}",
            account_number, bank_code
        );

        let response = self
            .http
            .get(&url)
            .bearer_auth(&api_key)
            .timeout(Duration::from_secs(self.config.request_timeout_seconds))
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    VerificationError::Timeout
                } else {
                    VerificationError::Network(e.to_string())
                }
            })?;

        if !response.status().is_success() {
            let status = response.status();
            if status.as_u16() == 404 {
                return Err(VerificationError::AccountNotFound);
            }
            return Err(VerificationError::ProviderError(format!(
                "Status: {}",
                status
            )));
        }

        let body = response
            .text()
            .await
            .map_err(|e| VerificationError::Parse(e.to_string()))?;

        let api_response: PaystackNameEnquiryResponse =
            serde_json::from_str(&body).map_err(|e| VerificationError::Parse(e.to_string()))?;

        if !api_response.status {
            return Err(VerificationError::ProviderError(api_response.message));
        }

        let data = api_response
            .data
            .ok_or(VerificationError::ProviderError("No data in response".into()))?;

        Ok(AccountVerificationResult {
            account_number: data.account_number,
            bank_code: bank_code.to_string(),
            account_name: data.account_name,
            bank_name: "Bank".to_string(), // Paystack doesn't return bank name in resolve
            verified: true,
            response_code: None,
            request_id: Uuid::new_v4().to_string(),
            verified_at: Utc::now(),
        })
    }
}

/// Verification errors
#[derive(Debug, thiserror::Error)]
pub enum VerificationError {
    #[error("Account not found")]
    AccountNotFound,

    #[error("Request timed out")]
    Timeout,

    #[error("Network error: {0}")]
    Network(String),

    #[error("Provider error: {0}")]
    ProviderError(String),

    #[error("Parse error: {0}")]
    Parse(String),

    #[error("Configuration error: {0}")]
    Configuration(&'static str),
}

impl std::fmt::Display for VerificationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerificationError::AccountNotFound => write!(f, "Account not found"),
            VerificationError::Timeout => write!(f, "Request timed out"),
            VerificationError::Network(s) => write!(f, "Network error: {}", s),
            VerificationError::ProviderError(s) => write!(f, "Provider error: {}", s),
            VerificationError::Parse(s) => write!(f, "Parse error: {}", s),
            VerificationError::Configuration(s) => write!(f, "Configuration error: {}", s),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_verification_result_serialization() {
        let result = AccountVerificationResult {
            account_number: "1234567890".to_string(),
            bank_code: "044".to_string(),
            account_name: "JOHN DOE".to_string(),
            bank_name: "Access Bank".to_string(),
            verified: true,
            response_code: Some("00".to_string()),
            request_id: "test-uuid".to_string(),
            verified_at: Utc::now(),
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("JOHN DOE"));
    }
}