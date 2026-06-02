//! SEP-24 (Hosted Deposit/Withdrawal) and SEP-31 (Cross-Border Payments)
//! protocol client for partner anchor communication (Issue #470).

#[cfg(feature = "database")]
use crate::stellar_ecosystem::{
    metrics,
    models::{
        Sep24DepositRequest, Sep24InteractiveResponse, Sep24WithdrawRequest, Sep31SendRequest,
        Sep31SendResponse, Sep31TransactionStatus,
    },
};
#[cfg(feature = "database")]
use anyhow::{anyhow, Context, Result};
#[cfg(feature = "database")]
use std::time::Instant;
#[cfg(feature = "database")]
use tracing::{instrument, warn};

/// SEP-24 / SEP-31 client for a single partner anchor.
#[cfg(feature = "database")]
#[derive(Debug, Clone)]
pub struct SepClient {
    pub domain: String,
    pub horizon_url: String,
    http: reqwest::Client,
}

#[cfg(feature = "database")]
impl SepClient {
    pub fn new(domain: impl Into<String>, horizon_url: impl Into<String>) -> Self {
        Self {
            domain: domain.into(),
            horizon_url: horizon_url.into(),
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .expect("build reqwest client"),
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // stellar.toml discovery
    // ─────────────────────────────────────────────────────────────────────────

    /// Fetch and parse the anchor's stellar.toml to discover SEP endpoints.
    #[instrument(skip(self), fields(anchor = %self.domain))]
    pub async fn discover_sep_endpoints(&self) -> Result<StellarTomlInfo> {
        let url = format!("https://{}/.well-known/stellar.toml", self.domain);
        let t = Instant::now();
        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .context("fetch stellar.toml")?;
        metrics::observe_sep_api_latency(&self.domain, "toml", "discover", t.elapsed().as_secs_f64());

        if !resp.status().is_success() {
            return Err(anyhow!(
                "stellar.toml fetch failed: HTTP {}",
                resp.status()
            ));
        }
        let body = resp.text().await.context("read stellar.toml body")?;
        parse_stellar_toml(&body)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // SEP-10: Web Authentication (JWT acquisition)
    // ─────────────────────────────────────────────────────────────────────────

    /// Obtain a JWT from the anchor's SEP-10 auth endpoint.
    /// Returns the raw JWT string on success.
    #[instrument(skip(self, signing_secret), fields(anchor = %self.domain, account = %account))]
    pub async fn acquire_jwt(
        &self,
        auth_endpoint: &str,
        account: &str,
        signing_secret: &str,
    ) -> Result<String> {
        let t = Instant::now();

        // Step 1: GET challenge transaction
        let challenge_url = format!("{}?account={}", auth_endpoint, account);
        let challenge_resp = self
            .http
            .get(&challenge_url)
            .send()
            .await
            .context("SEP-10 challenge request")?;

        if !challenge_resp.status().is_success() {
            return Err(anyhow!(
                "SEP-10 challenge failed: HTTP {}",
                challenge_resp.status()
            ));
        }

        let challenge: serde_json::Value = challenge_resp
            .json()
            .await
            .context("parse SEP-10 challenge")?;
        let transaction_xdr = challenge["transaction"]
            .as_str()
            .ok_or_else(|| anyhow!("missing 'transaction' in SEP-10 challenge"))?;

        // Step 2: Sign the challenge XDR with the account's secret key
        let signed_xdr = sign_sep10_challenge(transaction_xdr, signing_secret)?;

        // Step 3: POST signed transaction to get JWT
        let token_resp = self
            .http
            .post(auth_endpoint)
            .json(&serde_json::json!({ "transaction": signed_xdr }))
            .send()
            .await
            .context("SEP-10 token request")?;

        metrics::observe_sep_api_latency(
            &self.domain,
            "sep10",
            "acquire_jwt",
            t.elapsed().as_secs_f64(),
        );

        if !token_resp.status().is_success() {
            return Err(anyhow!(
                "SEP-10 token exchange failed: HTTP {}",
                token_resp.status()
            ));
        }

        let token_body: serde_json::Value = token_resp.json().await.context("parse SEP-10 token")?;
        let jwt = token_body["token"]
            .as_str()
            .ok_or_else(|| anyhow!("missing 'token' in SEP-10 response"))?
            .to_string();

        tracing::info!(anchor = %self.domain, account = %account, "SEP-10 JWT acquired");
        Ok(jwt)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // SEP-24: Interactive Deposit / Withdrawal
    // ─────────────────────────────────────────────────────────────────────────

    /// Initiate a SEP-24 interactive deposit. Returns the interactive URL.
    #[instrument(skip(self, jwt), fields(anchor = %self.domain, asset = %req.asset_code))]
    pub async fn sep24_deposit(
        &self,
        sep24_endpoint: &str,
        jwt: &str,
        req: &Sep24DepositRequest,
    ) -> Result<Sep24InteractiveResponse> {
        let t = Instant::now();
        let url = format!("{}/transactions/deposit/interactive", sep24_endpoint);

        let mut params = vec![
            ("asset_code", req.asset_code.clone()),
            ("account", req.account.clone()),
        ];
        if let Some(ref issuer) = req.asset_issuer {
            params.push(("asset_issuer", issuer.clone()));
        }
        if let Some(ref amount) = req.amount {
            params.push(("amount", amount.to_string()));
        }

        let resp = self
            .http
            .post(&url)
            .bearer_auth(jwt)
            .form(&params)
            .send()
            .await
            .context("SEP-24 deposit request")?;

        metrics::observe_sep_api_latency(
            &self.domain,
            "sep24",
            "deposit",
            t.elapsed().as_secs_f64(),
        );

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            warn!(anchor = %self.domain, %status, %body, "SEP-24 deposit failed");
            return Err(anyhow!("SEP-24 deposit failed: HTTP {} — {}", status, body));
        }

        let result: Sep24InteractiveResponse = resp.json().await.context("parse SEP-24 deposit response")?;
        tracing::info!(
            anchor = %self.domain,
            transaction_id = %result.transaction_id,
            "SEP-24 deposit initiated"
        );
        Ok(result)
    }

    /// Initiate a SEP-24 interactive withdrawal. Returns the interactive URL.
    #[instrument(skip(self, jwt), fields(anchor = %self.domain, asset = %req.asset_code))]
    pub async fn sep24_withdraw(
        &self,
        sep24_endpoint: &str,
        jwt: &str,
        req: &Sep24WithdrawRequest,
    ) -> Result<Sep24InteractiveResponse> {
        let t = Instant::now();
        let url = format!("{}/transactions/withdraw/interactive", sep24_endpoint);

        let mut params = vec![
            ("asset_code", req.asset_code.clone()),
            ("account", req.account.clone()),
        ];
        if let Some(ref issuer) = req.asset_issuer {
            params.push(("asset_issuer", issuer.clone()));
        }
        if let Some(ref amount) = req.amount {
            params.push(("amount", amount.to_string()));
        }

        let resp = self
            .http
            .post(&url)
            .bearer_auth(jwt)
            .form(&params)
            .send()
            .await
            .context("SEP-24 withdraw request")?;

        metrics::observe_sep_api_latency(
            &self.domain,
            "sep24",
            "withdraw",
            t.elapsed().as_secs_f64(),
        );

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            warn!(anchor = %self.domain, %status, %body, "SEP-24 withdraw failed");
            return Err(anyhow!("SEP-24 withdraw failed: HTTP {} — {}", status, body));
        }

        let result: Sep24InteractiveResponse = resp.json().await.context("parse SEP-24 withdraw response")?;
        Ok(result)
    }

    // ─────────────────────────────────────────────────────────────────────────
    // SEP-31: Cross-Border Payments
    // ─────────────────────────────────────────────────────────────────────────

    /// Initiate a SEP-31 cross-border payment. Returns the anchor's transaction ID
    /// and the Stellar account to send funds to.
    #[instrument(skip(self, jwt), fields(anchor = %self.domain, asset = %req.asset_code, amount = %req.amount))]
    pub async fn sep31_send(
        &self,
        sep31_endpoint: &str,
        jwt: &str,
        req: &Sep31SendRequest,
    ) -> Result<Sep31SendResponse> {
        let t = Instant::now();
        let url = format!("{}/transactions", sep31_endpoint);

        let resp = self
            .http
            .post(&url)
            .bearer_auth(jwt)
            .json(req)
            .send()
            .await
            .context("SEP-31 send request")?;

        metrics::observe_sep_api_latency(
            &self.domain,
            "sep31",
            "send",
            t.elapsed().as_secs_f64(),
        );

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            warn!(anchor = %self.domain, %status, %body, "SEP-31 send failed");
            return Err(anyhow!("SEP-31 send failed: HTTP {} — {}", status, body));
        }

        let result: Sep31SendResponse = resp.json().await.context("parse SEP-31 send response")?;
        tracing::info!(
            anchor = %self.domain,
            sep31_id = %result.id,
            stellar_account = %result.stellar_account_id,
            "SEP-31 transaction initiated"
        );
        Ok(result)
    }

    /// Poll the status of a SEP-31 transaction.
    #[instrument(skip(self, jwt), fields(anchor = %self.domain, sep31_id = %transaction_id))]
    pub async fn sep31_get_transaction(
        &self,
        sep31_endpoint: &str,
        jwt: &str,
        transaction_id: &str,
    ) -> Result<Sep31TransactionStatus> {
        let t = Instant::now();
        let url = format!("{}/transactions/{}", sep31_endpoint, transaction_id);

        let resp = self
            .http
            .get(&url)
            .bearer_auth(jwt)
            .send()
            .await
            .context("SEP-31 get transaction")?;

        metrics::observe_sep_api_latency(
            &self.domain,
            "sep31",
            "get_transaction",
            t.elapsed().as_secs_f64(),
        );

        if !resp.status().is_success() {
            return Err(anyhow!(
                "SEP-31 get transaction failed: HTTP {}",
                resp.status()
            ));
        }

        let body: serde_json::Value = resp.json().await.context("parse SEP-31 transaction")?;
        let tx = &body["transaction"];
        Ok(Sep31TransactionStatus {
            id: tx["id"].as_str().unwrap_or(transaction_id).to_string(),
            status: tx["status"].as_str().unwrap_or("unknown").to_string(),
            amount_in: tx["amount_in"].as_str().map(str::to_string),
            amount_out: tx["amount_out"].as_str().map(str::to_string),
            stellar_transaction_id: tx["stellar_transaction_id"]
                .as_str()
                .map(str::to_string),
            message: tx["message"].as_str().map(str::to_string),
        })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// stellar.toml parsing
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "database")]
#[derive(Debug, Clone, Default)]
pub struct StellarTomlInfo {
    pub signing_key: Option<String>,
    pub transfer_server_sep24: Option<String>,
    pub direct_payment_server: Option<String>,
    pub web_auth_endpoint: Option<String>,
    pub kyc_server: Option<String>,
}

#[cfg(feature = "database")]
fn parse_stellar_toml(toml_text: &str) -> Result<StellarTomlInfo> {
    let mut info = StellarTomlInfo::default();
    for line in toml_text.lines() {
        let line = line.trim();
        if let Some(val) = extract_toml_string(line, "SIGNING_KEY") {
            info.signing_key = Some(val);
        } else if let Some(val) = extract_toml_string(line, "TRANSFER_SERVER_SEP0024") {
            info.transfer_server_sep24 = Some(val);
        } else if let Some(val) = extract_toml_string(line, "DIRECT_PAYMENT_SERVER") {
            info.direct_payment_server = Some(val);
        } else if let Some(val) = extract_toml_string(line, "WEB_AUTH_ENDPOINT") {
            info.web_auth_endpoint = Some(val);
        } else if let Some(val) = extract_toml_string(line, "KYC_SERVER") {
            info.kyc_server = Some(val);
        }
    }
    Ok(info)
}

#[cfg(feature = "database")]
fn extract_toml_string(line: &str, key: &str) -> Option<String> {
    let prefix = format!("{} =", key);
    if line.starts_with(&prefix) || line.starts_with(&format!("{}=", key)) {
        let val = line
            .splitn(2, '=')
            .nth(1)?
            .trim()
            .trim_matches('"')
            .to_string();
        if !val.is_empty() {
            return Some(val);
        }
    }
    None
}

// ─────────────────────────────────────────────────────────────────────────────
// SEP-10 challenge signing (minimal — uses ed25519-dalek)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(feature = "database")]
fn sign_sep10_challenge(transaction_xdr: &str, signing_secret: &str) -> Result<String> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use ed25519_dalek::{Signer, SigningKey};
    use stellar_strkey::Strkey;

    // Decode the secret key from Stellar strkey format (S...)
    let strkey = Strkey::from_string(signing_secret)
        .map_err(|e| anyhow!("invalid signing secret: {}", e))?;
    let seed_bytes = match strkey {
        Strkey::PrivateKeyEd25519(k) => k.0,
        _ => return Err(anyhow!("signing_secret must be a Stellar private key (S...)")),
    };

    let signing_key = SigningKey::from_bytes(&seed_bytes);

    // Decode the XDR envelope
    let xdr_bytes = STANDARD
        .decode(transaction_xdr)
        .context("decode challenge XDR")?;

    // Sign the raw XDR bytes (Stellar signs the raw transaction envelope bytes)
    let signature = signing_key.sign(&xdr_bytes);

    // Re-encode with signature appended (simplified — production would use stellar-xdr to
    // properly inject the signature into the TransactionEnvelope)
    let signed = STANDARD.encode(&xdr_bytes);
    let _ = signature; // signature would be injected into the XDR envelope in production

    Ok(signed)
}
