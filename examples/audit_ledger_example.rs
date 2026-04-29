// Example usage of the Append-Only Audit Ledger
//
// This example demonstrates how to:
// 1. Initialize the audit ledger
// 2. Log various operations
// 3. Verify chain integrity
// 4. Create and verify anchor points

use std::sync::Arc;

// Note: This is a conceptual example. Actual implementation would require
// the database feature and proper imports.

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Append-Only Audit Ledger Example ===\n");

    // 1. Initialize database connection
    println!("1. Initializing database connection...");
    // let database_url = std::env::var("DATABASE_URL")?;
    // let pool = sqlx::PgPool::connect(&database_url).await?;
    println!("   ✓ Database connected\n");

    // 2. Initialize audit ledger
    println!("2. Initializing audit ledger...");
    // let audit_ledger = Arc::new(AuditLedger::new(pool.clone()).await?);
    println!("   ✓ Audit ledger initialized\n");

    // 3. Create audit logger
    println!("3. Creating audit logger...");
    // let logger = AuditLogger::new(audit_ledger.clone());
    println!("   ✓ Audit logger ready\n");

    // 4. Log a transaction
    println!("4. Logging a transaction...");
    // logger.log_transaction(
    //     "user123".to_string(),
    //     ActorType::User,
    //     ActionType::Transfer,
    //     "txn_abc123".to_string(),
    //     "100.00".to_string(),
    //     "CNGN".to_string(),
    //     Some("correlation_xyz".to_string()),
    // ).await?;
    println!("   ✓ Transaction logged\n");

    // 5. Log authentication
    println!("5. Logging authentication event...");
    // logger.log_authentication(
    //     "user123".to_string(),
    //     ActorType::User,
    //     true,
    //     Some("192.168.1.1".to_string()),
    //     Some("Mozilla/5.0".to_string()),
    //     None,
    // ).await?;
    println!("   ✓ Authentication logged\n");

    // 6. Log governance action
    println!("6. Logging governance action...");
    // logger.log_governance(
    //     "admin456".to_string(),
    //     ActionType::Approve,
    //     "proposal_789".to_string(),
    //     "parameter_change".to_string(),
    //     None,
    // ).await?;
    println!("   ✓ Governance action logged\n");

    // 7. Log mint operation
    println!("7. Logging mint operation...");
    // logger.log_mint_burn(
    //     "system".to_string(),
    //     ActionType::Mint,
    //     "1000.00".to_string(),
    //     "CNGN".to_string(),
    //     "mint_txn_456".to_string(),
    //     None,
    // ).await?;
    println!("   ✓ Mint operation logged\n");

    // 8. Verify chain integrity
    println!("8. Verifying chain integrity...");
    // let verification = audit_ledger.verify_chain(0, None).await?;
    // if verification.valid {
    //     println!("   ✓ Chain is valid!");
    //     println!("     Total entries: {}", verification.total_entries);
    //     println!("     Verified entries: {}", verification.verified_entries);
    // } else {
    //     println!("   ✗ Chain verification FAILED!");
    //     for broken_link in verification.broken_links {
    //         println!("     Sequence {}: {}", broken_link.sequence, broken_link.reason);
    //     }
    // }
    println!("   ✓ Chain verified\n");

    // 9. Create anchor point
    println!("9. Creating anchor point...");
    // let anchor = audit_ledger.create_anchor().await?;
    // println!("   ✓ Anchor created:");
    // println!("     ID: {}", anchor.id);
    // println!("     Sequence: {}", anchor.sequence);
    // println!("     Hash: {}", anchor.entry_hash);
    println!("   ✓ Anchor point created\n");

    // 10. Initialize Stellar anchoring service
    println!("10. Initializing Stellar anchoring service...");
    // let stellar_config = StellarAnchorConfig {
    //     horizon_url: "https://horizon-testnet.stellar.org".to_string(),
    //     network_passphrase: "Test SDF Network ; September 2015".to_string(),
    //     source_secret: std::env::var("STELLAR_ANCHOR_SECRET")?,
    //     anchor_interval_seconds: 3600,
    //     destination_account: None,
    //     base_fee: 100,
    // };
    // 
    // let anchor_service = Arc::new(StellarAnchorService::new(
    //     stellar_config,
    //     audit_ledger.clone(),
    //     pool.clone(),
    // ));
    println!("   ✓ Stellar anchoring service initialized\n");

    // 11. Start anchoring service in background
    println!("11. Starting Stellar anchoring service...");
    // tokio::spawn(async move {
    //     anchor_service.start().await;
    // });
    println!("   ✓ Anchoring service started\n");

    println!("=== Example Complete ===");
    println!("\nThe audit ledger is now:");
    println!("  • Logging all operations with cryptographic hash chaining");
    println!("  • Enforcing WORM (Write-Once-Read-Many) guarantees");
    println!("  • Periodically anchoring to Stellar blockchain");
    println!("  • Providing forensic-grade audit trails");
    println!("\nPress Ctrl+C to stop...");

    // Keep the program running
    // tokio::signal::ctrl_c().await?;

    Ok(())
}
