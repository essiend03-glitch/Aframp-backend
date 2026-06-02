use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{info, instrument, warn};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum HsmSigningAlgorithm {
    #[serde(rename = "ECDSA-P256")]
    EcdsaP256,
    #[serde(rename = "ECDSA-P384")]
    EcdsaP384,
    Ed25519,
    #[serde(rename = "RSA-2048")]
    Rsa2048,
    #[serde(rename = "PKCS11-HSM")]
    Pkcs11Hsm,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HsmSignature {
    pub signature_hex: String,
    pub signing_key_id: String,
    pub algorithm: HsmSigningAlgorithm,
    pub signed_at: String,
    pub public_key: Option<String>,
}

#[derive(Debug, Clone)]
pub struct HsmClientConfig {
    pub hsm_url: String,
    pub api_key: String,
    pub timeout_secs: u64,
    pub signing_key_label: String,
    pub algorithm: HsmSigningAlgorithm,
}

impl Default for HsmClientConfig {
    fn default() -> Self {
        Self {
            hsm_url: std::env::var("CBDC_HSM_URL")
                .unwrap_or_else(|_| "http://localhost:8080".to_string()),
            api_key: std::env::var("CBDC_HSM_API_KEY").unwrap_or_default(),
            timeout_secs: std::env::var("CBDC_HSM_TIMEOUT_SECS")
                .ok().and_then(|v| v.parse().ok()).unwrap_or(10),
            signing_key_label: std::env::var("CBDC_HSM_KEY_LABEL")
                .unwrap_or_else(|_| "cbdc-sovereign-key".to_string()),
            algorithm: HsmSigningAlgorithm::Pkcs11Hsm,
        }
    }
}

/// HSM (Hardware Security Module) signing client using PKCS#11 protocols.
///
/// Authenticates payload signatures with institutional central bank keys.
/// Communicates with an HSM proxy service that wraps the PKCS#11 interface.
pub struct HsmClient {
    config: HsmClientConfig,
    client: reqwest::Client,
}

impl HsmClient {
    pub fn new(config: HsmClientConfig) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        Self { config, client }
    }

    /// Signs the given payload using the HSM's configured signing key.
    #[instrument(skip(self, payload))]
    pub async fn sign(&self, payload: &[u8]) -> Result<HsmSignature, String> {
        let payload_hex = hex::encode(payload);

        let body = serde_json::json!({
            "key_label": self.config.signing_key_label,
            "algorithm": match self.config.algorithm {
                HsmSigningAlgorithm::EcdsaP256 => "ECDSA-P256",
                HsmSigningAlgorithm::EcdsaP384 => "ECDSA-P384",
                HsmSigningAlgorithm::Ed25519 => "ED25519",
                HsmSigningAlgorithm::Rsa2048 => "RSA-2048",
                HsmSigningAlgorithm::Pkcs11Hsm => "PKCS11-HSM",
            },
            "payload_hex": payload_hex,
            "payload_encoding": "hex",
        });

        let response = self
            .client
            .post(&format!("{}/api/v1/sign", self.config.hsm_url))
            .header("X-API-Key", &self.config.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("HSM signing request failed: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body_text = response.text().await.unwrap_or_default();
            return Err(format!("HSM signing failed ({}): {}", status, body_text));
        }

        let result: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("HSM response parse failed: {}", e))?;

        let signature_hex = result["signature_hex"]
            .as_str()
            .ok_or_else(|| "Missing signature in HSM response".to_string())?
            .to_string();

        let signing_key_id = result["key_id"]
            .as_str()
            .unwrap_or(&self.config.signing_key_label)
            .to_string();

        info!(
            algorithm = ?self.config.algorithm,
            key_id = %signing_key_id,
            "HSM signing completed successfully"
        );

        Ok(HsmSignature {
            signature_hex,
            signing_key_id,
            algorithm: self.config.algorithm.clone(),
            signed_at: chrono::Utc::now().to_rfc3339(),
            public_key: result["public_key"].as_str().map(|s| s.to_string()),
        })
    }

    /// Verifies a signature against the original payload using the HSM.
    #[instrument(skip(self, payload))]
    pub async fn verify(
        &self,
        payload: &[u8],
        signature: &HsmSignature,
    ) -> Result<bool, String> {
        let body = serde_json::json!({
            "key_label": signature.signing_key_id,
            "algorithm": match signature.algorithm {
                HsmSigningAlgorithm::EcdsaP256 => "ECDSA-P256",
                HsmSigningAlgorithm::EcdsaP384 => "ECDSA-P384",
                HsmSigningAlgorithm::Ed25519 => "ED25519",
                HsmSigningAlgorithm::Rsa2048 => "RSA-2048",
                HsmSigningAlgorithm::Pkcs11Hsm => "PKCS11-HSM",
            },
            "payload_hex": hex::encode(payload),
            "signature_hex": signature.signature_hex,
        });

        let response = self
            .client
            .post(&format!("{}/api/v1/verify", self.config.hsm_url))
            .header("X-API-Key", &self.config.api_key)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("HSM verification request failed: {}", e))?;

        let result: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("HSM verification response parse failed: {}", e))?;

        Ok(result["verified"].as_bool().unwrap_or(false))
    }

    /// Retrieves the public key associated with the configured signing key.
    #[instrument(skip(self))]
    pub async fn get_public_key(&self) -> Result<String, String> {
        let response = self
            .client
            .get(&format!(
                "{}/api/v1/keys/{}",
                self.config.hsm_url, self.config.signing_key_label
            ))
            .header("X-API-Key", &self.config.api_key)
            .send()
            .await
            .map_err(|e| format!("HSM key retrieval failed: {}", e))?;

        let result: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("HSM key response parse failed: {}", e))?;

        result["public_key"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| "Missing public key in HSM response".to_string())
    }

    /// Performs a health check against the HSM service.
    #[instrument(skip(self))]
    pub async fn health_check(&self) -> Result<bool, String> {
        let response = self
            .client
            .get(&format!("{}/api/v1/health", self.config.hsm_url))
            .header("X-API-Key", &self.config.api_key)
            .send()
            .await
            .map_err(|e| format!("HSM health check failed: {}", e))?;

        Ok(response.status().is_success())
    }
}
