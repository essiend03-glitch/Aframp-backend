# CTR Batch Filing and Deadline Monitoring API

## Overview

The CTR Batch Filing and Deadline Monitoring API provides endpoints for filing multiple CTRs in a single batch operation with per-CTR status tracking, monitoring filing deadlines, and sending automated reminders. The system alerts the compliance director immediately when any CTR becomes overdue.

## Features

- ✅ Batch file multiple CTRs with per-CTR status tracking
- ✅ Comprehensive batch summary report
- ✅ Deadline status monitoring for all pending CTRs
- ✅ Automated reminders at 3 days, 1 day, and on deadline day
- ✅ Immediate alerts to compliance director for overdue CTRs
- ✅ Skip already-filed or non-approved CTRs
- ✅ Continue batch processing even if individual CTRs fail

## API Endpoints

### 1. Batch File CTRs

**POST** `/api/admin/compliance/ctrs/batch-file`

File multiple CTRs in a single batch operation with per-CTR status tracking.

#### Request Body

```json
{
  "ctr_ids": [
    "123e4567-e89b-12d3-a456-426614174000",
    "987fcdeb-51a2-43f7-8b9c-123456789abc",
    "456789ab-cdef-0123-4567-89abcdef0123"
  ]
}
```

#### Response (Success - 200 OK)

```json
{
  "success": true,
  "data": {
    "batch_id": "batch-uuid",
    "total_ctrs": 3,
    "successful": 2,
    "failed": 0,
    "skipped": 1,
    "ctr_statuses": [
      {
        "ctr_id": "123e4567-e89b-12d3-a456-426614174000",
        "subject_name": "John Doe",
        "total_amount": "5500000.00",
        "status": "Submitted",
        "submission_reference": "NFIU-2024-04-20-12345",
        "error": null,
        "retry_count": 0
      },
      {
        "ctr_id": "987fcdeb-51a2-43f7-8b9c-123456789abc",
        "subject_name": "ABC Corporation",
        "total_amount": "55000000.00",
        "status": "Submitted",
        "submission_reference": "NFIU-2024-04-20-12346",
        "error": null,
        "retry_count": 2
      },
      {
        "ctr_id": "456789ab-cdef-0123-4567-89abcdef0123",
        "subject_name": "XYZ Ltd",
        "total_amount": "3000000.00",
        "status": "Skipped",
        "submission_reference": null,
        "error": "Already in Filed status",
        "retry_count": 0
      }
    ],
    "started_at": "2024-04-20T14:00:00Z",
    "completed_at": "2024-04-20T14:02:15Z",
    "duration_seconds": 135
  }
}
```

#### Response (Error - Empty Request)

```json
{
  "success": false,
  "error": "No CTR IDs provided"
}
```

#### Example (cURL)

```bash
curl -X POST https://api.example.com/api/admin/compliance/ctrs/batch-file \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -d '{
    "ctr_ids": [
      "123e4567-e89b-12d3-a456-426614174000",
      "987fcdeb-51a2-43f7-8b9c-123456789abc"
    ]
  }'
```

---

### 2. Get Deadline Status

**GET** `/api/admin/compliance/ctrs/deadline-status`

Get deadline status for all pending CTRs with reminder tracking.

#### Response (Success - 200 OK)

```json
{
  "success": true,
  "data": {
    "total_ctrs": 5,
    "overdue": 1,
    "due_today": 1,
    "due_within_3_days": 2,
    "ctrs": [
      {
        "ctr_id": "123e4567-e89b-12d3-a456-426614174000",
        "subject_name": "John Doe",
        "total_amount": "5500000.00",
        "filing_deadline": "2024-04-18T23:59:59Z",
        "days_until_deadline": -2,
        "status": "Approved",
        "is_overdue": true,
        "reminder_sent": true
      },
      {
        "ctr_id": "987fcdeb-51a2-43f7-8b9c-123456789abc",
        "subject_name": "ABC Corporation",
        "total_amount": "55000000.00",
        "filing_deadline": "2024-04-20T23:59:59Z",
        "days_until_deadline": 0,
        "status": "Approved",
        "is_overdue": false,
        "reminder_sent": true
      },
      {
        "ctr_id": "456789ab-cdef-0123-4567-89abcdef0123",
        "subject_name": "XYZ Ltd",
        "total_amount": "3000000.00",
        "filing_deadline": "2024-04-21T23:59:59Z",
        "days_until_deadline": 1,
        "status": "UnderReview",
        "is_overdue": false,
        "reminder_sent": true
      },
      {
        "ctr_id": "789abcde-f012-3456-789a-bcdef0123456",
        "subject_name": "DEF Inc",
        "total_amount": "8000000.00",
        "filing_deadline": "2024-04-23T23:59:59Z",
        "days_until_deadline": 3,
        "status": "Approved",
        "is_overdue": false,
        "reminder_sent": true
      },
      {
        "ctr_id": "bcdef012-3456-789a-bcde-f0123456789a",
        "subject_name": "GHI Corp",
        "total_amount": "12000000.00",
        "filing_deadline": "2024-04-25T23:59:59Z",
        "days_until_deadline": 5,
        "status": "Draft",
        "is_overdue": false,
        "reminder_sent": false
      }
    ],
    "generated_at": "2024-04-20T14:30:00Z"
  }
}
```

#### Example (cURL)

```bash
curl -X GET https://api.example.com/api/admin/compliance/ctrs/deadline-status \
  -H "Authorization: Bearer YOUR_TOKEN"
```

---

## Batch Filing Workflow

### Process Flow

```
1. Receive Batch Request
         ↓
2. For Each CTR:
    ├─ Get CTR Details
    ├─ Check Status
    │   ├─ Already Filed? → Skip
    │   ├─ Not Approved? → Skip
    │   └─ Approved? → Continue
    ├─ Attempt Filing
    │   ├─ Success → Mark as Submitted
    │   └─ Failure → Mark as Failed
    └─ Record Status
         ↓
3. Generate Summary Report
    ├─ Total CTRs
    ├─ Successful Count
    ├─ Failed Count
    ├─ Skipped Count
    └─ Per-CTR Status Details
```

### Status Values

- **Submitted**: Successfully filed with NFIU
- **Acknowledged**: Filed and acknowledged by NFIU
- **Failed**: Filing attempt failed
- **Skipped**: Not filed (already filed, not approved, or error)

---

## Deadline Monitoring

### Reminder Schedule

| Days Before Deadline | Reminder Type | Action |
|---------------------|---------------|--------|
| 3 days | First Reminder | Notify assigned officer |
| 1 day | Second Reminder | Urgent notification |
| 0 days (deadline day) | Final Reminder | Final warning |
| Past deadline | Overdue Alert | Alert compliance director |

### Automated Reminder Process

```
Daily Cron Job (e.g., 9:00 AM)
         ↓
Get All Pending CTRs
         ↓
For Each CTR:
    ├─ Calculate Days Until Deadline
    ├─ Check if Reminder Already Sent (last 24h)
    ├─ Determine Reminder Type
    │   ├─ 3 days → First Reminder
    │   ├─ 1 day → Second Reminder
    │   ├─ 0 days → Final Reminder
    │   └─ Overdue → Overdue Alert
    ├─ Send Notification
    └─ Record Reminder Sent
         ↓
If Overdue:
    └─ Alert Compliance Director Immediately
```

### Overdue Alert

When a CTR becomes overdue:
1. **Immediate Alert** to compliance director
2. **Logged as ERROR** in system logs
3. **Recorded in database** with alert details
4. **Email notification** sent (if configured)

---

## Configuration

### Batch Filing Configuration

```rust
use Bitmesh_backend::aml::{BatchFilingConfig, CtrBatchFilingService};

let config = BatchFilingConfig {
    compliance_director_email: "director@example.com".to_string(),
    first_reminder_days: 3,
    second_reminder_days: 1,
    final_reminder_days: 0,
};

let service = CtrBatchFilingService::new(pool, config, filing_service);
```

### Custom Reminder Schedule

```rust
let config = BatchFilingConfig {
    compliance_director_email: "director@example.com".to_string(),
    first_reminder_days: 5,   // 5 days before
    second_reminder_days: 2,  // 2 days before
    final_reminder_days: 0,   // On deadline day
};
```

---

## Database Schema

### ctr_deadline_notifications Table

```sql
CREATE TABLE ctr_deadline_notifications (
    id UUID PRIMARY KEY,
    ctr_id UUID NOT NULL REFERENCES ctrs(ctr_id),
    notification_type TEXT NOT NULL,
    message TEXT NOT NULL,
    sent_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_ctr_deadline_notif_ctr ON ctr_deadline_notifications(ctr_id);
CREATE INDEX idx_ctr_deadline_notif_sent ON ctr_deadline_notifications(sent_at);
```

### compliance_director_alerts Table

```sql
CREATE TABLE compliance_director_alerts (
    id UUID PRIMARY KEY,
    ctr_id UUID NOT NULL REFERENCES ctrs(ctr_id),
    alert_type TEXT NOT NULL,
    message TEXT NOT NULL,
    recipient_email TEXT NOT NULL,
    sent_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_director_alerts_ctr ON compliance_director_alerts(ctr_id);
CREATE INDEX idx_director_alerts_sent ON compliance_director_alerts(sent_at);
```

---

## Batch Filing Examples

### Example 1: File All Approved CTRs

```bash
# Get all approved CTRs
curl -X GET "https://api.example.com/api/admin/compliance/ctrs?status=approved" \
  -H "Authorization: Bearer TOKEN"

# Extract CTR IDs and batch file
curl -X POST https://api.example.com/api/admin/compliance/ctrs/batch-file \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer TOKEN" \
  -d '{
    "ctr_ids": ["id1", "id2", "id3"]
  }'
```

### Example 2: File CTRs Due Today

```bash
# Get deadline status
curl -X GET https://api.example.com/api/admin/compliance/ctrs/deadline-status \
  -H "Authorization: Bearer TOKEN"

# Filter CTRs due today and batch file
# (Extract IDs where days_until_deadline == 0)
```

---

## Monitoring & Alerts

### Key Metrics

1. **Batch Filing Success Rate**
   ```sql
   SELECT 
       batch_id,
       total_ctrs,
       successful,
       failed,
       skipped,
       ROUND(100.0 * successful / total_ctrs, 2) as success_rate
   FROM batch_filing_logs
   WHERE started_at > NOW() - INTERVAL '7 days';
   ```

2. **Overdue CTRs**
   ```sql
   SELECT COUNT(*)
   FROM ctrs
   WHERE status IN ('draft', 'under_review', 'approved')
     AND filing_timestamp < NOW();
   ```

3. **Upcoming Deadlines**
   ```sql
   SELECT 
       COUNT(*) FILTER (WHERE filing_timestamp < NOW() + INTERVAL '1 day') as due_tomorrow,
       COUNT(*) FILTER (WHERE filing_timestamp < NOW() + INTERVAL '3 days') as due_within_3_days
   FROM ctrs
   WHERE status IN ('draft', 'under_review', 'approved')
     AND filing_timestamp > NOW();
   ```

### Alert Logs

```sql
-- View recent overdue alerts
SELECT 
    cda.ctr_id,
    c.subject_full_name,
    c.filing_timestamp as deadline,
    cda.message,
    cda.sent_at
FROM compliance_director_alerts cda
JOIN ctrs c ON c.ctr_id = cda.ctr_id
WHERE cda.alert_type = 'overdue_ctr'
  AND cda.sent_at > NOW() - INTERVAL '7 days'
ORDER BY cda.sent_at DESC;
```

---

## Scheduled Tasks

### Daily Reminder Job

```rust
// Run daily at 9:00 AM
#[tokio::main]
async fn run_daily_reminders() {
    let service = CtrBatchFilingService::new(pool, config, filing_service);
    
    match service.process_deadline_reminders().await {
        Ok(notifications) => {
            println!("Sent {} reminders", notifications.len());
        }
        Err(e) => {
            eprintln!("Failed to process reminders: {}", e);
        }
    }
}
```

### Cron Configuration

```bash
# Run at 9:00 AM daily
0 9 * * * /usr/local/bin/ctr-reminder-job

# Run at 5:00 PM daily (second check)
0 17 * * * /usr/local/bin/ctr-reminder-job
```

---

## Error Handling

### Batch Filing Errors

The batch filing process continues even if individual CTRs fail:

```json
{
  "ctr_id": "...",
  "status": "Failed",
  "error": "NFIU submission failed: Connection timeout",
  "retry_count": 5
}
```

### Skipped CTRs

CTRs are skipped with reason:

```json
{
  "ctr_id": "...",
  "status": "Skipped",
  "error": "Not approved (status: UnderReview)"
}
```

---

## Best Practices

### 1. Batch Filing

- **Group by deadline**: File CTRs with similar deadlines together
- **Monitor progress**: Check batch summary for failures
- **Retry failures**: Manually retry failed CTRs after investigating
- **Schedule batches**: Run batch filing during off-peak hours

### 2. Deadline Monitoring

- **Daily checks**: Run deadline monitoring at least twice daily
- **Early filing**: File CTRs well before deadline
- **Track reminders**: Monitor reminder delivery
- **Investigate overdue**: Immediately investigate overdue CTRs

### 3. Alert Management

- **Acknowledge alerts**: Respond to overdue alerts promptly
- **Document actions**: Record actions taken for overdue CTRs
- **Escalate issues**: Escalate persistent issues to management
- **Review patterns**: Analyze recurring deadline issues

---

## Troubleshooting

### Issue: Batch Filing Takes Too Long

**Symptoms:** Batch operation times out

**Solutions:**
1. Reduce batch size (file in smaller batches)
2. Increase timeout configuration
3. File during off-peak hours
4. Check NFIU API performance

### Issue: Reminders Not Sending

**Symptoms:** No reminders received

**Solutions:**
1. Check cron job is running
2. Verify email configuration
3. Check database for notification records
4. Review application logs

### Issue: False Overdue Alerts

**Symptoms:** Alerts for CTRs that aren't overdue

**Solutions:**
1. Verify server timezone is correct
2. Check filing_timestamp values
3. Ensure deadline calculation is accurate
4. Review reminder logic

---

## Compliance Notes

- **Batch filing** should be used for efficiency, not to delay filing
- **Overdue CTRs** must be investigated and filed immediately
- **Reminder history** must be maintained for audit purposes
- **Director alerts** must be acknowledged and acted upon
- **Filing deadlines** are regulatory requirements and must be met
- **Batch logs** should be retained for compliance audits

---

## Testing

### Integration Test Example

```rust
#[tokio::test]
async fn test_batch_filing() {
    let pool = setup_test_db().await;
    let filing_service = Arc::new(CtrFilingService::new(pool.clone(), config));
    let batch_service = CtrBatchFilingService::new(
        pool.clone(),
        BatchFilingConfig::default(),
        filing_service,
    );
    
    // Create test CTRs
    let ctr_ids = vec![
        create_approved_ctr(&pool).await,
        create_approved_ctr(&pool).await,
        create_filed_ctr(&pool).await, // Should be skipped
    ];
    
    // Batch file
    let result = batch_service.batch_file(BatchFilingRequest { ctr_ids }).await.unwrap();
    
    assert_eq!(result.total_ctrs, 3);
    assert_eq!(result.successful, 2);
    assert_eq!(result.skipped, 1);
}

#[tokio::test]
async fn test_deadline_monitoring() {
    let pool = setup_test_db().await;
    let service = setup_batch_service(&pool).await;
    
    // Create CTR with deadline tomorrow
    let ctr_id = create_ctr_with_deadline(&pool, Utc::now() + Duration::days(1)).await;
    
    // Get deadline status
    let report = service.get_deadline_status().await.unwrap();
    
    assert_eq!(report.due_within_3_days, 1);
    assert!(!report.ctrs[0].is_overdue);
}
```

---

## Performance Considerations

### Batch Size

- **Recommended**: 10-50 CTRs per batch
- **Maximum**: 100 CTRs per batch
- **Large batches**: Split into multiple smaller batches

### Concurrent Processing

For very large batches, consider parallel processing:

```rust
// Process CTRs in parallel (with concurrency limit)
use futures::stream::{self, StreamExt};

let results = stream::iter(ctr_ids)
    .map(|ctr_id| async move {
        service.file_single_ctr(ctr_id).await
    })
    .buffer_unordered(10) // Process 10 at a time
    .collect::<Vec<_>>()
    .await;
```

---

## Security Considerations

- **Authorization**: Only compliance officers should access batch filing
- **Audit trail**: Log all batch operations
- **Rate limiting**: Prevent abuse of batch endpoint
- **Validation**: Verify all CTR IDs belong to the organization
- **Alert security**: Protect compliance director contact information
