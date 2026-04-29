# CTR Management API Documentation

## Overview

The CTR Management API provides endpoints for reviewing, approving, and managing Currency Transaction Reports (CTRs). The system enforces a mandatory review checklist and requires senior officer approval for high-value CTRs above a configurable threshold (default: NGN 50M).

## Features

- ✅ List all CTRs with optional status filtering
- ✅ Get detailed CTR information including transactions, reviews, and approvals
- ✅ Review CTRs with mandatory checklist enforcement
- ✅ Approve CTRs with automatic senior approval requirement for high-value reports
- ✅ Return CTRs for correction with issue tracking
- ✅ Query CTRs requiring senior approval

## API Endpoints

### 1. Get All CTRs

**GET** `/api/admin/compliance/ctrs`

Retrieve all CTRs with optional status filtering.

#### Query Parameters

- `status` (string, optional): Filter by CTR status
  - Valid values: `draft`, `under_review`, `approved`, `filed`, `acknowledged`, `rejected`

#### Response (Success - 200 OK)

```json
{
  "success": true,
  "data": {
    "ctrs": [
      {
        "ctr_id": "123e4567-e89b-12d3-a456-426614174000",
        "reporting_period": "2024-04-15T00:00:00Z",
        "ctr_type": "individual",
        "subject_kyc_id": "987fcdeb-51a2-43f7-8b9c-123456789abc",
        "subject_full_name": "John Doe",
        "subject_identification": "KYC-987fcdeb",
        "subject_address": "123 Main St, Lagos",
        "total_transaction_amount": "5500000.00",
        "transaction_count": 12,
        "transaction_references": ["tx1", "tx2", "..."],
        "detection_method": "automatic",
        "status": "under_review",
        "assigned_compliance_officer": "officer-uuid",
        "filing_timestamp": "2024-04-30T23:59:59Z",
        "regulatory_reference_number": null
      }
    ],
    "total_count": 1
  }
}
```

#### Example (cURL)

```bash
# Get all CTRs
curl -X GET https://api.example.com/api/admin/compliance/ctrs \
  -H "Authorization: Bearer YOUR_TOKEN"

# Get CTRs in under_review status
curl -X GET "https://api.example.com/api/admin/compliance/ctrs?status=under_review" \
  -H "Authorization: Bearer YOUR_TOKEN"
```

---

### 2. Get CTR by ID

**GET** `/api/admin/compliance/ctrs/:ctr_id`

Get detailed information about a specific CTR including all transactions, reviews, and approvals.

#### Path Parameters

- `ctr_id` (UUID): The CTR ID

#### Response (Success - 200 OK)

```json
{
  "success": true,
  "data": {
    "ctr": {
      "ctr_id": "123e4567-e89b-12d3-a456-426614174000",
      "reporting_period": "2024-04-15T00:00:00Z",
      "ctr_type": "individual",
      "subject_kyc_id": "987fcdeb-51a2-43f7-8b9c-123456789abc",
      "subject_full_name": "John Doe",
      "subject_identification": "KYC-987fcdeb",
      "subject_address": "123 Main St, Lagos",
      "total_transaction_amount": "5500000.00",
      "transaction_count": 12,
      "transaction_references": ["tx1", "tx2"],
      "detection_method": "automatic",
      "status": "under_review",
      "assigned_compliance_officer": "officer-uuid",
      "filing_timestamp": "2024-04-30T23:59:59Z",
      "regulatory_reference_number": null
    },
    "transactions": [
      {
        "ctr_id": "123e4567-e89b-12d3-a456-426614174000",
        "transaction_id": "tx1-uuid",
        "transaction_timestamp": "2024-04-15T10:30:00Z",
        "transaction_type": "onramp",
        "transaction_amount_ngn": "500000.00",
        "counterparty_details": "Provider: Paystack, From: NGN, To: CNGN",
        "direction": "credit"
      }
    ],
    "reviews": [
      {
        "id": "review-uuid",
        "ctr_id": "123e4567-e89b-12d3-a456-426614174000",
        "reviewer_id": "reviewer-uuid",
        "checklist": {
          "subject_identity_verified": true,
          "transaction_details_accurate": true,
          "amounts_reconciled": true,
          "supporting_documents_attached": true,
          "suspicious_activity_noted": false,
          "regulatory_requirements_met": true
        },
        "review_notes": "All checks passed",
        "reviewed_at": "2024-04-16T14:20:00Z"
      }
    ],
    "approvals": [],
    "requires_senior_approval": false
  }
}
```

#### Example (cURL)

```bash
curl -X GET https://api.example.com/api/admin/compliance/ctrs/123e4567-e89b-12d3-a456-426614174000 \
  -H "Authorization: Bearer YOUR_TOKEN"
```

---

### 3. Review CTR

**POST** `/api/admin/compliance/ctrs/:ctr_id/review`

Review a CTR with mandatory checklist. The checklist must be complete to proceed to approval (if enforcement is enabled).

#### Path Parameters

- `ctr_id` (UUID): The CTR ID

#### Request Body

```json
{
  "reviewer_id": "reviewer-uuid",
  "checklist": {
    "subject_identity_verified": true,
    "transaction_details_accurate": true,
    "amounts_reconciled": true,
    "supporting_documents_attached": true,
    "suspicious_activity_noted": false,
    "regulatory_requirements_met": true
  },
  "review_notes": "All verification checks completed successfully"
}
```

**Checklist Fields (all boolean):**
- `subject_identity_verified` (required): Subject identity has been verified
- `transaction_details_accurate` (required): Transaction details are accurate
- `amounts_reconciled` (required): Amounts have been reconciled
- `supporting_documents_attached` (required): Supporting documents are attached
- `suspicious_activity_noted` (optional): Suspicious activity has been noted
- `regulatory_requirements_met` (required): All regulatory requirements are met

#### Response (Success - 200 OK)

```json
{
  "success": true,
  "data": {
    "ctr_id": "123e4567-e89b-12d3-a456-426614174000",
    "review_id": "review-uuid",
    "checklist_complete": true,
    "incomplete_items": [],
    "can_proceed_to_approval": true,
    "message": "CTR reviewed successfully. Ready for approval."
  }
}
```

#### Response (Checklist Incomplete)

```json
{
  "success": true,
  "data": {
    "ctr_id": "123e4567-e89b-12d3-a456-426614174000",
    "review_id": "review-uuid",
    "checklist_complete": false,
    "incomplete_items": [
      "Transaction details accuracy check",
      "Supporting documents attachment"
    ],
    "can_proceed_to_approval": false,
    "message": "CTR reviewed but checklist is incomplete."
  }
}
```

#### Example (cURL)

```bash
curl -X POST https://api.example.com/api/admin/compliance/ctrs/123e4567-e89b-12d3-a456-426614174000/review \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -d '{
    "reviewer_id": "reviewer-uuid",
    "checklist": {
      "subject_identity_verified": true,
      "transaction_details_accurate": true,
      "amounts_reconciled": true,
      "supporting_documents_attached": true,
      "suspicious_activity_noted": false,
      "regulatory_requirements_met": true
    },
    "review_notes": "All checks passed"
  }'
```

---

### 4. Approve CTR

**POST** `/api/admin/compliance/ctrs/:ctr_id/approve`

Approve a CTR. High-value CTRs (above threshold) require senior officer approval.

#### Path Parameters

- `ctr_id` (UUID): The CTR ID

#### Request Body

```json
{
  "approver_id": "approver-uuid",
  "approval_level": "senior",
  "approval_notes": "Approved for filing"
}
```

**Fields:**
- `approver_id` (UUID, required): ID of the approving officer
- `approval_level` (string, required): Approval level - `"standard"` or `"senior"`
- `approval_notes` (string, optional): Notes about the approval

#### Response (Success - 200 OK)

```json
{
  "success": true,
  "data": {
    "ctr_id": "123e4567-e89b-12d3-a456-426614174000",
    "approval_id": "approval-uuid",
    "requires_senior_approval": false,
    "senior_approval_received": false,
    "can_proceed_to_filing": true,
    "message": "CTR approved successfully. Ready for filing."
  }
}
```

#### Response (Senior Approval Required)

```json
{
  "success": true,
  "data": {
    "ctr_id": "123e4567-e89b-12d3-a456-426614174000",
    "approval_id": "approval-uuid",
    "requires_senior_approval": true,
    "senior_approval_received": false,
    "can_proceed_to_filing": false,
    "message": "CTR approval recorded. Senior officer approval still required."
  }
}
```

#### Response (Error - Senior Approval Required)

```json
{
  "success": false,
  "error": "CTR amount 55000000.00 exceeds threshold 50000000.00. Senior officer approval required."
}
```

#### Example (cURL)

```bash
# Standard approval
curl -X POST https://api.example.com/api/admin/compliance/ctrs/123e4567-e89b-12d3-a456-426614174000/approve \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -d '{
    "approver_id": "approver-uuid",
    "approval_level": "standard",
    "approval_notes": "Approved"
  }'

# Senior approval
curl -X POST https://api.example.com/api/admin/compliance/ctrs/123e4567-e89b-12d3-a456-426614174000/approve \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -d '{
    "approver_id": "senior-officer-uuid",
    "approval_level": "senior",
    "approval_notes": "Senior approval granted"
  }'
```

---

### 5. Return for Correction

**POST** `/api/admin/compliance/ctrs/:ctr_id/return-for-correction`

Return a CTR for correction. The CTR status will be set back to Draft.

#### Path Parameters

- `ctr_id` (UUID): The CTR ID

#### Request Body

```json
{
  "reviewer_id": "reviewer-uuid",
  "correction_notes": "Subject address needs to be updated with complete information",
  "issues": [
    "Incomplete subject address",
    "Missing transaction reference for TX-12345"
  ]
}
```

**Fields:**
- `reviewer_id` (UUID, required): ID of the reviewer returning the CTR
- `correction_notes` (string, required): Detailed notes about required corrections
- `issues` (array of strings, required): List of specific issues to be corrected

#### Response (Success - 200 OK)

```json
{
  "success": true,
  "data": {
    "ctr_id": "123e4567-e89b-12d3-a456-426614174000",
    "issues": [
      "Incomplete subject address",
      "Missing transaction reference for TX-12345"
    ],
    "message": "CTR returned for correction. Status set to Draft."
  }
}
```

#### Example (cURL)

```bash
curl -X POST https://api.example.com/api/admin/compliance/ctrs/123e4567-e89b-12d3-a456-426614174000/return-for-correction \
  -H "Content-Type: application/json" \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -d '{
    "reviewer_id": "reviewer-uuid",
    "correction_notes": "Address incomplete",
    "issues": ["Incomplete subject address"]
  }'
```

---

### 6. Get CTRs Requiring Senior Approval

**GET** `/api/admin/compliance/ctrs/senior-approval-required`

Get all CTRs that require senior approval (high-value CTRs without senior approval).

#### Response (Success - 200 OK)

```json
{
  "success": true,
  "data": {
    "ctrs": [
      {
        "ctr_id": "123e4567-e89b-12d3-a456-426614174000",
        "total_transaction_amount": "55000000.00",
        "status": "under_review",
        "subject_full_name": "ABC Corporation",
        "...": "..."
      }
    ],
    "total_count": 1
  }
}
```

#### Example (cURL)

```bash
curl -X GET https://api.example.com/api/admin/compliance/ctrs/senior-approval-required \
  -H "Authorization: Bearer YOUR_TOKEN"
```

---

## Workflow

### Standard CTR Workflow

```
1. CTR Auto-Generated (Status: Draft)
         ↓
2. Review with Checklist (Status: UnderReview)
         ↓
3. Approve (Status: Approved)
         ↓
4. File with Regulator (Status: Filed)
         ↓
5. Receive Acknowledgment (Status: Acknowledged)
```

### High-Value CTR Workflow (Above Threshold)

```
1. CTR Auto-Generated (Status: Draft)
         ↓
2. Review with Checklist (Status: UnderReview)
         ↓
3. Standard Approval (Status: UnderReview)
         ↓
4. Senior Approval Required ⚠️
         ↓
5. Senior Officer Approves (Status: Approved)
         ↓
6. File with Regulator (Status: Filed)
```

### Correction Workflow

```
At any point before Filing:
         ↓
Return for Correction (Status: Draft)
         ↓
Corrections Made
         ↓
Re-enter Review Process
```

---

## Configuration

### CTR Management Configuration

```rust
use Bitmesh_backend::aml::{CtrManagementConfig, CtrManagementService};
use rust_decimal::Decimal;
use std::str::FromStr;

let config = CtrManagementConfig {
    senior_approval_threshold: Decimal::from_str("50000000").unwrap(), // NGN 50M
    enforce_checklist: true,
};

let service = CtrManagementService::new(pool, config);
```

### Custom Threshold

```rust
let config = CtrManagementConfig {
    senior_approval_threshold: Decimal::from_str("100000000").unwrap(), // NGN 100M
    enforce_checklist: true,
};
```

### Disable Checklist Enforcement (Not Recommended)

```rust
let config = CtrManagementConfig {
    senior_approval_threshold: Decimal::from_str("50000000").unwrap(),
    enforce_checklist: false, // Allow approval without complete checklist
};
```

---

## Database Schema

### ctr_reviews Table

```sql
CREATE TABLE ctr_reviews (
    id UUID PRIMARY KEY,
    ctr_id UUID NOT NULL REFERENCES ctrs(ctr_id),
    reviewer_id UUID NOT NULL,
    checklist JSONB NOT NULL,
    review_notes TEXT,
    reviewed_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_ctr_reviews_ctr ON ctr_reviews(ctr_id);
CREATE INDEX idx_ctr_reviews_reviewer ON ctr_reviews(reviewer_id);
```

### ctr_approvals Table

```sql
CREATE TABLE ctr_approvals (
    id UUID PRIMARY KEY,
    ctr_id UUID NOT NULL REFERENCES ctrs(ctr_id),
    approver_id UUID NOT NULL,
    approval_level TEXT NOT NULL CHECK (approval_level IN ('standard', 'senior')),
    approval_notes TEXT,
    approved_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_ctr_approvals_ctr ON ctr_approvals(ctr_id);
CREATE INDEX idx_ctr_approvals_level ON ctr_approvals(ctr_id, approval_level);
```

### ctr_corrections Table

```sql
CREATE TABLE ctr_corrections (
    id UUID PRIMARY KEY,
    ctr_id UUID NOT NULL REFERENCES ctrs(ctr_id),
    reviewer_id UUID NOT NULL,
    correction_notes TEXT NOT NULL,
    issues TEXT[] NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_ctr_corrections_ctr ON ctr_corrections(ctr_id);
```

---

## Error Handling

### Common Errors

1. **CTR Not Found**
   ```json
   {
     "success": false,
     "error": "CTR not found"
   }
   ```

2. **Invalid Status for Operation**
   ```json
   {
     "success": false,
     "error": "CTR must be in Draft or UnderReview status to be reviewed"
   }
   ```

3. **Incomplete Checklist**
   ```json
   {
     "success": false,
     "error": "Cannot approve CTR: review checklist is incomplete"
   }
   ```

4. **Senior Approval Required**
   ```json
   {
     "success": false,
     "error": "CTR amount 55000000.00 exceeds threshold 50000000.00. Senior officer approval required."
   }
   ```

5. **No Review Record**
   ```json
   {
     "success": false,
     "error": "Cannot approve CTR: no review record found"
   }
   ```

---

## Security & Compliance

### Access Control

- All endpoints require admin authentication
- Review and approval actions should be logged for audit trail
- Reviewer and approver IDs should be validated against authorized personnel

### Mandatory Checklist

The review checklist enforces:
- Subject identity verification
- Transaction details accuracy
- Amount reconciliation
- Supporting documentation
- Regulatory compliance

### Senior Approval Threshold

High-value CTRs require additional oversight:
- Default threshold: NGN 50,000,000
- Configurable per deployment
- Prevents single-officer approval of large reports
- Ensures proper escalation

---

## Testing

### Integration Test Example

```rust
#[tokio::test]
async fn test_ctr_review_and_approval_workflow() {
    let pool = setup_test_db().await;
    let config = CtrManagementConfig::default();
    let service = CtrManagementService::new(pool.clone(), config);
    
    // Create test CTR
    let ctr_id = create_test_ctr(&pool, Decimal::from_str("5000000").unwrap()).await;
    
    // Review CTR
    let review_result = service.review_ctr(
        ctr_id,
        ReviewCtrRequest {
            reviewer_id: Uuid::new_v4(),
            checklist: ReviewChecklist {
                subject_identity_verified: true,
                transaction_details_accurate: true,
                amounts_reconciled: true,
                supporting_documents_attached: true,
                suspicious_activity_noted: false,
                regulatory_requirements_met: true,
            },
            review_notes: Some("All checks passed".to_string()),
        },
    ).await.unwrap();
    
    assert!(review_result.checklist_complete);
    assert!(review_result.can_proceed_to_approval);
    
    // Approve CTR
    let approval_result = service.approve_ctr(
        ctr_id,
        ApproveCtrRequest {
            approver_id: Uuid::new_v4(),
            approval_level: "standard".to_string(),
            approval_notes: Some("Approved".to_string()),
        },
    ).await.unwrap();
    
    assert!(approval_result.can_proceed_to_filing);
}
```

---

## Best Practices

1. **Always Complete Checklist**: Ensure all mandatory items are verified before approval
2. **Document Decisions**: Use notes fields to document reasoning
3. **Senior Approval**: High-value CTRs should always be reviewed by senior officers
4. **Timely Filing**: Monitor filing deadlines and file approved CTRs promptly
5. **Audit Trail**: Maintain complete records of all reviews and approvals
6. **Correction Tracking**: Document all issues when returning for correction
