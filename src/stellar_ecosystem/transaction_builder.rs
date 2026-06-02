//! Stellar transaction builder for atomic multi-operation transactions (Issue #470).
//! Constructs PathPaymentStrictReceive and ManageBuyOffer operations.

#[cfg(feature = "database")]
use anyhow::{anyhow, Context, Result};
#[cfg(feature = "database")]
use base64::{engine::general_purpose::STANDARD, Engine};
#[cfg(feature = "database")]
use rust_decimal::prelude::*;
#[cfg(feature = "database")]
use tracing::instrument;

/// Stellar amount precision: 7 decimal places (1 stroop = 0.0000001 XLM).
pub const STELLAR_DECIMAL_PLACES: u32 = 7;
/// 1 stroop in Decimal.
pub const ONE_STROOP: &str = "0.0000001";

/// A built transaction ready for submission.
#[cfg(feature = "database")]
#[derive(Debug, Clone)]
pub struct BuiltTransaction {
    /// Base64-encoded XDR of the TransactionEnvelope.
    pub xdr_base64: String,
    /// Human-readable summary of operations included.
    pub operations: Vec<String>,
}

/// Builder for Stellar transactions.
#[cfg(feature = "database")]
#[derive(Debug, Default)]
pub struct StellarTransactionBuilder {
    source_account: String,
    sequence_number: i64,
    operations: Vec<StellarOperation>,
    base_fee: u32,
    network_passphrase: String,
}

#[cfg(feature = "database")]
#[derive(Debug, Clone)]
enum StellarOperation {
    PathPaymentStrictReceive {
        send_asset: String,
        send_max: Decimal,
        destination: String,
        dest_asset: String,
        dest_amount: Decimal,
        path: Vec<String>,
    },
    ManageBuyOffer {
        selling: String,
        buying: String,
        buy_amount: Decimal,
        price_n: i64,
        price_d: i64,
        offer_id: i64,
    },
}

#[cfg(feature = "database")]
impl StellarTransactionBuilder {
    pub fn new(
        source_account: impl Into<String>,
        sequence_number: i64,
        network_passphrase: impl Into<String>,
    ) -> Self {
        Self {
            source_account: source_account.into(),
            sequence_number,
            operations: Vec::new(),
            base_fee: 100, // 100 stroops
            network_passphrase: network_passphrase.into(),
        }
    }

    /// Add a PathPaymentStrictReceive operation.
    /// `send_max` is the maximum source asset to spend (slippage guard applied here).
    #[instrument(skip(self))]
    pub fn add_path_payment_strict_receive(
        mut self,
        send_asset: impl Into<String>,
        send_max: Decimal,
        destination: impl Into<String>,
        dest_asset: impl Into<String>,
        dest_amount: Decimal,
        path: Vec<String>,
    ) -> Result<Self> {
        validate_stellar_amount(send_max, "send_max")?;
        validate_stellar_amount(dest_amount, "dest_amount")?;
        self.operations.push(StellarOperation::PathPaymentStrictReceive {
            send_asset: send_asset.into(),
            send_max,
            destination: destination.into(),
            dest_asset: dest_asset.into(),
            dest_amount,
            path,
        });
        Ok(self)
    }

    /// Add a ManageBuyOffer operation.
    pub fn add_manage_buy_offer(
        mut self,
        selling: impl Into<String>,
        buying: impl Into<String>,
        buy_amount: Decimal,
        price_n: i64,
        price_d: i64,
        offer_id: i64,
    ) -> Result<Self> {
        validate_stellar_amount(buy_amount, "buy_amount")?;
        self.operations.push(StellarOperation::ManageBuyOffer {
            selling: selling.into(),
            buying: buying.into(),
            buy_amount,
            price_n,
            price_d,
            offer_id,
        });
        Ok(self)
    }

    /// Build the transaction. Returns a `BuiltTransaction` with the XDR envelope.
    #[instrument(skip(self), fields(source = %self.source_account, ops = self.operations.len()))]
    pub fn build(self) -> Result<BuiltTransaction> {
        if self.operations.is_empty() {
            return Err(anyhow!("transaction must have at least one operation"));
        }

        // Construct a minimal XDR representation.
        // In production this would use stellar-xdr to build a proper TransactionEnvelope.
        // Here we produce a deterministic JSON-encoded envelope that is base64-encoded,
        // matching the pattern used elsewhere in this codebase (xdr_parser.rs).
        let envelope = serde_json::json!({
            "source_account": self.source_account,
            "sequence_number": self.sequence_number,
            "fee": self.base_fee * self.operations.len() as u32,
            "network_passphrase": self.network_passphrase,
            "operations": self.operations.iter().map(op_to_json).collect::<Vec<_>>(),
        });

        let xdr_base64 = STANDARD.encode(envelope.to_string().as_bytes());
        let op_summaries = self.operations.iter().map(op_summary).collect();

        tracing::info!(
            source = %self.source_account,
            op_count = self.operations.len(),
            "Stellar transaction built"
        );

        Ok(BuiltTransaction {
            xdr_base64,
            operations: op_summaries,
        })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Validate that an amount has at most 7 decimal places (Stellar precision).
#[cfg(feature = "database")]
pub fn validate_stellar_amount(amount: Decimal, field: &str) -> Result<()> {
    if amount <= Decimal::ZERO {
        return Err(anyhow!("{} must be positive", field));
    }
    // Stellar amounts must not exceed 7 decimal places
    let scaled = amount.round_dp(STELLAR_DECIMAL_PLACES);
    if scaled != amount {
        return Err(anyhow!(
            "{} has more than {} decimal places",
            field,
            STELLAR_DECIMAL_PLACES
        ));
    }
    Ok(())
}

/// Round a Decimal to Stellar's 7 decimal places.
#[cfg(feature = "database")]
pub fn to_stellar_precision(amount: Decimal) -> Decimal {
    amount.round_dp(STELLAR_DECIMAL_PLACES)
}

/// Apply a slippage buffer to a send_max amount.
/// `send_max = dest_amount * (1 + slippage_fraction)` rounded to 7dp.
#[cfg(feature = "database")]
pub fn apply_slippage_buffer(base_amount: Decimal, slippage: Decimal) -> Decimal {
    to_stellar_precision(base_amount * (Decimal::ONE + slippage))
}

#[cfg(feature = "database")]
fn op_to_json(op: &StellarOperation) -> serde_json::Value {
    match op {
        StellarOperation::PathPaymentStrictReceive {
            send_asset,
            send_max,
            destination,
            dest_asset,
            dest_amount,
            path,
        } => serde_json::json!({
            "type": "path_payment_strict_receive",
            "send_asset": send_asset,
            "send_max": send_max.to_string(),
            "destination": destination,
            "dest_asset": dest_asset,
            "dest_amount": dest_amount.to_string(),
            "path": path,
        }),
        StellarOperation::ManageBuyOffer {
            selling,
            buying,
            buy_amount,
            price_n,
            price_d,
            offer_id,
        } => serde_json::json!({
            "type": "manage_buy_offer",
            "selling": selling,
            "buying": buying,
            "buy_amount": buy_amount.to_string(),
            "price": { "n": price_n, "d": price_d },
            "offer_id": offer_id,
        }),
    }
}

#[cfg(feature = "database")]
fn op_summary(op: &StellarOperation) -> String {
    match op {
        StellarOperation::PathPaymentStrictReceive {
            send_asset,
            dest_asset,
            dest_amount,
            ..
        } => format!(
            "PathPaymentStrictReceive {} → {} {}",
            send_asset, dest_amount, dest_asset
        ),
        StellarOperation::ManageBuyOffer {
            buying,
            buy_amount,
            ..
        } => format!("ManageBuyOffer {} {}", buy_amount, buying),
    }
}
