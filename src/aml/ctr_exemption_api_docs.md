# CTR Exemption Management API

## Overview

The CTR Exemption API allows administrators to manage exemptions for subjects that qualify for Currency Transaction Report (CTR) exemptions. The system automatically checks for active exemptions before generating CTRs and alerts when exemptions are approaching expiry.

## Features

- ✅ Create exemptions for qualified subjects
- ✅ List all exemptions with status information
- ✅ Delete exemptions
- ✅ Automatic exemption checking before CTR generation
- ✅ Expiry alerts (configurable, default 30 days)
- ✅ Comprehensive logging of all exemption checks
- ✅ Query exemptions approaching expiry

## API Endpoints

### 1. Create Exemption

**POST** `/api/admin/compliance/ctr/exemptions`

Create a new CTR exemption for a subject.

#### Request Body

```json
{
  "subject_id": "123e4567-e89b-12d3-a456-426614174000",
  "exemption_category": "Phase 1 Exemption",
  "exemption_basis": "Qualified financial institution under 31 CFR 1020.315",
  "expiry_date": "2025-12-31T23:59:59Z"
}
```

**Fields:**
- `subject_id` (UUID, required): The KYC ID of the subject
- `exemption_category` (string, required): Category of exemption (e.g., "Phase 1", "Phase 2", "Permanent")
- `exemption_basis` (string, required): Legal or regulatory basis for the exemption
- `expiry_date` (DateTime, optional): When the exemption expires. Null means perpetual exemption.

#### Response (Success - 200 OK)

```json
{
  "success": true,
  "data": {
    "subject_id": "123e4567-e89b-12d3-a456-426614174000",
    "exemption_category": "Phase 1 Exemption",
    "exemption_basis": "Qualified financial institution under 31 CFR 1020.315",
    "expiry_date": "2025-12-31T23:59:59Z",
    "message": "Exemption created successfully"
  }
}
```

#### Response (Error - 400 Bad Request)

```json
{
  "success": false,
  "error": "Active exemption already exists for subject 123e4567-e89b-12d3-a456-426614174000"
}
```

#### Example (cURL)

```bash
curl -X POST https://api.example.com/api/admin/compliance/ctr/exemptions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -d '{
    "subject_id": "123e4567-e89b-12d3-a456-426614174000",
    "exemption_category": "Phase 1 Exemption",
    "exemption_basis": "Qualified financial institution",
    "expiry_date": "2025-12-31T23:59:59Z"
  }'
```

---

### 2. Get All Exemptions

**GET** `/api/admin/compliance/ctr/exemptions`

Retrieve all CTR exemptions with status information.

#### Response (Success - 200 OK)

```json
{
  "success": true,
  "data": {
    "exemptions": [
      {
        "subject_id": "123e4567-e89b-12d3-a456-426614174000",
        "exemption_category": "Phase 1 Exemption",
        "exemption_basis": "Qualified financial institution",
        "expiry_date": "2025-12-31T23:59:59Z",
        "is_active": true,
        "days_until_expiry": 245,
        "created_at": "2024-01-15T10:30:00Z"
      },
      {
        "subject_id": "987fcdeb-51a2-43f7-8b9c-123456789abc",
        "exemption_category": "Permanent Exemption",
        "exemption_basis": "Government entity",
        "expiry_date": null,
        "is_active": true,
        "days_until_expiry": null,
        "created_at": "2024-02-01T14:20:00Z"
      }
    ],
    "total_count": 2
  }
}
```

**Response Fields:**
- `is_active` (boolean): Whether the exemption is currently active (not expired)
- `days_until_expiry` (integer, nullable): Days remaining until expiry, null for perpetual exemptions
- `created_at` (DateTime): When the exemption was created

#### Example (cURL)

```bash
curl -X GET https://api.example.com/api/admin/compliance/ctr/exemptions \
  -H "Authorization: Bearer YOUR_TOKEN"
```

---

### 3. Delete Exemption

**DELETE** `/api/admin/compliance/ctr/exemptions/:exemption_id`

Delete a CTR exemption by subject ID.

#### Path Parameters

- `exemption_id` (UUID): The subject ID of the exemption to delete

#### Response (Success - 200 OK)

```json
{
  "success": true,
  "data": {
    "subject_id": "123e4567-e89b-12d3-a456-426614174000",
    "message": "Exemption deleted successfully"
  }
}
```

#### Response (Error - 400 Bad Request)

```json
{
  "success": false,
  "error": "Exemption not found"
}
```

#### Example (cURL)

```bash
curl -X DELETE https://api.example.com/api/admin/compliance/ctr/exemptions/123e4567-e89b-12d3-a456-426614174000 \
  -H "Authorization: Bearer YOUR_TOKEN"
```

---

### 4. Get Expiring Exemptions

**GET** `/api/admin/compliance/ctr/exemptions/expiring`

Retrieve exemptions that are expiring within the configured alert window (default: 30 days).

#### Response (Success - 200 OK)

```json
{
  "success": true,
  "data": {
    "exemptions": [
      {
        "subject_id": "123e4567-e89b-12d3-a456-426614174000",
        "exemption_category": "Phase 1 Exemption",
        "exemption_basis": "Qualified financial institution",
        "expiry_date": "2024-05-15T23:59:59Z",
        "is_active": true,
        "days_until_expiry": 25,
        "created_at": "2024-01-15T10:30:00Z"
      }
    ],
    "total_count": 1
  }
}
```

#### Example (cURL)

```bash
curl -X GET https://api.example.com/api/admin/compliance/ctr/exemptions/expiring \
  -H "Authorization: Bearer YOUR_TOKEN"
```

---

## Integration with CTR Generation

### Automatic Exemption Checking

When a threshold breach occurs, the system automatically checks for active exemptions before generating a CTR:

```rust
// Initialize services with exemption checking
let exemption_service = Arc::new(CtrExemptionService::new(pool.clone(), config));
let ctr_generator = Arc::new(CtrGeneratorService::with_exemption_service(
    pool.clone(),
    generator_config,
    exemption_service.clone(),
));

// Process transaction - exemption is checked automatically
let result = aggregation_service.process_transaction(
    subject_id,
    CtrType::Individual,
    transaction_id,
    amount,
    timestamp,
).await?;

// Check if exemption was applied
if let Some(ctr_result) = result.ctr_generated {
    if ctr_result.exemption_applied {
        println!("Subject is exempt, no CTR generated");
    }
}
```

### Exemption Check Logging

Every exemption check is logged with full context:

```
INFO CTR exemption check performed subject_id=123e4567-e89b-12d3-a456-426614174000 
     is_exempt=true exemption_category="Phase 1 Exemption" 
     expiry_date=2025-12-31T23:59:59Z

INFO Subject is exempt from CTR reporting, skipping CTR generation 
     subject_id=123e4567-e89b-12d3-a456-426614174000 
     exemption_category="Phase 1 Exemption"
```

### Expiry Alerts

When an exemption is checked and found to be expiring soon:

```
WARN CTR exemption approaching expiry subject_id=123e4567-e89b-12d3-a456-426614174000 
     exemption_category="Phase 1 Exemption" days_until_expiry=25 
     expiry_date=2024-05-15T23:59:59Z
```

---

## Database Schema

### ctr_exemptions Table

```sql
CREATE TABLE ctr_exemptions (
    subject_id UUID PRIMARY KEY REFERENCES kyc_records(id),
    exemption_category TEXT NOT NULL,
    exemption_basis TEXT NOT NULL,
    expiry_date TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_ctr_exemptions_expiry ON ctr_exemptions(expiry_date)
    WHERE expiry_date IS NOT NULL;

CREATE INDEX idx_ctr_exemptions_active ON ctr_exemptions(subject_id, expiry_date)
    WHERE expiry_date IS NULL OR expiry_date > NOW();
```

---

## Configuration

### Exemption Service Configuration

```rust
use Bitmesh_backend::aml::{CtrExemptionConfig, CtrExemptionService};

let config = CtrExemptionConfig {
    expiry_alert_days: 30,  // Alert 30 days before expiry
};

let service = CtrExemptionService::new(pool, config);
```

### Custom Alert Window

```rust
let config = CtrExemptionConfig {
    expiry_alert_days: 60,  // Alert 60 days before expiry
};
```

---

## Common Exemption Categories

### Phase 1 Exemptions
- Qualified financial institutions
- Government entities
- Listed public companies

### Phase 2 Exemptions
- Non-listed businesses
- Payroll customers
- Eligible businesses with established banking relationships

### Permanent Exemptions
- Government agencies
- Central banks
- International organizations

---

## Workflow Examples

### 1. Grant Exemption to Qualified Institution

```bash
# Create exemption
curl -X POST https://api.example.com/api/admin/compliance/ctr/exemptions \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer TOKEN" \
  -d '{
    "subject_id": "123e4567-e89b-12d3-a456-426614174000",
    "exemption_category": "Phase 1 Exemption",
    "exemption_basis": "Qualified financial institution under 31 CFR 1020.315",
    "expiry_date": "2025-12-31T23:59:59Z"
  }'
```

### 2. Monitor Expiring Exemptions

```bash
# Get exemptions expiring soon
curl -X GET https://api.example.com/api/admin/compliance/ctr/exemptions/expiring \
  -H "Authorization: Bearer TOKEN"

# Review and renew as needed
```

### 3. Revoke Exemption

```bash
# Delete exemption
curl -X DELETE https://api.example.com/api/admin/compliance/ctr/exemptions/123e4567-e89b-12d3-a456-426614174000 \
  -H "Authorization: Bearer TOKEN"
```

### 4. Audit All Exemptions

```bash
# Get all exemptions with status
curl -X GET https://api.example.com/api/admin/compliance/ctr/exemptions \
  -H "Authorization: Bearer TOKEN"
```

---

## Error Handling

### Common Errors

1. **Duplicate Exemption**
   ```json
   {
     "success": false,
     "error": "Active exemption already exists for subject"
   }
   ```
   **Solution**: Delete existing exemption first or update expiry date

2. **Exemption Not Found**
   ```json
   {
     "success": false,
     "error": "Exemption not found"
   }
   ```
   **Solution**: Verify the subject ID is correct

3. **Invalid Subject ID**
   ```json
   {
     "success": false,
     "error": "Invalid UUID format"
   }
   ```
   **Solution**: Ensure subject_id is a valid UUID

---

## Maintenance Tasks

### Cleanup Expired Exemptions

```rust
// Run periodically (e.g., daily cron job)
let deleted = exemption_service.cleanup_expired_exemptions().await?;
println!("Cleaned up {} expired exemptions", deleted);
```

### Alert on Expiring Exemptions

```rust
// Run daily to alert compliance team
let expiring = exemption_service.get_expiring_exemptions().await?;

for exemption in expiring {
    send_alert_to_compliance_team(
        exemption.subject_id,
        exemption.exemption_category,
        exemption.days_until_expiry,
    ).await?;
}
```

---

## Security Considerations

1. **Authentication**: All endpoints require admin authentication
2. **Authorization**: Only compliance officers should have access
3. **Audit Trail**: All exemption operations are logged
4. **Validation**: Subject IDs must reference valid KYC records

---

## Testing

### Integration Test Example

```rust
#[tokio::test]
async fn test_exemption_prevents_ctr_generation() {
    let pool = setup_test_db().await;
    
    // Create exemption
    let exemption_service = Arc::new(CtrExemptionService::new(
        pool.clone(),
        CtrExemptionConfig::default(),
    ));
    
    exemption_service.create_exemption(CreateExemptionRequest {
        subject_id: test_subject_id,
        exemption_category: "Test Exemption".to_string(),
        exemption_basis: "Testing".to_string(),
        expiry_date: Some(Utc::now() + Duration::days(365)),
    }).await.unwrap();
    
    // Setup CTR generator with exemption checking
    let ctr_generator = Arc::new(CtrGeneratorService::with_exemption_service(
        pool.clone(),
        CtrGeneratorConfig::default(),
        exemption_service,
    ));
    
    // Attempt to generate CTR
    let result = ctr_generator.generate_ctr_on_breach(
        test_subject_id,
        window_start,
        window_end,
        Decimal::from_str("10000000").unwrap(),
        10,
        None,
    ).await.unwrap();
    
    // Verify exemption was applied
    assert!(result.exemption_applied);
    assert_eq!(result.ctr_id, Uuid::nil());
}
```

---

## Compliance Notes

- Exemptions must be reviewed and renewed periodically
- Document the basis for each exemption
- Monitor expiring exemptions proactively
- Maintain audit trail of all exemption changes
- Ensure exemptions comply with local regulations (e.g., FinCEN guidelines in the US)
