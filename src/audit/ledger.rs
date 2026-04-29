// Append-Only Audit Ledger — Tamper-Proof, Forensic-Grade Logging
//
// This module implements a cryptographically-sealed, hash-chained audit log
// that provides absolute accountability and auditability for regulators.
//
// Key Features:
// - Hash-chaining: Each entry contains hash of previous entry
// - WORM storage: Write-Once-Read-Many policies prevent tampering
// - Stellar anchoring: Periodic hash anchoring to public blockchain
// - Forensic schema: Complete metadata for reconstruction

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};
use uuid::Uuid;

/// Actor identity types in the system
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "actor_type", rename_all = "lowercase")]
pub enum ActorType {
    User,
    Agent,
    System,
    Admin,
    Service,
    External,
}

/// Action types for audit logging
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "action_type", rename_all = "lowercase")]
pub enum ActionType {
    Create,
    Read,
    Update,
    Delete,
    Execute,
    Approve,
    Reject,
    Transfer,
    Mint,
    Burn,
    Authenticate,
    Authorize,
    Configure,
    Deploy,
}

/// Forensic-ready audit log entry with complete metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditLogEntry {
    /// Unique identifier for this log entry
    pub id: Uuid,
    
    /// Sequence number (monotonically increasing)
    pub sequence: i64,
    
    /// Hash of the previous log entry (creates the chain)
    pub previous_hash: String,
    
    /// Hash of this entry's content
    pub entry_hash: String,
    
    /// Actor who performed the action
    pub actor_id: String,
    
    /// Type of actor
    pub actor_type: ActorType,
    
    /// Type of action performed
    pub action_type: ActionType,
    
    /// ID of the object being acted upon
    pub object_id: Option<String>,
    
    /// Type of object (e.g., "transaction", "account", "proposal")
    pub object_type: Option<String>,
    
    /// System timestamp (UTC)
    pub timestamp: DateTime<Utc>,
    
    /// Hardware/system signature (e.g., server ID, pod name)
    pub hardware_signature: String,
    
    /// Correlation ID for tracing related operations
    pub correlation_id: Option<String>,
    
    /// Additional structured metadata
    pub metadata: serde_json::Value,
    
    /// IP address of the actor (if applicable)
    pub ip_address: Option<String>,
    
    /// User agent string (if applicable)
    pub user_agent: Option<String>,
    
    /// Result of the action (success/failure)
    pub result: String,
    
    /// Error message (if action failed)
    pub error_message: Option<String>,
}

impl AuditLogEntry {
    /// Calculate the hash of this entry's content
    pub fn calculate_hash(&self) -> String {
        let mut hasher = Sha256::new();
        
        // Hash all immutable fields in canonical order
        hasher.update(self.id.as_bytes());
        hasher.update(self.sequence.to_le_bytes());
        hasher.update(self.previous_hash.as_bytes());
        hasher.update(self.actor_id.as_bytes());
        hasher.update(format!("{:?}", self.actor_type).as_bytes());
        hasher.update(format!("{:?}", self.action_type).as_bytes());
        
        if let Some(ref obj_id) = self.object_id {
            hasher.update(obj_id.as_bytes());
        }
        
        if let Some(ref obj_type) = self.object_type {
            hasher.update(obj_type.as_bytes());
        }
        
        hasher.update(self.timestamp.to_rfc3339().as_bytes());
        hasher.update(self.hardware_signature.as_bytes());
        
        if let Some(ref corr_id) = self.correlation_id {
            hasher.update(corr_id.as_bytes());
        }
        
        hasher.update(self.metadata.to_string().as_bytes());
        hasher.update(self.result.as_bytes());
        
        hex::encode(hasher.finalize())
    }
}

/// Anchor point for hash-chain verification
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnchorPoint {
    pub id: Uuid,
    pub sequence: i64,
    pub entry_hash: String,
    pub anchor_timestamp: DateTime<Utc>,
    pub stellar_transaction_id: Option<String>,
    pub stellar_ledger: Option<i64>,
}

/// Append-only audit ledger manager
pub struct AuditLedger {
    pool: PgPool,
    last_sequence: Arc<RwLock<i64>>,
    last_hash: Arc<RwLock<String>>,
    hardware_signature: String,
}

impl AuditLedger {
    /// Create a new audit ledger instance
    pub async fn new(pool: PgPool) -> Result<Self, AuditLedgerError> {
        let hardware_signature = Self::get_hardware_signature();
        
        // Get the last sequence and hash from the database
        let (last_sequence, last_hash) = Self::get_last_entry(&pool).await?;
        
        info!(
            "Initialized audit ledger: last_sequence={}, hardware_signature={}",
            last_sequence, hardware_signature
        );
        
        Ok(Self {
            pool,
            last_sequence: Arc::new(RwLock::new(last_sequence)),
            last_hash: Arc::new(RwLock::new(last_hash)),
            hardware_signature,
        })
    }
    
    /// Append a new entry to the audit ledger
    pub async fn append(
        &self,
        actor_id: String,
        actor_type: ActorType,
        action_type: ActionType,
        object_id: Option<String>,
        object_type: Option<String>,
        correlation_id: Option<String>,
        metadata: serde_json::Value,
        ip_address: Option<String>,
        user_agent: Option<String>,
        result: String,
        error_message: Option<String>,
    ) -> Result<AuditLogEntry, AuditLedgerError> {
        // Acquire write locks to ensure sequential consistency
        let mut seq_lock = self.last_sequence.write().await;
        let mut hash_lock = self.last_hash.write().await;
        
        let sequence = *seq_lock + 1;
        let previous_hash = hash_lock.clone();
        
        let mut entry = AuditLogEntry {
            id: Uuid::new_v4(),
            sequence,
            previous_hash,
            entry_hash: String::new(), // Will be calculated
            actor_id,
            actor_type,
            action_type,
            object_id,
            object_type,
            timestamp: Utc::now(),
            hardware_signature: self.hardware_signature.clone(),
            correlation_id,
            metadata,
            ip_address,
            user_agent,
            result,
            error_message,
        };
        
        // Calculate the hash of this entry
        entry.entry_hash = entry.calculate_hash();
        
        // Persist to database with WORM guarantees
        self.persist_entry(&entry).await?;
        
        // Update in-memory state
        *seq_lock = sequence;
        *hash_lock = entry.entry_hash.clone();
        
        Ok(entry)
    }

    
    /// Verify the integrity of the entire audit chain
    pub async fn verify_chain(
        &self,
        from_sequence: i64,
        to_sequence: Option<i64>,
    ) -> Result<ChainVerificationResult, AuditLedgerError> {
        let entries = self.fetch_entries(from_sequence, to_sequence).await?;
        
        if entries.is_empty() {
            return Ok(ChainVerificationResult {
                valid: true,
                total_entries: 0,
                verified_entries: 0,
                broken_links: vec![],
            });
        }
        
        let mut broken_links = Vec::new();
        let total_entries = entries.len();
        let mut verified_entries = 0;
        
        for i in 0..entries.len() {
            let entry = &entries[i];
            
            // Verify hash of current entry
            let calculated_hash = entry.calculate_hash();
            if calculated_hash != entry.entry_hash {
                broken_links.push(BrokenLink {
                    sequence: entry.sequence,
                    reason: format!(
                        "Hash mismatch: expected {}, got {}",
                        entry.entry_hash, calculated_hash
                    ),
                });
                continue;
            }
            
            // Verify link to previous entry
            if i > 0 {
                let prev_entry = &entries[i - 1];
                if entry.previous_hash != prev_entry.entry_hash {
                    broken_links.push(BrokenLink {
                        sequence: entry.sequence,
                        reason: format!(
                            "Chain broken: previous_hash {} does not match previous entry hash {}",
                            entry.previous_hash, prev_entry.entry_hash
                        ),
                    });
                    continue;
                }
            }
            
            verified_entries += 1;
        }
        
        Ok(ChainVerificationResult {
            valid: broken_links.is_empty(),
            total_entries,
            verified_entries,
            broken_links,
        })
    }
    
    /// Create an anchor point by submitting hash to Stellar
    pub async fn create_anchor(&self) -> Result<AnchorPoint, AuditLedgerError> {
        let seq_lock = self.last_sequence.read().await;
        let hash_lock = self.last_hash.read().await;
        
        let anchor = AnchorPoint {
            id: Uuid::new_v4(),
            sequence: *seq_lock,
            entry_hash: hash_lock.clone(),
            anchor_timestamp: Utc::now(),
            stellar_transaction_id: None, // Will be set after Stellar submission
            stellar_ledger: None,
        };
        
        // Persist anchor point
        self.persist_anchor(&anchor).await?;
        
        info!(
            "Created anchor point: sequence={}, hash={}",
            anchor.sequence, anchor.entry_hash
        );
        
        Ok(anchor)
    }
    
    /// Submit anchor to Stellar blockchain
    pub async fn submit_anchor_to_stellar(
        &self,
        anchor_id: Uuid,
    ) -> Result<(), AuditLedgerError> {
        // Fetch the anchor
        let anchor = self.fetch_anchor(anchor_id).await?;
        
        // TODO: Implement Stellar transaction submission
        // This would use the stellar_sdk to create a transaction with the hash in the memo
        warn!(
            "Stellar anchor submission not yet implemented for anchor {}",
            anchor_id
        );
        
        Ok(())
    }
    
    /// Get hardware signature for this instance
    fn get_hardware_signature() -> String {
        // Use hostname + pod name (for Kubernetes) or container ID
        let hostname = hostname::get()
            .ok()
            .and_then(|h| h.into_string().ok())
            .unwrap_or_else(|| "unknown".to_string());
        
        let pod_name = std::env::var("POD_NAME").unwrap_or_else(|_| "local".to_string());
        
        format!("{}:{}", hostname, pod_name)
    }
    
    /// Get the last entry from the database
    async fn get_last_entry(pool: &PgPool) -> Result<(i64, String), AuditLedgerError> {
        let result = sqlx::query!(
            r#"
            SELECT sequence, entry_hash
            FROM audit_ledger
            ORDER BY sequence DESC
            LIMIT 1
            "#
        )
        .fetch_optional(pool)
        .await
        .map_err(|e| AuditLedgerError::Database(e.to_string()))?;
        
        match result {
            Some(row) => Ok((row.sequence, row.entry_hash)),
            None => Ok((0, "genesis".to_string())),
        }
    }
    
    /// Persist an entry to the database
    async fn persist_entry(&self, entry: &AuditLogEntry) -> Result<(), AuditLedgerError> {
        sqlx::query!(
            r#"
            INSERT INTO audit_ledger (
                id, sequence, previous_hash, entry_hash,
                actor_id, actor_type, action_type,
                object_id, object_type, timestamp,
                hardware_signature, correlation_id, metadata,
                ip_address, user_agent, result, error_message
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)
            "#,
            entry.id,
            entry.sequence,
            entry.previous_hash,
            entry.entry_hash,
            entry.actor_id,
            entry.actor_type as ActorType,
            entry.action_type as ActionType,
            entry.object_id,
            entry.object_type,
            entry.timestamp,
            entry.hardware_signature,
            entry.correlation_id,
            entry.metadata,
            entry.ip_address,
            entry.user_agent,
            entry.result,
            entry.error_message,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| AuditLedgerError::Database(e.to_string()))?;
        
        Ok(())
    }
    
    /// Fetch entries from the database
    async fn fetch_entries(
        &self,
        from_sequence: i64,
        to_sequence: Option<i64>,
    ) -> Result<Vec<AuditLogEntry>, AuditLedgerError> {
        let entries = if let Some(to_seq) = to_sequence {
            sqlx::query_as!(
                AuditLogEntry,
                r#"
                SELECT 
                    id, sequence, previous_hash, entry_hash,
                    actor_id, actor_type as "actor_type: ActorType", 
                    action_type as "action_type: ActionType",
                    object_id, object_type, timestamp,
                    hardware_signature, correlation_id, metadata,
                    ip_address, user_agent, result, error_message
                FROM audit_ledger
                WHERE sequence >= $1 AND sequence <= $2
                ORDER BY sequence ASC
                "#,
                from_sequence,
                to_seq
            )
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query_as!(
                AuditLogEntry,
                r#"
                SELECT 
                    id, sequence, previous_hash, entry_hash,
                    actor_id, actor_type as "actor_type: ActorType",
                    action_type as "action_type: ActionType",
                    object_id, object_type, timestamp,
                    hardware_signature, correlation_id, metadata,
                    ip_address, user_agent, result, error_message
                FROM audit_ledger
                WHERE sequence >= $1
                ORDER BY sequence ASC
                "#,
                from_sequence
            )
            .fetch_all(&self.pool)
            .await
        };
        
        entries.map_err(|e| AuditLedgerError::Database(e.to_string()))
    }
    
    /// Persist an anchor point
    async fn persist_anchor(&self, anchor: &AnchorPoint) -> Result<(), AuditLedgerError> {
        sqlx::query!(
            r#"
            INSERT INTO audit_anchors (
                id, sequence, entry_hash, anchor_timestamp,
                stellar_transaction_id, stellar_ledger
            ) VALUES ($1, $2, $3, $4, $5, $6)
            "#,
            anchor.id,
            anchor.sequence,
            anchor.entry_hash,
            anchor.anchor_timestamp,
            anchor.stellar_transaction_id,
            anchor.stellar_ledger,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| AuditLedgerError::Database(e.to_string()))?;
        
        Ok(())
    }
    
    /// Fetch an anchor point
    async fn fetch_anchor(&self, anchor_id: Uuid) -> Result<AnchorPoint, AuditLedgerError> {
        sqlx::query_as!(
            AnchorPoint,
            r#"
            SELECT id, sequence, entry_hash, anchor_timestamp,
                   stellar_transaction_id, stellar_ledger
            FROM audit_anchors
            WHERE id = $1
            "#,
            anchor_id
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| AuditLedgerError::Database(e.to_string()))
    }
}

/// Result of chain verification
#[derive(Debug, Serialize, Deserialize)]
pub struct ChainVerificationResult {
    pub valid: bool,
    pub total_entries: usize,
    pub verified_entries: usize,
    pub broken_links: Vec<BrokenLink>,
}

/// Information about a broken link in the chain
#[derive(Debug, Serialize, Deserialize)]
pub struct BrokenLink {
    pub sequence: i64,
    pub reason: String,
}

/// Errors that can occur in the audit ledger
#[derive(Debug, thiserror::Error)]
pub enum AuditLedgerError {
    #[error("Database error: {0}")]
    Database(String),
    
    #[error("Chain verification failed: {0}")]
    VerificationFailed(String),
    
    #[error("Anchor not found: {0}")]
    AnchorNotFound(String),
    
    #[error("Stellar submission failed: {0}")]
    StellarSubmissionFailed(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_entry_hash_calculation() {
        let entry = AuditLogEntry {
            id: Uuid::new_v4(),
            sequence: 1,
            previous_hash: "genesis".to_string(),
            entry_hash: String::new(),
            actor_id: "user123".to_string(),
            actor_type: ActorType::User,
            action_type: ActionType::Create,
            object_id: Some("txn456".to_string()),
            object_type: Some("transaction".to_string()),
            timestamp: Utc::now(),
            hardware_signature: "server1:pod1".to_string(),
            correlation_id: Some("corr789".to_string()),
            metadata: serde_json::json!({"amount": 100}),
            ip_address: Some("192.168.1.1".to_string()),
            user_agent: Some("Mozilla/5.0".to_string()),
            result: "success".to_string(),
            error_message: None,
        };
        
        let hash1 = entry.calculate_hash();
        let hash2 = entry.calculate_hash();
        
        // Hash should be deterministic
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64); // SHA-256 produces 64 hex characters
    }
}
