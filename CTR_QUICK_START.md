# CTR System Quick Start Guide

## Overview
The Currency Transaction Report (CTR) system automatically monitors transactions, detects threshold breaches, and manages the complete CTR lifecycle from generation to regulatory filing.

## Quick Start

### 1. Service Initialization

```rust
use crate::aml::{
    CtrAggregationService, CtrAggregationConfig,
    CtrGeneratorService, CtrGeneratorConfig,
    CtrExemptionService, CtrExemptionConfig,
    CtrManagementService, CtrManagementConfig,
    CtrFilingService, CtrFilingConfig,
    CtrBatchFilingService, BatchFilingConfig,
    CtrReconciliationService,
};
use sqlx::PgPool;
use std::sync::Arc;

async fn setup_ctr_services(pool: PgPool) -> Result<(), anyhow::Error> {
    // 1. Create exemption service
    let exemption_service = Arc::new(CtrExemptionService::new(
        pool.clone(),
        CtrExemptionConfig::default(),
    ));

    // 2. Create generator service with exemption checking
    let generator_service = Arc::new(CtrGeneratorService::with_exemption_service(
        pool.clone(),
        CtrGeneratorConfig::default(),
        exemption_service.clone(),
    ));

    // 3. Create aggregation service with auto-generation
    let aggregation_service = Arc::new(CtrAggregationService::with_ctr_generator(
        pool.clone(),
        CtrAggregationConfig::default(),
        generator_service.clone(),
    ));

    // 4. Create management service
    let management_service = Arc::new(CtrManagementService::new(
        pool.clone(),
        CtrManagementConfig::default(),
    ));

    // 5. Create filing service
    let filing_service = Arc::new(CtrFilingService::new(
        pool.clone(),
        CtrFilingConfig {
            nfiu_api_endpoint: "https://api.nfiu.gov.ng/ctr/submit".to_string(),
            nfiu_api_key: std::env::var("NFIU_API_KEY")?,
            ..Default::default()
        },
    ));

    // 6. Create batch filing service
    let batch_service = Arc::new(CtrBatchFilingService::new(
        pool.clone(),
        BatchFilingConfig::default(),
        filing_service.clone(),
    ));

    // 7. Create reconciliation service
    let reconciliation_service = Arc::new(CtrReconciliationService::new(pool.clone()));

    Ok(())
}
```

### 2. Process Transactions (Automatic CTR Generation)

```rust
use uuid::Uuid;
use rust_decimal::Decimal;
use chrono::Utc;

async fn process_transaction(
    aggregation_service: &CtrAggregationService,
    subject_id: Uuid,
    transaction_id: Uuid,
    amount_ngn: Decimal,
) -> Result<(), anyhow::Error> {
    let result = aggregation_service
        .process_transaction(
            subject_id,
            CtrType::Individual, // or CtrType::Corporate
            transaction_id,
            amount_ngn,
            Utc::now(),
        )
        .await?;

    if result.threshold_breached {
        println!("Threshold breached! CTR auto-generated: {:?}", result.ctr_generated);
    } else if result.proximity_warning {
        println!("Approaching threshold: {} / {}", 
            result.new_running_total, 
            result.applicable_threshold
        );
    }

    Ok(())
}
```

### 3. Manage Exemptions

```rust
use crate::aml::CreateExemptionRequest;
use chrono::Duration;

async fn create_exemption(
    exemption_service: &CtrExemptionService,
    subject_id: Uuid,
) -> Result<(), anyhow::Error> {
    let request = CreateExemptionRequest {
        subject_id,
        exemption_category: "government_entity".to_string(),
        exemption_basis: "Federal government agency".to_string(),
        expiry_date: Utc::now() + Duration::days(365),
    };

    let exemption = exemption_service.create_exemption(request).await?;
    println!("Exemption created: {}", exemption.id);

    Ok(())
}

async fn check_exemption(
    exemption_service: &CtrExemptionService,
    subject_id: Uuid,
) -> Result<bool, anyhow::Error> {
    let check = exemption_service.check_exemption(subject_id).await?;
    
    if check.is_exempt {
        println!("Subject is exempt: {:?}", check.exemption);
        Ok(true)
    } else {
        Ok(false)
    }
}
```

### 4. Review and Approve CTRs

```rust
use crate::aml::{ReviewCtrRequest, ReviewChecklist, ApproveCtrRequest};

async fn review_ctr(
    management_service: &CtrManagementService,
    ctr_id: Uuid,
    reviewer_id: Uuid,
) -> Result<(), anyhow::Error> {
    let request = ReviewCtrRequest {
        reviewer_id,
        checklist: ReviewChecklist {
            subject_identity_verified: true,
            transaction_details_accurate: true,
            amounts_reconciled: true,
            supporting_documents_attached: true,
            suspicious_activity_noted: false,
            regulatory_requirements_met: true,
        },
        notes: Some("All checks passed".to_string()),
    };

    management_service.review_ctr(ctr_id, request).await?;
    println!("CTR reviewed successfully");

    Ok(())
}

async fn approve_ctr(
    management_service: &CtrManagementService,
    ctr_id: Uuid,
    approver_id: Uuid,
) -> Result<(), anyhow::Error> {
    let request = ApproveCtrRequest {
        approver_id,
        is_senior_officer: true, // Required for high-value CTRs
        notes: Some("Approved for filing".to_string()),
    };

    management_service.approve_ctr(ctr_id, request).await?;
    println!("CTR approved successfully");

    Ok(())
}
```

### 5. File CTRs

```rust
// Single CTR filing
async fn file_single_ctr(
    filing_service: &CtrFilingService,
    ctr_id: Uuid,
) -> Result<(), anyhow::Error> {
    // Generate documents
    let documents = filing_service.generate_documents(ctr_id).await?;
    println!("Documents generated: XML size = {}", documents.xml_content.len());

    // File with NFIU
    let result = filing_service.file_ctr(ctr_id).await?;
    println!("Filed successfully: {}", result.submission_reference);

    Ok(())
}

// Batch filing
async fn batch_file_ctrs(
    batch_service: &CtrBatchFilingService,
    ctr_ids: Vec<Uuid>,
) -> Result<(), anyhow::Error> {
    let request = BatchFilingRequest { ctr_ids };
    let summary = batch_service.batch_file(request).await?;

    println!("Batch filing completed:");
    println!("  Total: {}", summary.total_ctrs);
    println!("  Successful: {}", summary.successful);
    println!("  Failed: {}", summary.failed);
    println!("  Skipped: {}", summary.skipped);

    Ok(())
}
```

### 6. Monitor Deadlines

```rust
async fn monitor_deadlines(
    batch_service: &CtrBatchFilingService,
) -> Result<(), anyhow::Error> {
    // Get deadline status
    let report = batch_service.get_deadline_status().await?;
    
    println!("Deadline Status:");
    println!("  Total CTRs: {}", report.total_ctrs);
    println!("  Overdue: {}", report.overdue);
    println!("  Due today: {}", report.due_today);
    println!("  Due within 3 days: {}", report.due_within_3_days);

    // Process reminders
    let reminders = batch_service.process_deadline_reminders().await?;
    println!("Sent {} reminders", reminders.len());

    Ok(())
}
```

### 7. Reconciliation and Reporting

```rust
use crate::aml::ReconciliationRequest;
use chrono::NaiveDate;

async fn reconcile_ctrs(
    reconciliation_service: &CtrReconciliationService,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> Result<(), anyhow::Error> {
    let request = ReconciliationRequest { start_date, end_date };
    let result = reconciliation_service.reconcile(request).await?;

    println!("Reconciliation completed:");
    println!("  CTRs checked: {}", result.total_ctrs_checked);
    println!("  Discrepancies: {}", result.ctrs_with_discrepancies);

    for discrepancy in &result.discrepancies {
        println!("  - {}: {} (expected: {}, actual: {})",
            discrepancy.ctr_id,
            discrepancy.discrepancy_type,
            discrepancy.expected_value,
            discrepancy.actual_value
        );
    }

    Ok(())
}

async fn generate_monthly_report(
    reconciliation_service: &CtrReconciliationService,
    year: i32,
    month: u32,
) -> Result<(), anyhow::Error> {
    let report = reconciliation_service
        .generate_monthly_report(year, month)
        .await?;

    println!("Monthly Report for {}-{:02}:", year, month);
    println!("  Total CTRs: {}", report.total_ctrs_generated);
    println!("  Filed: {}", report.total_ctrs_filed);
    println!("  Overdue: {}", report.total_ctrs_overdue);
    println!("  Total Amount: {} NGN", report.total_amount_reported);
    println!("\nStatus Breakdown:");
    println!("  Draft: {}", report.ctrs_by_status.draft);
    println!("  Under Review: {}", report.ctrs_by_status.under_review);
    println!("  Approved: {}", report.ctrs_by_status.approved);
    println!("  Filed: {}", report.ctrs_by_status.filed);

    Ok(())
}
```

## API Endpoints

### CTR Management
```bash
# List CTRs
GET /api/admin/compliance/ctrs?status=draft&limit=50

# Get CTR details
GET /api/admin/compliance/ctrs/{ctr_id}

# Review CTR
POST /api/admin/compliance/ctrs/{ctr_id}/review
{
  "reviewer_id": "uuid",
  "checklist": {
    "subject_identity_verified": true,
    "transaction_details_accurate": true,
    "amounts_reconciled": true,
    "supporting_documents_attached": true,
    "suspicious_activity_noted": false,
    "regulatory_requirements_met": true
  },
  "notes": "All checks passed"
}

# Approve CTR
POST /api/admin/compliance/ctrs/{ctr_id}/approve
{
  "approver_id": "uuid",
  "is_senior_officer": true,
  "notes": "Approved for filing"
}
```

### CTR Exemptions
```bash
# Create exemption
POST /api/admin/compliance/ctr/exemptions
{
  "subject_id": "uuid",
  "exemption_category": "government_entity",
  "exemption_basis": "Federal government agency",
  "expiry_date": "2025-12-31T23:59:59Z"
}

# List exemptions
GET /api/admin/compliance/ctr/exemptions?status=active

# Delete exemption
DELETE /api/admin/compliance/ctr/exemptions/{exemption_id}
```

### CTR Filing
```bash
# Generate documents
POST /api/admin/compliance/ctrs/{ctr_id}/generate

# Get document
GET /api/admin/compliance/ctrs/{ctr_id}/document

# File CTR
POST /api/admin/compliance/ctrs/{ctr_id}/file
```

### Batch Operations
```bash
# Batch file CTRs
POST /api/admin/compliance/ctrs/batch-file
{
  "ctr_ids": ["uuid1", "uuid2", "uuid3"]
}

# Get deadline status
GET /api/admin/compliance/ctrs/deadline-status
```

### Reconciliation & Reporting
```bash
# Reconcile CTRs
POST /api/admin/compliance/ctrs/reconcile
{
  "start_date": "2024-01-01",
  "end_date": "2024-01-31"
}

# Monthly report
GET /api/admin/compliance/ctrs/monthly-report?year=2024&month=1
```

## Monitoring

### Prometheus Metrics
```bash
# CTR generation rate
rate(ctr_generated_total[5m])

# Threshold breaches
ctr_threshold_breach_total

# Filing success rate
rate(ctr_filed_total[1h]) / rate(ctr_generated_total[1h])

# Overdue CTRs
ctr_overdue{days_overdue_range="1-7"}

# Processing time
histogram_quantile(0.95, ctr_processing_duration_seconds)
```

### Log Queries (JSON format)
```bash
# All CTR lifecycle events
event_type:ctr_lifecycle_event

# Threshold breaches
event_type:threshold_breach

# Overdue alerts
event_type:overdue_alert

# Filing failures
event_type:ctr_filed AND status:failed
```

## Configuration

### Environment Variables
```bash
# NFIU API
NFIU_API_ENDPOINT=https://api.nfiu.gov.ng/ctr/submit
NFIU_API_KEY=your_api_key_here

# Thresholds (optional, defaults shown)
CTR_INDIVIDUAL_THRESHOLD=5000000
CTR_CORPORATE_THRESHOLD=10000000
CTR_SENIOR_APPROVAL_THRESHOLD=50000000

# Deadlines (optional, defaults shown)
CTR_FILING_DEADLINE_DAYS=15
CTR_FIRST_REMINDER_DAYS=3
CTR_SECOND_REMINDER_DAYS=1

# Notifications
COMPLIANCE_DIRECTOR_EMAIL=director@example.com
```

## Best Practices

1. **Transaction Processing**
   - Process transactions in real-time for immediate threshold detection
   - Use WAT timezone for all date calculations
   - Convert all amounts to NGN at transaction confirmation time

2. **Exemption Management**
   - Review exemptions monthly for expiry
   - Document exemption basis thoroughly
   - Alert 30 days before exemption expiry

3. **CTR Review**
   - Complete all checklist items before approval
   - Attach supporting documents
   - Note any suspicious patterns

4. **Filing**
   - File CTRs well before deadline (don't wait until day 15)
   - Monitor retry attempts
   - Investigate failed filings immediately

5. **Monitoring**
   - Set up alerts for overdue CTRs
   - Monitor threshold breach trends
   - Review monthly reports for patterns

6. **Reconciliation**
   - Run daily reconciliation
   - Investigate discrepancies immediately
   - Generate monthly reports for compliance review

## Troubleshooting

### CTR Not Generated
- Check if subject has active exemption
- Verify threshold configuration
- Check aggregation service logs

### Filing Failed
- Verify NFIU API credentials
- Check network connectivity
- Review retry logs for error details

### Discrepancies in Reconciliation
- Verify transaction data integrity
- Check for concurrent updates
- Review transaction reference mapping

### Deadline Reminders Not Sent
- Check deadline monitoring cron job
- Verify email/notification configuration
- Review reminder tracking table

## Support

For issues or questions:
1. Check logs: `event_type:ctr_lifecycle_event`
2. Review metrics: Prometheus dashboard
3. Run reconciliation: Identify data issues
4. Contact compliance team: For regulatory questions
