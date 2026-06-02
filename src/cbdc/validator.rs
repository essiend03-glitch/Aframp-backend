use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{info, instrument, warn};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ScreeningResult {
    Pass,
    Fail,
    Pending,
    Escalated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SwapValidationReport {
    pub is_valid: bool,
    pub screening_result: ScreeningResult,
    pub screening_id: String,
    pub violations: Vec<String>,
    pub warnings: Vec<String>,
    pub compliance_tags: HashMap<String, String>,
}

/// AML/Compliance payload validator for CBDC cross-rail swaps.
///
/// Ensures all transactions meet strict sovereign AML requirements,
/// compliance metadata tagging, and source tracking requirements before
/// submission to the DLT gateway.
pub struct SwapValidator {
    aml_service_url: Option<String>,
    api_key: Option<String>,
    max_amount: f64,
    restricted_jurisdictions: Vec<String>,
}

impl Default for SwapValidator {
    fn default() -> Self {
        Self {
            aml_service_url: std::env::var("CBDC_AML_SERVICE_URL").ok(),
            api_key: std::env::var("CBDC_AML_API_KEY").ok(),
            max_amount: std::env::var("CBDC_MAX_SWAP_AMOUNT")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(1_000_000.0),
            restricted_jurisdictions: std::env::var("CBDC_RESTRICTED_JURISDICTIONS")
                .unwrap_or_default()
                .split(',')
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect(),
        }
    }
}

impl SwapValidator {
    pub fn new() -> Self {
        Self::default()
    }

    #[instrument(skip(self, payload))]
    pub async fn validate(&self, payload: &serde_json::Value) -> SwapValidationReport {
        let mut violations = Vec::new();
        let mut warnings = Vec::new();
        let mut compliance_tags = HashMap::new();

        // 1. Validate amount bounds
        if let Some(amount) = payload.get("amount").and_then(|v| v.as_f64()) {
            if amount <= 0.0 {
                violations.push("Amount must be positive".to_string());
            }
            if amount > self.max_amount {
                violations.push(format!(
                    "Amount ({}) exceeds maximum allowed ({})",
                    amount, self.max_amount
                ));
            }
        } else {
            violations.push("Amount is required and must be numeric".to_string());
        }

        // 2. Validate sender/recipient presence
        if payload.get("sender").and_then(|v| v.as_str()).unwrap_or("").is_empty() {
            violations.push("Sender is required".to_string());
        }
        if payload.get("recipient").and_then(|v| v.as_str()).unwrap_or("").is_empty() {
            violations.push("Recipient is required".to_string());
        }

        // 3. Validate jurisdiction/region
        if let Some(region) = payload.get("jurisdiction").and_then(|v| v.as_str()) {
            let region_lower = region.to_lowercase();
            if self.restricted_jurisdictions.contains(&region_lower) {
                violations.push(format!(
                    "Transactions from jurisdiction '{}' are restricted",
                    region
                ));
            }
            compliance_tags.insert("jurisdiction".to_string(), region.to_string());
        } else {
            warnings.push("No jurisdiction specified — will apply default compliance rules".to_string());
        }

        // 4. Validate compliance metadata tags
        if let Some(meta) = payload.get("compliance_metadata") {
            if let Some(obj) = meta.as_object() {
                for (key, value) in obj {
                    compliance_tags.insert(key.clone(), value.to_string());
                }
            }
        }

        if !compliance_tags.contains_key("purpose") {
            warnings.push("No compliance 'purpose' tag — transaction flagged for review".to_string());
        }
        if !compliance_tags.contains_key("source_of_funds") {
            warnings.push("No 'source_of_funds' tag — transaction flagged for review".to_string());
        }

        // 5. AML screening (if external service configured)
        let (screening_result, screening_id) = if self.aml_service_url.is_some() {
            match self.screen_with_aml_service(payload).await {
                Ok(result) => {
                    compliance_tags.insert("aml_screening_result".to_string(), format!("{:?}", result));
                    result
                }
                Err(e) => {
                    warn!(error = %e, "AML screening failed — defaulting to pending");
                    warnings.push(format!("AML screening service unavailable: {}", e));
                    (ScreeningResult::Pending, "aml-unavailable".to_string())
                }
            }
        } else {
            (ScreeningResult::Pending, "no-aml-service".to_string())
        };

        let is_valid = violations.is_empty();

        if is_valid {
            info!(
                screening_id = %screening_id,
                screening_result = ?screening_result,
                "Swap payload validation passed"
            );
        } else {
            warn!(
                violations = ?violations,
                "Swap payload validation failed"
            );
        }

        SwapValidationReport {
            is_valid,
            screening_result,
            screening_id,
            violations,
            warnings,
            compliance_tags,
        }
    }

    async fn screen_with_aml_service(
        &self,
        payload: &serde_json::Value,
    ) -> Result<(ScreeningResult, String), String> {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

        let url = format!(
            "{}/api/v1/screen",
            self.aml_service_url.as_ref().unwrap()
        );

        let response = client
            .post(&url)
            .header("X-API-Key", self.api_key.as_deref().unwrap_or(""))
            .json(payload)
            .send()
            .await
            .map_err(|e| format!("AML screening request failed: {}", e))?;

        let result: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("AML response parse failed: {}", e))?;

        let screening_id = result["screening_id"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();

        let verdict = result["verdict"].as_str().unwrap_or("pending");
        let screening_result = match verdict {
            "pass" | "clear" => ScreeningResult::Pass,
            "fail" | "blocked" => ScreeningResult::Fail,
            "escalated" | "manual_review" => ScreeningResult::Escalated,
            _ => ScreeningResult::Pending,
        };

        Ok((screening_result, screening_id))
    }
}
