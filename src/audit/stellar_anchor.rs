// Stellar Blockchain Anchoring Service
//
// This module handles the periodic submission of audit log hashes to the
// Stellar blockchain, creating immutable, publicly-verifiable checkpoints.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use stellar_sdk::types::{Asset, Memo, PublicKey};
use stellar_sdk::{KeyPair, Network, Server, TransactionBuilder};
use std::sync::Arc;
use tokio::time::{interval, Duration as TokioDuration};
use tracing::{error, info, warn};
use uuid::Uuid;

use super::ledger::{AnchorPoint, AuditLedger};

/// Configuration for Stellar anchoring
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StellarAnchorConfig {
    /// Stellar Horizon server URL
    pub horizon_url: String,
    
    /// Network passphrase (public or testnet)
    pub network_passphrase: String,
    
    /// Source account secret key for submitting transactions
    pub source_secret: String,
    
    /// Interval between anchor submissions (in seconds)
    pub anchor_interval_seconds: u64,
    
    /// Destination account for anchor transactions (optional)
    pub destination_account: Option<String>,
    
    /// Base fee for transactions (in stroops)
    pub base_fee: u32,
}

impl Default for StellarAnchorConfig {
    fn default() -> Self {
        Self {
            horizon_url: "https://horizon-testnet.stellar.org".to_string(),
            network_passphrase: Network::Testnet.to_string(),
            source_secret: String::new(),
            anchor_interval_seconds: 3600, // 1 hour
            destination_account: None,
            base_fee: 100,
        }
    }
}

/// Stellar anchoring service
pub struct StellarAnchorService {
    config: StellarAnchorConfig,
    audit_ledger: Arc<AuditLedger>,
    pool: PgPool,
}

impl StellarAnchorService {
    /// Create a new Stellar anchoring service
    pub fn new(
        config: StellarAnchorConfig,
        audit_ledger: Arc<AuditLedger>,
        pool: PgPool,
    ) -> Self {
        Self {
            config,
            audit_ledger,
            pool,
        }
    }
    
    /// Start the anchoring service (runs in background)
    pub async fn start(self: Arc<Self>) {
        info!(
            "Starting Stellar anchor service with interval: {}s",
            self.config.anchor_interval_seconds
        );
        
        let mut ticker = interval(TokioDuration::from_secs(
            self.config.anchor_interval_seconds,
        ));
        
        loop {
            ticker.tick().await;
            
            if let Err(e) = self.create_and_submit_anchor().await {
                error!("Failed to create and submit anchor: {}", e);
            }
        }
    }
    
    /// Create an anchor point and submit it to Stellar
    async fn create_and_submit_anchor(&self) -> Result<(), StellarAnchorError> {
        // Check if we need to create a new anchor
        if !self.should_create_anchor().await? {
            info!("Skipping anchor creation - recent anchor exists");
            return Ok(());
        }
        
        // Create anchor point in the database
        let anchor = self.audit_ledger.create_anchor().await.map_err(|e| {
            StellarAnchorError::AuditLedgerError(e.to_string())
        })?;
        
        info!(
            "Created anchor point: id={}, sequence={}, hash={}",
            anchor.id, anchor.sequence, anchor.entry_hash
        );
        
        // Submit to Stellar blockchain
        match self.submit_to_stellar(&anchor).await {
            Ok((tx_id, ledger)) => {
                info!(
                    "Successfully anchored to Stellar: tx={}, ledger={}",
                    tx_id, ledger
                );
                
                // Update anchor with Stellar information
                self.update_anchor_stellar_info(anchor.id, tx_id, ledger)
                    .await?;
            }
            Err(e) => {
                error!("Failed to submit anchor to Stellar: {}", e);
                return Err(e);
            }
        }
        
        Ok(())
    }
    
    /// Check if we should create a new anchor
    async fn should_create_anchor(&self) -> Result<bool, StellarAnchorError> {
        let last_anchor = sqlx::query!(
            r#"
            SELECT anchor_timestamp
            FROM audit_anchors
            ORDER BY anchor_timestamp DESC
            LIMIT 1
            "#
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| StellarAnchorError::Database(e.to_string()))?;
        
        match last_anchor {
            Some(anchor) => {
                let elapsed = Utc::now() - anchor.anchor_timestamp;
                let threshold = Duration::seconds(self.config.anchor_interval_seconds as i64);
                Ok(elapsed >= threshold)
            }
            None => Ok(true), // No anchors exist, create first one
        }
    }
    
    /// Submit anchor hash to Stellar blockchain
    async fn submit_to_stellar(
        &self,
        anchor: &AnchorPoint,
    ) -> Result<(String, i64), StellarAnchorError> {
        // Parse the source keypair
        let source_keypair = KeyPair::from_secret_seed(&self.config.source_secret)
            .map_err(|e| StellarAnchorError::StellarError(format!("Invalid secret key: {}", e)))?;
        
        // Create Stellar server connection
        let server = Server::new(&self.config.horizon_url)
            .map_err(|e| StellarAnchorError::StellarError(e.to_string()))?;
        
        // Get source account
        let source_account = server
            .load_account(&source_keypair.public_key())
            .await
            .map_err(|e| StellarAnchorError::StellarError(e.to_string()))?;
        
        // Determine destination (self if not specified)
        let destination = if let Some(ref dest) = self.config.destination_account {
            PublicKey::from_account_id(dest)
                .map_err(|e| StellarAnchorError::StellarError(e.to_string()))?
        } else {
            source_keypair.public_key()
        };
        
        // Create memo with anchor hash (truncated to 28 bytes for MEMO_HASH)
        let memo = Memo::hash(
            &hex::decode(&anchor.entry_hash[..56])
                .map_err(|e| StellarAnchorError::StellarError(e.to_string()))?,
        );
        
        // Build transaction
        let mut tx_builder = TransactionBuilder::new(
            source_account,
            Network::from_passphrase(&self.config.network_passphrase),
        )
        .base_fee(self.config.base_fee)
        .memo(memo);
        
        // Add payment operation (minimal amount to self or destination)
        tx_builder = tx_builder.add_operation(
            stellar_sdk::operations::Payment::new(
                destination,
                Asset::native(),
                "0.0000001", // Minimal XLM amount
            )
            .build(),
        );
        
        // Build and sign transaction
        let transaction = tx_builder
            .build()
            .map_err(|e| StellarAnchorError::StellarError(e.to_string()))?;
        
        let signed_tx = transaction
            .sign(&source_keypair)
            .map_err(|e| StellarAnchorError::StellarError(e.to_string()))?;
        
        // Submit to network
        let response = server
            .submit_transaction(&signed_tx)
            .await
            .map_err(|e| StellarAnchorError::StellarError(e.to_string()))?;
        
        Ok((response.hash, response.ledger as i64))
    }
    
    /// Update anchor with Stellar transaction information
    async fn update_anchor_stellar_info(
        &self,
        anchor_id: Uuid,
        tx_id: String,
        ledger: i64,
    ) -> Result<(), StellarAnchorError> {
        sqlx::query!(
            r#"
            UPDATE audit_anchors
            SET stellar_transaction_id = $1,
                stellar_ledger = $2,
                verified = TRUE,
                verification_timestamp = NOW()
            WHERE id = $3
            "#,
            tx_id,
            ledger,
            anchor_id
        )
        .execute(&self.pool)
        .await
        .map_err(|e| StellarAnchorError::Database(e.to_string()))?;
        
        Ok(())
    }
    
    /// Verify an anchor against Stellar blockchain
    pub async fn verify_anchor(
        &self,
        anchor_id: Uuid,
    ) -> Result<AnchorVerificationResult, StellarAnchorError> {
        // Fetch anchor from database
        let anchor = sqlx::query!(
            r#"
            SELECT id, sequence, entry_hash, stellar_transaction_id, stellar_ledger
            FROM audit_anchors
            WHERE id = $1
            "#,
            anchor_id
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| StellarAnchorError::Database(e.to_string()))?;
        
        let tx_id = anchor
            .stellar_transaction_id
            .ok_or_else(|| StellarAnchorError::AnchorNotSubmitted)?;
        
        // Query Stellar for the transaction
        let server = Server::new(&self.config.horizon_url)
            .map_err(|e| StellarAnchorError::StellarError(e.to_string()))?;
        
        let tx = server
            .load_transaction(&tx_id)
            .await
            .map_err(|e| StellarAnchorError::StellarError(e.to_string()))?;
        
        // Extract memo and verify hash
        let memo_hash = match tx.memo {
            Memo::Hash(hash) => hex::encode(hash),
            _ => return Err(StellarAnchorError::InvalidMemo),
        };
        
        let expected_hash = &anchor.entry_hash[..56]; // First 28 bytes (56 hex chars)
        let verified = memo_hash == expected_hash;
        
        Ok(AnchorVerificationResult {
            anchor_id: anchor.id,
            sequence: anchor.sequence,
            verified,
            stellar_transaction_id: tx_id,
            stellar_ledger: anchor.stellar_ledger,
            memo_hash,
            expected_hash: expected_hash.to_string(),
        })
    }
}

/// Result of anchor verification
#[derive(Debug, Serialize, Deserialize)]
pub struct AnchorVerificationResult {
    pub anchor_id: Uuid,
    pub sequence: i64,
    pub verified: bool,
    pub stellar_transaction_id: String,
    pub stellar_ledger: Option<i64>,
    pub memo_hash: String,
    pub expected_hash: String,
}

/// Errors that can occur in Stellar anchoring
#[derive(Debug, thiserror::Error)]
pub enum StellarAnchorError {
    #[error("Database error: {0}")]
    Database(String),
    
    #[error("Stellar error: {0}")]
    StellarError(String),
    
    #[error("Audit ledger error: {0}")]
    AuditLedgerError(String),
    
    #[error("Anchor not submitted to Stellar")]
    AnchorNotSubmitted,
    
    #[error("Invalid memo in Stellar transaction")]
    InvalidMemo,
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_default_config() {
        let config = StellarAnchorConfig::default();
        assert_eq!(config.anchor_interval_seconds, 3600);
        assert_eq!(config.base_fee, 100);
    }
}
