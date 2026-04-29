#!/bin/bash
# Setup script for Append-Only Audit Ledger

set -e

echo "=== Append-Only Audit Ledger Setup ==="
echo ""

# Check if DATABASE_URL is set
if [ -z "$DATABASE_URL" ]; then
    echo "Error: DATABASE_URL environment variable is not set"
    echo "Example: export DATABASE_URL=postgres://user:password@localhost:5432/aframp"
    exit 1
fi

echo "1. Checking database connection..."
psql "$DATABASE_URL" -c "SELECT 1" > /dev/null 2>&1
if [ $? -eq 0 ]; then
    echo "   ✓ Database connection successful"
else
    echo "   ✗ Database connection failed"
    exit 1
fi

echo ""
echo "2. Running audit ledger migration..."
psql "$DATABASE_URL" -f migrations/20270424000000_append_only_audit_ledger.sql
if [ $? -eq 0 ]; then
    echo "   ✓ Migration completed successfully"
else
    echo "   ✗ Migration failed"
    exit 1
fi

echo ""
echo "3. Verifying tables..."
TABLES=$(psql "$DATABASE_URL" -t -c "SELECT COUNT(*) FROM information_schema.tables WHERE table_name IN ('audit_ledger', 'audit_anchors')")
if [ "$TABLES" -eq 2 ]; then
    echo "   ✓ Tables created successfully"
else
    echo "   ✗ Table creation verification failed"
    exit 1
fi

echo ""
echo "4. Verifying genesis entry..."
GENESIS=$(psql "$DATABASE_URL" -t -c "SELECT COUNT(*) FROM audit_ledger WHERE sequence = 0")
if [ "$GENESIS" -eq 1 ]; then
    echo "   ✓ Genesis entry exists"
else
    echo "   ✗ Genesis entry not found"
    exit 1
fi

echo ""
echo "5. Testing WORM enforcement..."
psql "$DATABASE_URL" -c "UPDATE audit_ledger SET actor_id = 'test' WHERE sequence = 0" > /dev/null 2>&1
if [ $? -ne 0 ]; then
    echo "   ✓ WORM enforcement working (UPDATE blocked)"
else
    echo "   ✗ WORM enforcement failed (UPDATE allowed)"
    exit 1
fi

psql "$DATABASE_URL" -c "DELETE FROM audit_ledger WHERE sequence = 0" > /dev/null 2>&1
if [ $? -ne 0 ]; then
    echo "   ✓ WORM enforcement working (DELETE blocked)"
else
    echo "   ✗ WORM enforcement failed (DELETE allowed)"
    exit 1
fi

echo ""
echo "6. Verifying indexes..."
INDEXES=$(psql "$DATABASE_URL" -t -c "SELECT COUNT(*) FROM pg_indexes WHERE tablename = 'audit_ledger'")
if [ "$INDEXES" -ge 7 ]; then
    echo "   ✓ Indexes created successfully"
else
    echo "   ⚠ Warning: Expected at least 7 indexes, found $INDEXES"
fi

echo ""
echo "7. Verifying views..."
VIEWS=$(psql "$DATABASE_URL" -t -c "SELECT COUNT(*) FROM information_schema.views WHERE table_name IN ('audit_trail_summary', 'recent_audit_events')")
if [ "$VIEWS" -eq 2 ]; then
    echo "   ✓ Views created successfully"
else
    echo "   ⚠ Warning: Expected 2 views, found $VIEWS"
fi

echo ""
echo "8. Testing chain verification function..."
psql "$DATABASE_URL" -c "SELECT * FROM verify_audit_chain(0, NULL)" > /dev/null 2>&1
if [ $? -eq 0 ]; then
    echo "   ✓ Chain verification function working"
else
    echo "   ✗ Chain verification function failed"
    exit 1
fi

echo ""
echo "=== Setup Complete ==="
echo ""
echo "Next steps:"
echo "1. Configure Stellar anchoring in config/audit_ledger.toml"
echo "2. Set STELLAR_ANCHOR_SECRET environment variable"
echo "3. Start the application with audit ledger enabled"
echo ""
echo "Verify setup:"
echo "  psql \$DATABASE_URL -c 'SELECT * FROM recent_audit_events'"
echo "  psql \$DATABASE_URL -c 'SELECT * FROM verify_audit_chain(0, NULL)'"
echo ""
