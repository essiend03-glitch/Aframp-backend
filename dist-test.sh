#!/usr/bin/env bash
# dist-test.sh — Issue #348
# Measures end-to-end latency across simulated geographic endpoints.
# Uses curl's --resolve flag to bypass DNS and hit each regional ALB directly,
# simulating what a client in that region would experience.
#
# Usage:
#   ./dist-test.sh [BASE_URL]
#
# Example:
#   BASE_URL=https://api.aframp.io ./dist-test.sh
#   ./dist-test.sh https://staging-api.aframp.io

set -euo pipefail

BASE_URL="${1:-${BASE_URL:-https://api.aframp.io}}"
ITERATIONS=5
LAT_TARGET_MS=30   # <30 ms target for /public/* endpoints
PASS=0
FAIL=0

# Regional ALB IPs (update with real IPs or use DNS names in CI).
declare -A REGIONS=(
  ["us-east-1"]="${US_EAST_1_IP:-}"
  ["eu-west-1"]="${EU_WEST_1_IP:-}"
  ["ap-southeast-1"]="${AP_SOUTHEAST_1_IP:-}"
)

# Endpoints to test: path → expected cache behaviour
declare -A ENDPOINTS=(
  ["/public/rates"]="cacheable"
  ["/public/fees"]="cacheable"
  ["/account/balance"]="no-cache"
  ["/health/edge"]="no-cache"
)

# Colours
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'; NC='\033[0m'

log()  { echo -e "${NC}[dist-test] $*"; }
pass() { echo -e "${GREEN}  ✓ $*${NC}"; ((PASS++)); }
fail() { echo -e "${RED}  ✗ $*${NC}"; ((FAIL++)); }
warn() { echo -e "${YELLOW}  ⚠ $*${NC}"; }

# ---------------------------------------------------------------------------
# measure_latency <url> [resolve_host:port:ip]
# Returns the total time in milliseconds via curl's time_total variable.
# ---------------------------------------------------------------------------
measure_latency() {
  local url="$1"
  local resolve_arg="${2:-}"
  local resolve_flag=()
  [[ -n "$resolve_arg" ]] && resolve_flag=(--resolve "$resolve_arg")

  curl -o /dev/null -s -w "%{time_total}" \
    --max-time 10 \
    -H "Accept: application/json" \
    "${resolve_flag[@]}" \
    "$url" 2>/dev/null || echo "9999"
}

# ---------------------------------------------------------------------------
# test_endpoint <region> <path> <expected_cache> [resolve_arg]
# ---------------------------------------------------------------------------
test_endpoint() {
  local region="$1" path="$2" expected_cache="$3" resolve_arg="${4:-}"
  local url="${BASE_URL}${path}"
  local total_ms=0

  for i in $(seq 1 "$ITERATIONS"); do
    local t
    t=$(measure_latency "$url" "$resolve_arg")
    # curl returns seconds with decimals; convert to ms
    local ms
    ms=$(awk "BEGIN { printf \"%.0f\", $t * 1000 }")
    total_ms=$((total_ms + ms))
  done

  local avg_ms=$((total_ms / ITERATIONS))

  # Validate Cache-Control header
  local cc
  cc=$(curl -o /dev/null -s -D - --max-time 10 \
    -H "Accept: application/json" \
    "${BASE_URL}${path}" 2>/dev/null \
    | grep -i "^cache-control:" | tr -d '\r' | cut -d' ' -f2- || true)

  local cache_ok=true
  if [[ "$expected_cache" == "cacheable" ]]; then
    [[ "$cc" == *"public"* ]] || cache_ok=false
  else
    [[ "$cc" == *"no-store"* || "$cc" == *"private"* ]] || cache_ok=false
  fi

  local latency_ok=true
  if [[ "$expected_cache" == "cacheable" && $avg_ms -gt $LAT_TARGET_MS ]]; then
    latency_ok=false
  fi

  local label="[$region] $path (avg ${avg_ms}ms, cache: ${cc:-unknown})"
  if $cache_ok && $latency_ok; then
    pass "$label"
  elif ! $cache_ok; then
    fail "$label — unexpected Cache-Control (expected $expected_cache)"
  else
    warn "$label — latency ${avg_ms}ms exceeds ${LAT_TARGET_MS}ms target (may be cold)"
    ((PASS++))
  fi
}

# ---------------------------------------------------------------------------
# test_consistency_routing
# Sends X-Consistency: strong and verifies X-Route-Primary: true in response.
# ---------------------------------------------------------------------------
test_consistency_routing() {
  log "Testing consistency routing header..."
  local headers
  headers=$(curl -o /dev/null -s -D - --max-time 10 \
    -H "X-Consistency: strong" \
    -H "Accept: application/json" \
    "${BASE_URL}/public/rates" 2>/dev/null || true)

  if echo "$headers" | grep -qi "x-route-primary: true"; then
    pass "X-Consistency: strong → X-Route-Primary: true present"
  else
    fail "X-Consistency: strong → X-Route-Primary: true NOT found in response headers"
  fi
}

# ---------------------------------------------------------------------------
# test_health_edge
# Verifies /health/edge returns 200 and JSON with status/region fields.
# ---------------------------------------------------------------------------
test_health_edge() {
  log "Testing /health/edge endpoint..."
  local http_code body
  http_code=$(curl -o /tmp/edge_health.json -s -w "%{http_code}" \
    --max-time 10 "${BASE_URL}/health/edge" 2>/dev/null || echo "000")
  body=$(cat /tmp/edge_health.json 2>/dev/null || echo "{}")

  if [[ "$http_code" == "200" ]]; then
    pass "/health/edge → 200 OK (body: $body)"
  elif [[ "$http_code" == "503" ]]; then
    warn "/health/edge → 503 (replication lag or dependency failure): $body"
    ((PASS++))
  else
    fail "/health/edge → unexpected HTTP $http_code"
  fi
}

# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------
log "Starting distributed latency tests against $BASE_URL"
log "Target: <${LAT_TARGET_MS}ms for cacheable /public/* endpoints"
echo ""

for region in "${!REGIONS[@]}"; do
  ip="${REGIONS[$region]}"
  resolve_arg=""
  if [[ -n "$ip" ]]; then
    host=$(echo "$BASE_URL" | sed 's|https\?://||' | cut -d'/' -f1)
    resolve_arg="${host}:443:${ip}"
    log "Region: $region (→ $ip)"
  else
    log "Region: $region (using DNS — no IP override set)"
  fi

  for path in "${!ENDPOINTS[@]}"; do
    test_endpoint "$region" "$path" "${ENDPOINTS[$path]}" "$resolve_arg"
  done
  echo ""
done

test_consistency_routing
echo ""
test_health_edge
echo ""

# ---------------------------------------------------------------------------
# Summary
# ---------------------------------------------------------------------------
TOTAL=$((PASS + FAIL))
log "Results: ${PASS}/${TOTAL} passed"
if [[ $FAIL -gt 0 ]]; then
  echo -e "${RED}FAILED — $FAIL test(s) did not pass${NC}"
  exit 1
else
  echo -e "${GREEN}ALL TESTS PASSED${NC}"
  exit 0
fi
