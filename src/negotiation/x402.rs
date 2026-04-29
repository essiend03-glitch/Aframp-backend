/// Verifies that an x402 micro-payment was made before a negotiation session
/// is opened, preventing negotiation spam / DoS.
///
/// In production this calls the Stellar Horizon API (or a local cache) to
/// confirm the transaction exists, targets the platform's fee account, and
/// meets the minimum amount.  The stub below is intentionally thin so the
/// surrounding logic can be tested without a live network.
pub struct X402EntranceFee {
    /// Stellar account that must receive the fee.
    pub fee_account: String,
    /// Minimum fee in stroops (1 XLM = 10_000_000 stroops).
    pub min_amount_stroops: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum FeeError {
    #[error("payment reference is empty")]
    EmptyRef,
    #[error("payment not found or insufficient: {0}")]
    NotVerified(String),
}

impl X402EntranceFee {
    pub fn new(fee_account: impl Into<String>, min_amount_stroops: i64) -> Self {
        Self {
            fee_account: fee_account.into(),
            min_amount_stroops,
        }
    }

    /// Returns `Ok(())` when the payment reference is valid.
    ///
    /// Replace the body with a real Horizon lookup in production.
    pub async fn verify(&self, payment_ref: &str) -> Result<(), FeeError> {
        if payment_ref.is_empty() {
            return Err(FeeError::EmptyRef);
        }
        // TODO: query Horizon /transactions/{payment_ref} and assert
        //   - memo or destination == self.fee_account
        //   - amount >= self.min_amount_stroops
        Ok(())
    }
}
