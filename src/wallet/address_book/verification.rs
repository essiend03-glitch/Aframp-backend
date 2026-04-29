use super::models::*;
use regex::Regex;
use std::sync::Arc;
use uuid::Uuid;

/// Stellar address verification service
pub struct StellarAddressVerifier {
    horizon_url: String,
    cngn_issuer: String,
}

impl StellarAddressVerifier {
    pub fn new(horizon_url: String, cngn_issuer: String) -> Self {
        Self {
            horizon_url,
            cngn_issuer,
        }
    }

    /// Validate Stellar public key format
    pub fn validate_public_key_format(&self, public_key: &str) -> bool {
        // Stellar public keys start with 'G' and are 56 characters long
        if public_key.len() != 56 || !public_key.starts_with('G') {
            return false;
        }

        // Check if it contains only valid base32 characters
        public_key.chars().all(|c| {
            c.is_ascii_uppercase() || c.is_ascii_digit()
        })
    }

    /// Verify Stellar account against Horizon API
    pub async fn verify_account(
        &self,
        public_key: &str,
    ) -> Result<VerificationResult, Box<dyn std::error::Error>> {
        if !self.validate_public_key_format(public_key) {
            return Ok(VerificationResult {
                success: false,
                verification_status: VerificationStatus::Failed,
                message: Some("Invalid Stellar public key format".to_string()),
                verified_account_name: None,
                warnings: vec![],
            });
        }

        let client = reqwest::Client::new();
        let url = format!("{}/accounts/{}", self.horizon_url, public_key);

        let response = client.get(&url).send().await;

        match response {
            Ok(resp) if resp.status().is_success() => {
                let account_data: serde_json::Value = resp.json().await?;
                
                // Check for cNGN trustline
                let balances = account_data["balances"].as_array();
                let mut cngn_trustline_active = false;
                let mut warnings = Vec::new();

                if let Some(balances_array) = balances {
                    for balance in balances_array {
                        if balance["asset_code"].as_str() == Some("cNGN")
                            && balance["asset_issuer"].as_str() == Some(&self.cngn_issuer)
                        {
                            // Check if trustline is authorized
                            let is_authorized = balance["is_authorized"].as_bool().unwrap_or(true);
                            if is_authorized {
                                cngn_trustline_active = true;
                            } else {
                                warnings.push("cNGN trustline exists but is not authorized".to_string());
                            }
                            break;
                        }
                    }
                }

                if !cngn_trustline_active {
                    warnings.push(
                        "Account does not have an active cNGN trustline. cNGN cannot be sent until trustline is established.".to_string()
                    );
                }

                Ok(VerificationResult {
                    success: true,
                    verification_status: if cngn_trustline_active {
                        VerificationStatus::Verified
                    } else {
                        VerificationStatus::Pending
                    },
                    message: Some("Account verified on Stellar network".to_string()),
                    verified_account_name: None,
                    warnings,
                })
            }
            Ok(resp) if resp.status().as_u16() == 404 => {
                Ok(VerificationResult {
                    success: false,
                    verification_status: VerificationStatus::Pending,
                    message: Some("Account does not exist on Stellar network yet".to_string()),
                    verified_account_name: None,
                    warnings: vec![
                        "Account must be funded with XLM before it can receive cNGN".to_string()
                    ],
                })
            }
            Ok(resp) => {
                Ok(VerificationResult {
                    success: false,
                    verification_status: VerificationStatus::Failed,
                    message: Some(format!("Horizon API error: {}", resp.status())),
                    verified_account_name: None,
                    warnings: vec![],
                })
            }
            Err(e) => {
                Ok(VerificationResult {
                    success: false,
                    verification_status: VerificationStatus::Failed,
                    message: Some(format!("Failed to connect to Horizon: {}", e)),
                    verified_account_name: None,
                    warnings: vec![],
                })
            }
        }
    }
}

/// Mobile money verification service
pub struct MobileMoneyVerifier;

impl MobileMoneyVerifier {
    pub fn new() -> Self {
        Self
    }

    /// Validate phone number format
    pub fn validate_phone_number(&self, phone_number: &str, country_code: &str) -> Result<bool, String> {
        // Remove common formatting characters
        let cleaned = phone_number.replace(&[' ', '-', '(', ')'][..], "");

        // Basic validation - must start with + or digits
        if !cleaned.starts_with('+') && !cleaned.chars().all(|c| c.is_ascii_digit()) {
            return Err("Phone number must contain only digits and optional + prefix".to_string());
        }

        // Country-specific validation
        match country_code.to_uppercase().as_str() {
            "NG" => {
                // Nigerian numbers: +234 followed by 10 digits or 0 followed by 10 digits
                let pattern = Regex::new(r"^(\+234|0)[789]\d{9}$").unwrap();
                if pattern.is_match(&cleaned) {
                    Ok(true)
                } else {
                    Err("Invalid Nigerian phone number format. Expected +234XXXXXXXXXX or 0XXXXXXXXXX".to_string())
                }
            }
            "KE" => {
                // Kenyan numbers: +254 followed by 9 digits or 0 followed by 9 digits
                let pattern = Regex::new(r"^(\+254|0)[17]\d{8}$").unwrap();
                if pattern.is_match(&cleaned) {
                    Ok(true)
                } else {
                    Err("Invalid Kenyan phone number format. Expected +254XXXXXXXXX or 0XXXXXXXXX".to_string())
                }
            }
            "GH" => {
                // Ghanaian numbers: +233 followed by 9 digits or 0 followed by 9 digits
                let pattern = Regex::new(r"^(\+233|0)[2-5]\d{8}$").unwrap();
                if pattern.is_match(&cleaned) {
                    Ok(true)
                } else {
                    Err("Invalid Ghanaian phone number format. Expected +233XXXXXXXXX or 0XXXXXXXXX".to_string())
                }
            }
            _ => {
                // Generic validation for other countries
                if cleaned.len() >= 10 && cleaned.len() <= 15 {
                    Ok(true)
                } else {
                    Err("Phone number must be between 10 and 15 digits".to_string())
                }
            }
        }
    }

    /// Verify mobile money account (placeholder for provider integration)
    pub async fn verify_account(
        &self,
        provider_name: &str,
        phone_number: &str,
        country_code: &str,
    ) -> Result<VerificationResult, Box<dyn std::error::Error>> {
        // Validate phone number format first
        match self.validate_phone_number(phone_number, country_code) {
            Ok(_) => {}
            Err(e) => {
                return Ok(VerificationResult {
                    success: false,
                    verification_status: VerificationStatus::Failed,
                    message: Some(e),
                    verified_account_name: None,
                    warnings: vec![],
                });
            }
        }

        // TODO: Integrate with actual mobile money provider APIs for account name lookup
        // For now, return not supported status
        Ok(VerificationResult {
            success: true,
            verification_status: VerificationStatus::NotSupported,
            message: Some(format!(
                "Phone number format validated. Account name lookup not supported for {}",
                provider_name
            )),
            verified_account_name: None,
            warnings: vec![
                "Account name could not be verified. Please ensure the phone number is correct.".to_string()
            ],
        })
    }
}

/// Bank account verification service
pub struct BankAccountVerifier;

impl BankAccountVerifier {
    pub fn new() -> Self {
        Self
    }

    /// Validate bank account number format
    pub fn validate_account_number(
        &self,
        account_number: &str,
        country_code: &str,
    ) -> Result<bool, String> {
        // Remove spaces and dashes
        let cleaned = account_number.replace(&[' ', '-'][..], "");

        // Must contain only digits
        if !cleaned.chars().all(|c| c.is_ascii_digit()) {
            return Err("Account number must contain only digits".to_string());
        }

        // Country-specific validation
        match country_code.to_uppercase().as_str() {
            "NG" => {
                // Nigerian account numbers are typically 10 digits
                if cleaned.len() == 10 {
                    Ok(true)
                } else {
                    Err("Nigerian bank account numbers must be 10 digits".to_string())
                }
            }
            "KE" => {
                // Kenyan account numbers vary, typically 10-13 digits
                if cleaned.len() >= 10 && cleaned.len() <= 13 {
                    Ok(true)
                } else {
                    Err("Kenyan bank account numbers must be 10-13 digits".to_string())
                }
            }
            "GH" => {
                // Ghanaian account numbers are typically 13 digits
                if cleaned.len() == 13 {
                    Ok(true)
                } else {
                    Err("Ghanaian bank account numbers must be 13 digits".to_string())
                }
            }
            _ => {
                // Generic validation
                if cleaned.len() >= 8 && cleaned.len() <= 20 {
                    Ok(true)
                } else {
                    Err("Account number must be between 8 and 20 digits".to_string())
                }
            }
        }
    }

    /// Verify bank account (placeholder for provider integration)
    pub async fn verify_account(
        &self,
        bank_name: &str,
        account_number: &str,
        country_code: &str,
    ) -> Result<VerificationResult, Box<dyn std::error::Error>> {
        // Validate account number format first
        match self.validate_account_number(account_number, country_code) {
            Ok(_) => {}
            Err(e) => {
                return Ok(VerificationResult {
                    success: false,
                    verification_status: VerificationStatus::Failed,
                    message: Some(e),
                    verified_account_name: None,
                    warnings: vec![],
                });
            }
        }

        // TODO: Integrate with actual payment provider APIs for account name lookup
        // For now, return not supported status
        Ok(VerificationResult {
            success: true,
            verification_status: VerificationStatus::NotSupported,
            message: Some(format!(
                "Account number format validated. Account name lookup not supported for {}",
                bank_name
            )),
            verified_account_name: None,
            warnings: vec![
                "Account name could not be verified. Please ensure the account details are correct.".to_string()
            ],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stellar_public_key_validation() {
        let verifier = StellarAddressVerifier::new(
            "https://horizon-testnet.stellar.org".to_string(),
            "GXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX".to_string(),
        );

        // Valid key
        assert!(verifier.validate_public_key_format("GBRPYHIL2CI3FNQ4BXLFMNDLFJUNPU2HY3ZMFSHONUCEOASW7QC7OX2H"));

        // Invalid - too short
        assert!(!verifier.validate_public_key_format("GBRPYHIL2CI3FNQ4BXLFMNDLFJUNPU2HY3ZMFSHONUCEOASW7QC7OX"));

        // Invalid - doesn't start with G
        assert!(!verifier.validate_public_key_format("SBRPYHIL2CI3FNQ4BXLFMNDLFJUNPU2HY3ZMFSHONUCEOASW7QC7OX2H"));

        // Invalid - contains lowercase
        assert!(!verifier.validate_public_key_format("GBRPYHIl2CI3FNQ4BXLFMNDLFJUNPU2HY3ZMFSHONUCEOASW7QC7OX2H"));
    }

    #[test]
    fn test_nigerian_phone_validation() {
        let verifier = MobileMoneyVerifier::new();

        // Valid formats
        assert!(verifier.validate_phone_number("+2348012345678", "NG").is_ok());
        assert!(verifier.validate_phone_number("08012345678", "NG").is_ok());
        assert!(verifier.validate_phone_number("+2347012345678", "NG").is_ok());
        assert!(verifier.validate_phone_number("+2349012345678", "NG").is_ok());

        // Invalid formats
        assert!(verifier.validate_phone_number("+2346012345678", "NG").is_err()); // Invalid prefix
        assert!(verifier.validate_phone_number("0801234567", "NG").is_err()); // Too short
        assert!(verifier.validate_phone_number("+234801234567", "NG").is_err()); // Too short
    }

    #[test]
    fn test_nigerian_account_number_validation() {
        let verifier = BankAccountVerifier::new();

        // Valid
        assert!(verifier.validate_account_number("0123456789", "NG").is_ok());

        // Invalid - too short
        assert!(verifier.validate_account_number("012345678", "NG").is_err());

        // Invalid - too long
        assert!(verifier.validate_account_number("01234567890", "NG").is_err());

        // Invalid - contains letters
        assert!(verifier.validate_account_number("012345678A", "NG").is_err());
    }
}
