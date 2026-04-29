# CTR Transaction Aggregation Service

## Overview

The CTR (Currency Transaction Report) Aggregation Service maintains rolling daily aggregation windows per subject (00:00–23:59 WAT), computes NGN equivalents at confirmation time, updates the subject's daily total on every confirmed transaction, and triggers threshold breach flags.

## Features

- **Rolling Daily Windows**: Aggregates transactions within WAT (West Africa Time) day boundaries (00:00–23:59)
- **Automatic Threshold Detection**: 
  - Individual subjects: NGN 5,000,000
  - Corporate subjects: NGN 10,000,000
- **Proximity Warnings**: Configurable early warning when approaching threshold (default: 90%)
- **Comprehensive Logging**: Every aggregation update is logged with full context

## Usage

### Initialize the Service

```rust
use Bitmesh_backend::aml::{CtrAggregationService, CtrAggregationConfig};
use sqlx::PgPool;

let pool = PgPool::connect(&database_url).await?;
let config = CtrAggregationConfig::default();
let service = CtrAggregationService::new(pool, config);
```

### Process a Confirmed Transaction

```rust
use Bitmesh_backend::aml::CtrType;
use rust_decimal::Decimal;
use std::str::FromStr;

let result = service.process_transaction(
    subject_id,                                    // UUID of the KYC subject
    CtrType::Individual,                           // or CtrType::Corporate
    transaction_id,                                // UUID of the transaction
    Decimal::from_str("1500000").unwrap(),        // Amount in NGN
    chrono::Utc::now(),                           // Transaction timestamp
).await?;

if result.threshold_breached {
    println!("CTR threshold breached! Total: {}", result.new_running_total);
}

if result.proximity_warning {
    println!("Approaching threshold! Total: {}", result.new_running_total);
}
```

### Query Aggregations

```rust
use chrono::NaiveDate;

// Get aggregation for a specific subject and date
let aggregation = service.get_aggregation_for_date(
    subject_id,
    NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
).await?;

// Get all threshold breaches for a date
let breaches = service.get_threshold_breaches_for_date(
    NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
).await?;

// Get proximity warnings for a date
let warnings = service.get_proximity_warnings_for_date(
    NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
    CtrType::Individual,
).await?;
```

### Custom Configuration

```rust
use rust_decimal::Decimal;
use std::str::FromStr;

let config = CtrAggregationConfig {
    individual_threshold: Decimal::from_str("5000000").unwrap(),  // NGN 5M
    corporate_threshold: Decimal::from_str("10000000").unwrap(),  // NGN 10M
    proximity_threshold: Decimal::from_str("0.85").unwrap(),      // 85% warning
};

let service = CtrAggregationService::new(pool, config);
```

## Database Schema

The service requires a `ctr_aggregations` table:

```sql
CREATE TABLE ctr_aggregations (
    subject_id UUID NOT NULL,
    aggregation_window_start TIMESTAMPTZ NOT NULL,
    aggregation_window_end TIMESTAMPTZ NOT NULL,
    running_total_amount DECIMAL(20, 2) NOT NULL,
    transaction_count INTEGER NOT NULL,
    transaction_amounts DECIMAL(20, 2)[] NOT NULL,
    transaction_timestamps TIMESTAMPTZ[] NOT NULL,
    threshold_breach_flag BOOLEAN NOT NULL DEFAULT FALSE,
    PRIMARY KEY (subject_id, aggregation_window_start, aggregation_window_end)
);

CREATE INDEX idx_ctr_agg_breach ON ctr_aggregations(threshold_breach_flag, aggregation_window_start)
    WHERE threshold_breach_flag = TRUE;
```

## Logging

The service logs:
- Every aggregation update with full context
- Threshold breaches (WARN level)
- Proximity warnings (WARN level)
- New aggregation record creation (INFO level)

Example log output:
```
INFO CTR aggregation updated subject_id=123e4567-e89b-12d3-a456-426614174000 
     transaction_id=987fcdeb-51a2-43f7-8b9c-123456789abc running_total=4500000.00 
     transaction_count=3 threshold=5000000.00 threshold_breached=false proximity_warning=true

WARN Subject approaching CTR threshold subject_id=123e4567-e89b-12d3-a456-426614174000 
     subject_type=Individual running_total=4500000.00 threshold=5000000.00 proximity_percentage=90
```

## Integration Points

### Transaction Confirmation Hook

Integrate with your transaction confirmation flow:

```rust
// After transaction is confirmed
let result = ctr_service.process_transaction(
    kyc_record.id,
    determine_subject_type(&kyc_record),
    transaction.transaction_id,
    transaction.cngn_amount.to_decimal(),
    transaction.updated_at,
).await?;

// Trigger CTR generation if threshold breached
if result.threshold_breached {
    ctr_generator.create_ctr(result.subject_id, result.new_running_total).await?;
}
```

### Scheduled Reporting

Run daily reports to identify subjects requiring CTR filing:

```rust
use chrono::Utc;

let today = Utc::now().date_naive();
let breaches = ctr_service.get_threshold_breaches_for_date(today).await?;

for breach in breaches {
    println!("Subject {} breached threshold with total {}", 
             breach.subject_id, breach.running_total_amount);
}
```

## Time Zone Handling

The service uses **West Africa Time (WAT, UTC+1)** for day boundaries:
- Day starts: 00:00:00 WAT (23:00:00 UTC previous day)
- Day ends: 23:59:59.999 WAT (22:59:59.999 UTC same day)

All timestamps are stored in UTC but aggregation windows are calculated in WAT.
