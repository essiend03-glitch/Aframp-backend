# CTR Filing API Documentation

## Overview

The CTR Filing API handles document generation (NFIU-compliant XML and PDF), validation, and regulatory filing with automatic retry logic and exponential backoff. The system ensures compliance with Nigerian Financial Intelligence Unit (NFIU) requirements.

## Features

- ✅ Generate NFIU-compliant XML documents
- ✅ Generate PDF for internal records
- ✅ Validate CTR data before filing
- ✅ Submit to NFIU with retry logic (exponential backoff)
- ✅ Record submission details and track status
- ✅ Handle rejection and failure scenarios
- ✅ Maximum 5 retry attempts with configurable delays

## API Endpoints

### 1. Generate CTR Documents

**POST** `/api/admin/compliance/ctrs/:ctr_id/generate`

Generate NFIU-compliant XML and PDF documents for an approved CTR.

#### Path Parameters

- `ctr_id` (UUID): The CTR ID

#### Response (Success - 200 OK)

```json
{
  "success": true,
  "data": {
    "ctr_id": "123e4567-e89b-12d3-a456-426614174000",
    "xml_size": 4523,
    "pdf_url": "/documents/ctrs/ctr_123e4567-e89b-12d3-a456-426614174000.pdf",
    "generated_at": "2024-04-20T14:30:00Z",
    "message": "CTR documents generated successfully"
  }
}
```

#### Response (Error - CTR Not Approved)

```json
{
  "success": false,
  "error": "CTR must be in Approved status to generate documents. Current status: UnderReview"
}
```

#### Example (cURL)

```bash
curl -X POST https://api.example.com/api/admin/compliance/ctrs/123e4567-e89b-12d3-a456-426614174000/generate \
  -H "Authorization: Bearer YOUR_TOKEN"
```

---

### 2. Get CTR Document

**GET** `/api/admin/compliance/ctrs/:ctr_id/document`

Retrieve the generated XML document for a CTR.

#### Path Parameters

- `ctr_id` (UUID): The CTR ID

#### Response (Success - 200 OK)

Returns XML content with appropriate headers:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<CTR xmlns="http://nfiu.gov.ng/ctr/v1">
  <CtrId>123e4567-e89b-12d3-a456-426614174000</CtrId>
  <ReportingPeriod>2024-04-15T00:00:00Z</ReportingPeriod>
  <SubjectType>individual</SubjectType>
  <Subject>
    <FullName>John Doe</FullName>
    <IdentificationNumber>KYC-987fcdeb</IdentificationNumber>
    <Address>123 Main St, Lagos</Address>
    <KycId>987fcdeb-51a2-43f7-8b9c-123456789abc</KycId>
  </Subject>
  <Transactions>
    <Transaction>
      <TransactionId>tx1-uuid</TransactionId>
      <Timestamp>2024-04-15T10:30:00Z</Timestamp>
      <Type>onramp</Type>
      <AmountNGN>500000.00</AmountNGN>
      <Direction>credit</Direction>
      <Counterparty>Provider: Paystack, From: NGN, To: CNGN</Counterparty>
    </Transaction>
  </Transactions>
  <TotalAmount>5500000.00</TotalAmount>
  <TransactionCount>12</TransactionCount>
  <DetectionMethod>automatic</DetectionMethod>
  <FilingInstitution>
    <Name>Bitmesh Financial Services</Name>
    <RegistrationNumber>RC-123456</RegistrationNumber>
    <ContactEmail>compliance@bitmesh.com</ContactEmail>
  </FilingInstitution>
  <SubmissionTimestamp>2024-04-20T14:30:00Z</SubmissionTimestamp>
</CTR>
```

**Headers:**
- `Content-Type: application/xml`
- `Content-Disposition: attachment; filename="ctr_{ctr_id}.xml"`

#### Response (Error - Document Not Found)

```json
{
  "success": false,
  "error": "CTR document not found. Generate documents first."
}
```

#### Example (cURL)

```bash
curl -X GET https://api.example.com/api/admin/compliance/ctrs/123e4567-e89b-12d3-a456-426614174000/document \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -o ctr_document.xml
```

---

### 3. File CTR with NFIU

**POST** `/api/admin/compliance/ctrs/:ctr_id/file`

File CTR with NFIU. Includes validation, automatic document generation (if needed), and retry logic with exponential backoff.

#### Path Parameters

- `ctr_id` (UUID): The CTR ID

#### Response (Success - 200 OK)

```json
{
  "success": true,
  "data": {
    "ctr_id": "123e4567-e89b-12d3-a456-426614174000",
    "filing_id": "filing-uuid",
    "submission_reference": "NFIU-2024-04-20-12345",
    "submission_timestamp": "2024-04-20T14:35:00Z",
    "status": "Submitted",
    "retry_count": 0,
    "message": "CTR filed successfully with NFIU"
  }
}
```

#### Response (Success After Retries)

```json
{
  "success": true,
  "data": {
    "ctr_id": "123e4567-e89b-12d3-a456-426614174000",
    "filing_id": "filing-uuid",
    "submission_reference": "NFIU-2024-04-20-12346",
    "submission_timestamp": "2024-04-20T14:36:15Z",
    "status": "Submitted",
    "retry_count": 3,
    "message": "CTR filed successfully with NFIU"
  }
}
```

#### Response (Error - Validation Failed)

```json
{
  "success": false,
  "error": "CTR validation failed: 2 errors"
}
```

#### Response (Error - Max Retries Exceeded)

```json
{
  "success": false,
  "error": "Failed to file CTR after 5 attempts: Connection timeout"
}
```

#### Example (cURL)

```bash
curl -X POST https://api.example.com/api/admin/compliance/ctrs/123e4567-e89b-12d3-a456-426614174000/file \
  -H "Authorization: Bearer YOUR_TOKEN"
```

---

## Workflow

### Complete Filing Workflow

```
1. CTR Approved
         ↓
2. Generate Documents (POST /generate)
         ↓
3. Review Documents (GET /document)
         ↓
4. File with NFIU (POST /file)
    ├─ Validate CTR
    ├─ Submit to NFIU
    ├─ Retry on Failure (exponential backoff)
    └─ Record Submission
         ↓
5. CTR Status: Filed
```

### Retry Logic

```
Attempt 1: Immediate
         ↓ (fails)
Attempt 2: Wait 2 seconds
         ↓ (fails)
Attempt 3: Wait 4 seconds
         ↓ (fails)
Attempt 4: Wait 8 seconds
         ↓ (fails)
Attempt 5: Wait 16 seconds
         ↓ (fails)
Max Retries Reached → Filing Failed
```

---

## Validation Rules

Before filing, the system validates:

1. **Subject Information**
   - Full name is not empty
   - Identification number is not empty
   - Address is not empty

2. **Transaction Data**
   - At least one transaction exists
   - Total amount is greater than zero

3. **CTR Status**
   - Must be in "Approved" status

4. **Filing Deadline**
   - Warning if deadline has passed (doesn't block filing)

### Validation Error Response

```json
{
  "success": false,
  "error": "CTR validation failed: 2 errors",
  "validation_errors": [
    {
      "field": "subject_address",
      "message": "Subject address is required"
    },
    {
      "field": "transaction_count",
      "message": "At least one transaction is required"
    }
  ]
}
```

---

## NFIU XML Format

### XML Structure

```xml
<?xml version="1.0" encoding="UTF-8"?>
<CTR xmlns="http://nfiu.gov.ng/ctr/v1">
  <CtrId>UUID</CtrId>
  <ReportingPeriod>ISO8601 DateTime</ReportingPeriod>
  <SubjectType>individual|corporate</SubjectType>
  <Subject>
    <FullName>string</FullName>
    <IdentificationNumber>string</IdentificationNumber>
    <Address>string</Address>
    <KycId>UUID</KycId>
  </Subject>
  <Transactions>
    <Transaction>
      <TransactionId>UUID</TransactionId>
      <Timestamp>ISO8601 DateTime</Timestamp>
      <Type>string</Type>
      <AmountNGN>decimal</AmountNGN>
      <Direction>credit|debit</Direction>
      <Counterparty>string</Counterparty>
    </Transaction>
    <!-- More transactions -->
  </Transactions>
  <TotalAmount>decimal</TotalAmount>
  <TransactionCount>integer</TransactionCount>
  <DetectionMethod>automatic|manual</DetectionMethod>
  <FilingInstitution>
    <Name>string</Name>
    <RegistrationNumber>string</RegistrationNumber>
    <ContactEmail>email</ContactEmail>
  </FilingInstitution>
  <SubmissionTimestamp>ISO8601 DateTime</SubmissionTimestamp>
</CTR>
```

### XML Special Character Escaping

The system automatically escapes XML special characters:
- `&` → `&amp;`
- `<` → `&lt;`
- `>` → `&gt;`
- `"` → `&quot;`
- `'` → `&apos;`

---

## Configuration

### Filing Service Configuration

```rust
use Bitmesh_backend::aml::{CtrFilingConfig, CtrFilingService};

let config = CtrFilingConfig {
    nfiu_api_endpoint: "https://api.nfiu.gov.ng/ctr/submit".to_string(),
    nfiu_api_key: "your-api-key".to_string(),
    max_retry_attempts: 5,
    initial_retry_delay_secs: 2,
    max_retry_delay_secs: 300,  // 5 minutes
    request_timeout_secs: 30,
};

let service = CtrFilingService::new(pool, config);
```

### Custom Retry Configuration

```rust
let config = CtrFilingConfig {
    nfiu_api_endpoint: "https://api.nfiu.gov.ng/ctr/submit".to_string(),
    nfiu_api_key: "your-api-key".to_string(),
    max_retry_attempts: 10,           // More retries
    initial_retry_delay_secs: 1,      // Faster initial retry
    max_retry_delay_secs: 600,        // 10 minutes max
    request_timeout_secs: 60,         // Longer timeout
};
```

---

## Database Schema

### ctr_documents Table

```sql
CREATE TABLE ctr_documents (
    ctr_id UUID PRIMARY KEY REFERENCES ctrs(ctr_id),
    xml_content TEXT NOT NULL,
    pdf_url TEXT,
    generated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_ctr_documents_generated ON ctr_documents(generated_at);
```

### ctr_filings Table (Already Exists)

```sql
CREATE TABLE ctr_filings (
    ctr_id UUID PRIMARY KEY REFERENCES ctrs(ctr_id),
    filing_method TEXT NOT NULL,
    submission_timestamp TIMESTAMPTZ NOT NULL,
    regulatory_submission_reference TEXT NOT NULL,
    acknowledgement_timestamp TIMESTAMPTZ,
    acknowledgement_reference TEXT,
    rejection_details TEXT
);

CREATE INDEX idx_ctr_filings_submission ON ctr_filings(submission_timestamp);
CREATE INDEX idx_ctr_filings_reference ON ctr_filings(regulatory_submission_reference);
```

---

## Error Handling

### Common Errors

1. **CTR Not Approved**
   ```json
   {
     "success": false,
     "error": "CTR must be in Approved status to generate documents"
   }
   ```

2. **Validation Failed**
   ```json
   {
     "success": false,
     "error": "CTR validation failed: 3 errors"
   }
   ```

3. **Document Not Found**
   ```json
   {
     "success": false,
     "error": "CTR document not found. Generate documents first."
   }
   ```

4. **NFIU API Error**
   ```json
   {
     "success": false,
     "error": "NFIU submission failed with status 500: Internal Server Error"
   }
   ```

5. **Max Retries Exceeded**
   ```json
   {
     "success": false,
     "error": "Failed to file CTR after 5 attempts: Connection timeout"
   }
   ```

---

## Retry Strategy

### Exponential Backoff

The system uses exponential backoff with a cap:

| Attempt | Delay | Cumulative Time |
|---------|-------|-----------------|
| 1       | 0s    | 0s              |
| 2       | 2s    | 2s              |
| 3       | 4s    | 6s              |
| 4       | 8s    | 14s             |
| 5       | 16s   | 30s             |

**Maximum delay:** 300 seconds (5 minutes)

### Retry Conditions

The system retries on:
- Network errors (connection timeout, DNS failure)
- HTTP 5xx errors (server errors)
- HTTP 429 (rate limiting)

The system does NOT retry on:
- HTTP 4xx errors (except 429)
- Validation errors
- Authentication errors

---

## Testing

### Integration Test Example

```rust
#[tokio::test]
async fn test_ctr_filing_workflow() {
    let pool = setup_test_db().await;
    let config = CtrFilingConfig::default();
    let service = CtrFilingService::new(pool.clone(), config);
    
    // Create and approve test CTR
    let ctr_id = create_approved_ctr(&pool).await;
    
    // Generate documents
    let documents = service.generate_documents(ctr_id).await.unwrap();
    assert!(!documents.xml_content.is_empty());
    assert!(documents.pdf_url.is_some());
    
    // File CTR
    let filing_result = service.file_ctr(ctr_id).await.unwrap();
    assert_eq!(filing_result.status, FilingStatus::Submitted);
    assert!(!filing_result.submission_reference.is_empty());
}
```

### Mock NFIU API

For testing without actual NFIU submission:

```rust
let config = CtrFilingConfig {
    nfiu_api_endpoint: "http://localhost:8080/mock-nfiu".to_string(),
    nfiu_api_key: "test-key".to_string(),
    ..Default::default()
};
```

---

## Security Considerations

1. **API Key Protection**
   - Store NFIU API key in secure environment variables
   - Never log or expose API keys
   - Rotate keys periodically

2. **Document Storage**
   - Store XML and PDF documents securely
   - Implement access controls
   - Consider encryption at rest

3. **Audit Trail**
   - Log all filing attempts
   - Record retry counts and failures
   - Track submission references

4. **Data Validation**
   - Validate all data before submission
   - Sanitize XML content
   - Escape special characters

---

## Monitoring & Alerts

### Key Metrics to Monitor

1. **Filing Success Rate**
   - Track successful vs failed filings
   - Alert on high failure rates

2. **Retry Counts**
   - Monitor average retry count
   - Alert on excessive retries

3. **Response Times**
   - Track NFIU API response times
   - Alert on timeouts

4. **Validation Failures**
   - Track validation error types
   - Alert on recurring issues

### Example Monitoring Query

```sql
-- Filing success rate (last 24 hours)
SELECT 
    COUNT(*) FILTER (WHERE status = 'submitted') as successful,
    COUNT(*) FILTER (WHERE status = 'failed') as failed,
    ROUND(100.0 * COUNT(*) FILTER (WHERE status = 'submitted') / COUNT(*), 2) as success_rate
FROM ctr_filings
WHERE submission_timestamp > NOW() - INTERVAL '24 hours';
```

---

## Best Practices

1. **Generate Documents Early**
   - Generate documents immediately after approval
   - Review documents before filing

2. **Monitor Filing Deadlines**
   - Track approaching deadlines
   - File well before deadline

3. **Handle Failures Gracefully**
   - Log all errors with context
   - Notify compliance team of failures
   - Implement manual retry process

4. **Validate Before Filing**
   - Always validate CTR data
   - Fix validation errors before attempting to file

5. **Keep Records**
   - Store all generated documents
   - Maintain complete audit trail
   - Archive filed CTRs

---

## Troubleshooting

### Issue: Documents Not Generating

**Symptoms:** POST /generate returns error

**Solutions:**
1. Verify CTR is in Approved status
2. Check that CTR has transactions
3. Verify database connectivity

### Issue: Filing Fails Repeatedly

**Symptoms:** Max retries exceeded

**Solutions:**
1. Check NFIU API endpoint configuration
2. Verify API key is valid
3. Check network connectivity
4. Review NFIU API status

### Issue: Validation Errors

**Symptoms:** Filing blocked by validation

**Solutions:**
1. Review validation error messages
2. Update CTR with missing information
3. Return CTR for correction if needed

---

## Compliance Notes

- CTRs must be filed within 15 days of the reporting period end
- All filed CTRs must be retained for at least 5 years
- Submission references must be recorded for audit purposes
- Failed filings must be investigated and refiled
- Document all filing attempts and outcomes
