#!/bin/bash
# KYA Module Structure Verification Script

echo "=== KYA Module Structure Test ==="
echo ""

# Check if all module files exist
echo "Checking module files..."
files=(
    "src/kya/mod.rs"
    "src/kya/error.rs"
    "src/kya/models.rs"
    "src/kya/identity.rs"
    "src/kya/reputation.rs"
    "src/kya/attestation.rs"
    "src/kya/zkp.rs"
    "src/kya/scoring.rs"
    "src/kya/registry.rs"
    "src/kya/routes.rs"
    "src/kya/README.md"
)

all_exist=true
for file in "${files[@]}"; do
    if [ -f "$file" ]; then
        echo "✓ $file"
    else
        echo "✗ $file (MISSING)"
        all_exist=false
    fi
done

echo ""
echo "Checking database schema..."
if [ -f "db/migrations/kya_schema.sql" ]; then
    echo "✓ db/migrations/kya_schema.sql"
else
    echo "✗ db/migrations/kya_schema.sql (MISSING)"
    all_exist=false
fi

echo ""
echo "Checking test file..."
if [ -f "tests/kya_integration.rs" ]; then
    echo "✓ tests/kya_integration.rs"
else
    echo "✗ tests/kya_integration.rs (MISSING)"
    all_exist=false
fi

echo ""
echo "Checking documentation..."
docs=(
    "KYA_IMPLEMENTATION.md"
    "KYA_QUICK_START.md"
    "KYA_COMPLETION_SUMMARY.md"
)

for doc in "${docs[@]}"; do
    if [ -f "$doc" ]; then
        echo "✓ $doc"
    else
        echo "✗ $doc (MISSING)"
        all_exist=false
    fi
done

echo ""
echo "=== Line Count Statistics ==="
echo ""
echo "Source code:"
find src/kya -name "*.rs" -exec wc -l {} + | tail -1

echo ""
echo "Database schema:"
wc -l db/migrations/kya_schema.sql

echo ""
echo "Tests:"
wc -l tests/kya_integration.rs

echo ""
echo "Documentation:"
wc -l KYA_*.md | tail -1

echo ""
if [ "$all_exist" = true ]; then
    echo "✅ All files present - Structure verification PASSED"
    exit 0
else
    echo "❌ Some files missing - Structure verification FAILED"
    exit 1
fi
