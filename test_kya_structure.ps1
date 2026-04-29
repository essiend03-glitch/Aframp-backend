# KYA Module Structure Verification Script

Write-Host "=== KYA Module Structure Test ===" -ForegroundColor Cyan
Write-Host ""

# Check if all module files exist
Write-Host "Checking module files..." -ForegroundColor Yellow
$files = @(
    "src/kya/mod.rs",
    "src/kya/error.rs",
    "src/kya/models.rs",
    "src/kya/identity.rs",
    "src/kya/reputation.rs",
    "src/kya/attestation.rs",
    "src/kya/zkp.rs",
    "src/kya/scoring.rs",
    "src/kya/registry.rs",
    "src/kya/routes.rs",
    "src/kya/README.md"
)

$allExist = $true
foreach ($file in $files) {
    if (Test-Path $file) {
        Write-Host "✓ $file" -ForegroundColor Green
    } else {
        Write-Host "✗ $file (MISSING)" -ForegroundColor Red
        $allExist = $false
    }
}

Write-Host ""
Write-Host "Checking database schema..." -ForegroundColor Yellow
if (Test-Path "db/migrations/kya_schema.sql") {
    Write-Host "✓ db/migrations/kya_schema.sql" -ForegroundColor Green
} else {
    Write-Host "✗ db/migrations/kya_schema.sql (MISSING)" -ForegroundColor Red
    $allExist = $false
}

Write-Host ""
Write-Host "Checking test file..." -ForegroundColor Yellow
if (Test-Path "tests/kya_integration.rs") {
    Write-Host "✓ tests/kya_integration.rs" -ForegroundColor Green
} else {
    Write-Host "✗ tests/kya_integration.rs (MISSING)" -ForegroundColor Red
    $allExist = $false
}

Write-Host ""
Write-Host "Checking documentation..." -ForegroundColor Yellow
$docs = @(
    "KYA_IMPLEMENTATION.md",
    "KYA_QUICK_START.md",
    "KYA_COMPLETION_SUMMARY.md"
)

foreach ($doc in $docs) {
    if (Test-Path $doc) {
        Write-Host "✓ $doc" -ForegroundColor Green
    } else {
        Write-Host "✗ $doc (MISSING)" -ForegroundColor Red
        $allExist = $false
    }
}

Write-Host ""
Write-Host "=== Line Count Statistics ===" -ForegroundColor Cyan
Write-Host ""

Write-Host "Source code files:" -ForegroundColor Yellow
$sourceFiles = Get-ChildItem -Path "src/kya" -Filter "*.rs" -Recurse
$totalSourceLines = 0
foreach ($file in $sourceFiles) {
    $lines = (Get-Content $file.FullName | Measure-Object -Line).Lines
    $totalSourceLines += $lines
    Write-Host "  $($file.Name): $lines lines"
}
Write-Host "  Total: $totalSourceLines lines" -ForegroundColor White

Write-Host ""
Write-Host "Database schema:" -ForegroundColor Yellow
$schemaLines = (Get-Content "db/migrations/kya_schema.sql" | Measure-Object -Line).Lines
Write-Host "  kya_schema.sql: $schemaLines lines"

Write-Host ""
Write-Host "Tests:" -ForegroundColor Yellow
$testLines = (Get-Content "tests/kya_integration.rs" | Measure-Object -Line).Lines
Write-Host "  kya_integration.rs: $testLines lines"

Write-Host ""
Write-Host "Documentation:" -ForegroundColor Yellow
$totalDocLines = 0
foreach ($doc in $docs) {
    if (Test-Path $doc) {
        $lines = (Get-Content $doc | Measure-Object -Line).Lines
        $totalDocLines += $lines
        Write-Host "  $doc`: $lines lines"
    }
}
Write-Host "  Total: $totalDocLines lines" -ForegroundColor White

Write-Host ""
Write-Host "=== Summary ===" -ForegroundColor Cyan
Write-Host "Total implementation: $($totalSourceLines + $schemaLines + $testLines) lines"
Write-Host "Total documentation: $totalDocLines lines"
Write-Host "Grand total: $($totalSourceLines + $schemaLines + $testLines + $totalDocLines) lines"

Write-Host ""
if ($allExist) {
    Write-Host "✅ All files present - Structure verification PASSED" -ForegroundColor Green
    exit 0
} else {
    Write-Host "❌ Some files missing - Structure verification FAILED" -ForegroundColor Red
    exit 1
}
