# CTR Auto-Generation on Threshold Breach

## Overview

The CTR auto-generation system automatically creates draft Currency Transaction Reports when a subject breaches the daily transaction threshold. The system:

1. **Detects threshold breaches** via the aggregation service
2. **Prevents duplicate CTRs** for the same subject and reporting window
3. **Pre-populates CTR data** with subject KYC information
4. **Links all transactions** in the reporting window
5. **Sets filing deadlines** automatically
6. **Notifies compliance officers** of new CTRs
7. **Handles concurrent breaches** with individual CTRs per subject

## Architecture

```
Transaction Confirmed
        ↓
CtrAggregationService.process_transaction()
        ↓
    Threshold Breached?
        ↓ (yes)
CtrGeneratorService.generate_ctr_on_breach()
        ↓
    Check for Existing CTR
        ↓ (none found)
    Fetch Subject KYC Data
        ↓
    Fetch All Transactions in Window
        ↓
    Create Draft CTR
        ↓
    Create CTR Transaction Records
        ↓
    Notify Compliance Officer
        ↓
    Return CtrGenerationResult
```

## Setup

### 1. Initialize Services

```rust
use Bitmesh_backend::aml::{
    CtrAggregationService, CtrAggregationConfig,
    CtrGeneratorService, CtrGeneratorConfig,
};
use sqlx::PgPool;
use std::sync::Arc;

let pool = PgPool::connect(&database_url).await?;

// Configure CTR generator
let generator_config = CtrGeneratorConfig {
    filing_deadline_days: 15,  // 15 days to file
    default_compliance_officer: Some(officer_uuid),
};
let ctr_generator = Arc::new(CtrGeneratorService::new(pool.clone(), generator_config));

// Configure aggregation service with auto-generation
let aggregation_config = CtrAggregationConfig::default();
let aggregation_service = CtrAggregationService::with_ctr_generator(
    pool.clone(),
    aggregation_config,
    ctr_generator.clone(),
);
```

### 2. Process Transactions with Auto-Generation

```rust
use Bitmesh_backend::aml::CtrType;
use rust_decimal::Decimal;
use std::str::FromStr;

// Process a confirmed transaction
let result = aggregation_service.process_transaction(
    subject_kyc_id,
    CtrType::Individual,
    transaction_id,
    Decimal::from_str("2000000").unwrap(),  // NGN 2M
    chrono::Utc::now(),
).await?;

// Check if CTR was auto-generated
if let Some(ctr_result) = result.ctr_generated {
    if ctr_result.already_existed {
        println!("Existing CTR updated: {}", ctr_result.ctr_id);
    } else {
        println!("New CTR created: {}", ctr_result.ctr_id);
        println!("Filing deadline: {}", ctr_result.filing_deadline);
    }
}
```

## Features

### Duplicate Prevention

The system checks for existing CTRs before creating a new one:

```rust
// If a CTR already exists for this subject and reporting window,
// it returns the existing CTR instead of creating a duplicate
let result = generator.generate_ctr_on_breach(
    subject_id,
    window_start,
    window_end,
    total_amount,
    transaction_count,
    None,
).await?;

if result.already_existed {
    println!("CTR already exists, no duplicate created");
}
```

### Concurrent Breach Handling

Each subject gets their own CTR, even if multiple subjects breach simultaneously:

```rust
// Process multiple subjects concurrently
let handles: Vec<_> = subjects.iter().map(|subject| {
    let service = aggregation_service.clone();
    tokio::spawn(async move {
        service.process_transaction(
            subject.kyc_id,
            subject.subject_type,
            transaction_id,
            amount,
            timestamp,
        ).await
    })
}).collect();

// Each subject gets their own CTR
for handle in handles {
    let result = handle.await??;
    if let Some(ctr) = result.ctr_generated {
        println!("CTR {} created for subject {}", ctr.ctr_id, result.subject_id);
    }
}
```

### Pre-Populated CTR Data

The generated CTR includes:

```rust
pub struct Ctr {
    pub ctr_id: Uuid,                           // Auto-generated
    pub reporting_period: DateTime<Utc>,        // Window start
    pub ctr_type: CtrType,                      // Individual/Corporate
    pub subject_kyc_id: Uuid,                   // From KYC records
    pub subject_full_name: String,              // From consumers table
    pub subject_identification: String,         // From KYC documents
    pub subject_address: String,                // From KYC documents
    pub total_transaction_amount: Decimal,      // Aggregated total
    pub transaction_count: i32,                 // Number of transactions
    pub transaction_references: Vec<String>,    // All transaction IDs
    pub detection_method: DetectionMethod,      // Automatic
    pub status: CtrStatus,                      // Draft
    pub assigned_compliance_officer: Option<Uuid>,
    pub filing_timestamp: Option<DateTime<Utc>>, // Deadline
    pub regulatory_reference_number: Option<String>,
}
```

### Transaction Linking

All transactions in the reporting window are linked to the CTR:

```rust
// Get all transactions for a CTR
let transactions = ctr_generator.get_ctr_transactions(ctr_id).await?;

for tx in transactions {
    println!("Transaction: {} - {} NGN - {}",
        tx.transaction_id,
        tx.transaction_amount_ngn,
        tx.transaction_type
    );
}
```

### Compliance Officer Notification

Officers are notified via the `compliance_notifications` table:

```rust
// Notification is automatically created with:
// - Officer ID
// - CTR ID
// - Subject information
// - Total amount and transaction count
// - Filing deadline

// Officers can query their pending CTRs
let pending_ctrs = ctr_generator.get_pending_ctrs_for_officer(officer_id).await?;

for ctr in pending_ctrs {
    println!("Pending CTR: {} - {} - {} NGN - Due: {}",
        ctr.ctr_id,
        ctr.subject_full_name,
        ctr.total_transaction_amount,
        ctr.filing_timestamp.unwrap()
    );
}
```

## Database Schema

### CTRs Table

```sql
CREATE TABLE ctrs (
    ctr_id UUID PRIMARY KEY,
    reporting_period TIMESTAMPTZ NOT NULL,
    ctr_type TEXT NOT NULL,
    subject_kyc_id UUID NOT NULL REFERENCES kyc_records(id),
    subject_full_name TEXT NOT NULL,
    subject_identification TEXT NOT NULL,
    subject_address TEXT NOT NULL,
    total_transaction_amount DECIMAL(20, 2) NOT NULL,
    transaction_count INTEGER NOT NULL,
    transaction_references TEXT[] NOT NULL,
    detection_method TEXT NOT NULL,
    status TEXT NOT NULL,
    assigned_compliance_officer UUID,
    filing_timestamp TIMESTAMPTZ,
    regulatory_reference_number TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_ctrs_subject ON ctrs(subject_kyc_id, reporting_period);
CREATE INDEX idx_ctrs_status ON ctrs(status, filing_timestamp);
CREATE INDEX idx_ctrs_officer ON ctrs(assigned_compliance_officer, status);
```

### CTR Transactions Table

```sql
CREATE TABLE ctr_transactions (
    ctr_id UUID NOT NULL REFERENCES ctrs(ctr_id),
    transaction_id UUID NOT NULL REFERENCES transactions(transaction_id),
    transaction_timestamp TIMESTAMPTZ NOT NULL,
    transaction_type TEXT NOT NULL,
    transaction_amount_ngn DECIMAL(20, 2) NOT NULL,
    counterparty_details TEXT NOT NULL,
    direction TEXT NOT NULL,
    PRIMARY KEY (ctr_id, transaction_id)
);

CREATE INDEX idx_ctr_tx_ctr ON ctr_transactions(ctr_id);
CREATE INDEX idx_ctr_tx_transaction ON ctr_transactions(transaction_id);
```

### Compliance Notifications Table

```sql
CREATE TABLE compliance_notifications (
    id UUID PRIMARY KEY,
    officer_id UUID NOT NULL,
    notification_type TEXT NOT NULL,
    subject_id UUID NOT NULL,
    subject_name TEXT NOT NULL,
    ctr_id UUID REFERENCES ctrs(ctr_id),
    total_amount DECIMAL(20, 2) NOT NULL,
    transaction_count INTEGER NOT NULL,
    filing_deadline TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    read_at TIMESTAMPTZ
);

CREATE INDEX idx_compliance_notif_officer ON compliance_notifications(officer_id, read_at);
CREATE INDEX idx_compliance_notif_ctr ON compliance_notifications(ctr_id);
```

## Workflow Integration

### Transaction Confirmation Hook

```rust
// In your transaction confirmation handler
async fn on_transaction_confirmed(
    transaction: &Transaction,
    aggregation_service: &CtrAggregationService,
) -> Result<(), Error> {
    // Get subject KYC ID and type
    let kyc_record = get_kyc_record(transaction.wallet_address).await?;
    let subject_type = determine_subject_type(&kyc_record);
    
    // Process transaction for CTR aggregation
    let result = aggregation_service.process_transaction(
        kyc_record.id,
        subject_type,
        transaction.transaction_id,
        transaction.cngn_amount,
        transaction.updated_at,
    ).await?;
    
    // Log CTR generation
    if let Some(ctr) = result.ctr_generated {
        log::info!(
            "CTR {} auto-generated for subject {} (total: {} NGN)",
            ctr.ctr_id,
            result.subject_id,
            ctr.total_amount
        );
    }
    
    Ok(())
}
```

### Compliance Officer Dashboard

```rust
// Get pending CTRs for review
async fn get_officer_dashboard(
    officer_id: Uuid,
    ctr_generator: &CtrGeneratorService,
) -> Result<DashboardData, Error> {
    let pending_ctrs = ctr_generator.get_pending_ctrs_for_officer(officer_id).await?;
    
    let mut dashboard = DashboardData::default();
    
    for ctr in pending_ctrs {
        // Get associated transactions
        let transactions = ctr_generator.get_ctr_transactions(ctr.ctr_id).await?;
        
        dashboard.ctrs.push(CtrWithTransactions {
            ctr,
            transactions,
        });
    }
    
    Ok(dashboard)
}
```

### CTR Status Updates

```rust
// Update CTR status as it progresses through workflow
async fn update_ctr_workflow(
    ctr_id: Uuid,
    new_status: CtrStatus,
    ctr_generator: &CtrGeneratorService,
) -> Result<(), Error> {
    ctr_generator.update_ctr_status(ctr_id, new_status).await?;
    
    log::info!("CTR {} status updated to {:?}", ctr_id, new_status);
    
    Ok(())
}

// Example workflow progression:
// Draft -> UnderReview -> Approved -> Filed -> Acknowledged
```

## Error Handling

The system handles various error scenarios:

```rust
match aggregation_service.process_transaction(...).await {
    Ok(result) => {
        if let Some(ctr) = result.ctr_generated {
            // CTR successfully generated
        }
    }
    Err(e) => {
        // Log error but don't fail transaction confirmation
        log::error!("CTR generation failed: {}", e);
        // Transaction is still confirmed, CTR can be created manually
    }
}
```

## Logging

The system provides comprehensive logging:

```
INFO CTR aggregation updated subject_id=... running_total=5200000.00 threshold_breached=true
WARN CTR threshold breached subject_id=... running_total=5200000.00 threshold=5000000.00
INFO Starting CTR auto-generation on threshold breach subject_id=... total_amount=5200000.00
INFO Draft CTR created successfully ctr_id=... subject_name="John Doe" transaction_count=12
INFO CTR transaction records created ctr_id=... transaction_records_created=12
INFO Compliance officer notified of new CTR officer_id=... ctr_id=...
INFO CTR auto-generation completed ctr_id=... already_existed=false
```

## Configuration

### Custom Filing Deadlines

```rust
let config = CtrGeneratorConfig {
    filing_deadline_days: 30,  // 30 days instead of default 15
    default_compliance_officer: Some(officer_uuid),
};
```

### Custom Thresholds

```rust
let config = CtrAggregationConfig {
    individual_threshold: Decimal::from_str("10000000").unwrap(),  // NGN 10M
    corporate_threshold: Decimal::from_str("20000000").unwrap(),   // NGN 20M
    proximity_threshold: Decimal::from_str("0.85").unwrap(),       // 85%
};
```

## Testing

```rust
#[tokio::test]
async fn test_ctr_auto_generation() {
    let pool = setup_test_db().await;
    
    // Setup services
    let generator = Arc::new(CtrGeneratorService::new(
        pool.clone(),
        CtrGeneratorConfig::default(),
    ));
    let service = CtrAggregationService::with_ctr_generator(
        pool.clone(),
        CtrAggregationConfig::default(),
        generator.clone(),
    );
    
    // Create test subject
    let subject_id = create_test_subject(&pool).await;
    
    // Process transactions until threshold breach
    let mut total = Decimal::ZERO;
    let threshold = Decimal::from_str("5000000").unwrap();
    
    while total < threshold {
        let amount = Decimal::from_str("1000000").unwrap();
        let result = service.process_transaction(
            subject_id,
            CtrType::Individual,
            Uuid::new_v4(),
            amount,
            Utc::now(),
        ).await.unwrap();
        
        total = result.new_running_total;
        
        if result.threshold_breached {
            assert!(result.ctr_generated.is_some());
            let ctr = result.ctr_generated.unwrap();
            assert_eq!(ctr.subject_id, subject_id);
            assert!(!ctr.already_existed);
            break;
        }
    }
}
```
