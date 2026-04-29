#!/bin/bash
# Verify Audit Chain Integrity

set -e

echo "=== Audit Chain Verification ==="
echo ""

# Check if DATABASE_URL is set
if [ -z "$DATABASE_URL" ]; then
    echo "Error: DATABASE_URL environment variable is not set"
    exit 1
fi

# Get chain statistics
echo "1. Chain Statistics:"
TOTAL_ENTRIES=$(psql "$DATABASE_URL" -t -c "SELECT COUNT(*) FROM audit_ledger")
echo "   Total entries: $TOTAL_ENTRIES"

FIRST_SEQ=$(psql "$DATABASE_URL" -t -c "SELECT MIN(sequence) FROM audit_ledger")
LAST_SEQ=$(psql "$DATABASE_URL" -t -c "SELECT MAX(sequence) FROM audit_ledger")
echo "   Sequence range: $FIRST_SEQ to $LAST_SEQ"

OLDEST=$(psql "$DATABASE_URL" -t -c "SELECT MIN(timestamp) FROM audit_ledger")
NEWEST=$(psql "$DATABASE_URL" -t -c "SELECT MAX(timestamp) FROM audit_ledger")
echo "   Time range: $OLDEST to $NEWEST"

echo ""
echo "2. Verifying Hash Chain..."
VERIFICATION=$(psql "$DATABASE_URL" -t -c "SELECT valid, broken_at, reason FROM verify_audit_chain(0, NULL)")
VALID=$(echo "$VERIFICATION" | awk '{print $1}')

if [ "$VALID" = "t" ]; then
    echo "   ✓ Chain is VALID"
    echo "   All $TOTAL_ENTRIES entries verified successfully"
else
    echo "   ✗ Chain is BROKEN!"
    echo "   $VERIFICATION"
    exit 1
fi

echo ""
echo "3. Anchor Status:"
TOTAL_ANCHORS=$(psql "$DATABASE_URL" -t -c "SELECT COUNT(*) FROM audit_anchors")
echo "   Total anchors: $TOTAL_ANCHORS"

if [ "$TOTAL_ANCHORS" -gt 0 ]; then
    VERIFIED_ANCHORS=$(psql "$DATABASE_URL" -t -c "SELECT COUNT(*) FROM audit_anchors WHERE verified = true")
    echo "   Verified anchors: $VERIFIED_ANCHORS"
    
    LAST_ANCHOR=$(psql "$DATABASE_URL" -t -c "SELECT anchor_timestamp FROM audit_anchors ORDER BY anchor_timestamp DESC LIMIT 1")
    echo "   Last anchor: $LAST_ANCHOR"
    
    LAST_STELLAR_TX=$(psql "$DATABASE_URL" -t -c "SELECT stellar_transaction_id FROM audit_anchors WHERE stellar_transaction_id IS NOT NULL ORDER BY anchor_timestamp DESC LIMIT 1")
    if [ -n "$LAST_STELLAR_TX" ]; then
        echo "   Last Stellar TX: $LAST_STELLAR_TX"
    fi
fi

echo ""
echo "4. Recent Activity:"
echo "   Last 5 entries:"
psql "$DATABASE_URL" -c "SELECT sequence, actor_type, action_type, object_type, result, timestamp FROM audit_ledger ORDER BY sequence DESC LIMIT 5"

echo ""
echo "5. Activity Summary (Last 24 Hours):"
psql "$DATABASE_URL" -c "SELECT actor_type, action_type, COUNT(*) as count FROM audit_ledger WHERE timestamp >= NOW() - INTERVAL '24 hours' GROUP BY actor_type, action_type ORDER BY count DESC LIMIT 10"

echo ""
echo "=== Verification Complete ==="
