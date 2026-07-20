#!/usr/bin/env bash
# ---------------------------------------------------------------------------
# E2E Test Suite for Neuro Gateway
#
# Starts the gateway in dev mode (no GATEWAY_API_KEY), runs functional
# tests against its HTTP endpoints, and reports results.
#
# Requirements:
#   - Gateway binary built: cargo build --release
#   - Port 7777 free (or set PORT env var)
#   - Redis running on localhost:6379 (or set GATEWAY_REDIS_URL)
#
# Usage:
#   chmod +x tests/e2e.sh
#   ./tests/e2e.sh
# ---------------------------------------------------------------------------
set -euo pipefail

PORT="${PORT:-7777}"
GATEWAY_BIN="${GATEWAY_BIN:-./target/release/neuro-gateway}"
BASE="http://localhost:${PORT}"
PASS=0
FAIL=0
GW_PID=""

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

start_gateway() {
    echo "Starting gateway on port ${PORT} (dev mode, no API key)..."
    PORT="${PORT}" \
    GATEWAY_REDIS_URL="${GATEWAY_REDIS_URL:-redis://localhost:6379}" \
    RUST_LOG=warn \
        "${GATEWAY_BIN}" &
    GW_PID=$!
    # Wait for the gateway to become ready.
    local retries=0
    while ! curl -sf "${BASE}/healthz" > /dev/null 2>&1; do
        retries=$((retries + 1))
        if [ "${retries}" -ge 30 ]; then
            echo "ERROR: gateway did not start within 15 seconds"
            exit 1
        fi
        sleep 0.5
    done
    echo "Gateway started (PID ${GW_PID})"
}

stop_gateway() {
    if [ -n "${GW_PID}" ]; then
        kill "${GW_PID}" 2>/dev/null || true
        wait "${GW_PID}" 2>/dev/null || true
    fi
}

assert_status() {
    local name="$1" expected="$2" actual="$3"
    if [ "$actual" -eq "$expected" ]; then
        echo "  PASS  $name (HTTP $actual)"
        PASS=$((PASS + 1))
    else
        echo "  FAIL  $name (expected $expected, got $actual)"
        FAIL=$((FAIL + 1))
    fi
}

assert_json_field() {
    local name="$1" body="$2" field="$3"
    if echo "$body" | grep -q "\"${field}\""; then
        echo "  PASS  $name (has $field)"
        PASS=$((PASS + 1))
    else
        echo "  FAIL  $name (missing $field)"
        FAIL=$((FAIL + 1))
    fi
}

assert_contains() {
    local name="$1" body="$2" needle="$3"
    if echo "$body" | grep -qi "$needle"; then
        echo "  PASS  $name"
        PASS=$((PASS + 1))
    else
        echo "  FAIL  $name (missing: $needle)"
        FAIL=$((FAIL + 1))
    fi
}

trap stop_gateway EXIT

# ---------------------------------------------------------------------------
# Start
# ---------------------------------------------------------------------------

start_gateway

# ---------------------------------------------------------------------------
# Health Checks
# ---------------------------------------------------------------------------

echo ""
echo "=== Health Checks ==="

STATUS=$(curl -sf -o /dev/null -w "%{http_code}" "${BASE}/healthz")
assert_status "/healthz returns 200" 200 "$STATUS"

BODY=$(curl -sf "${BASE}/healthz")
assert_json_field "/healthz has status field" "$BODY" "status"

# ---------------------------------------------------------------------------
# Model Discovery
# ---------------------------------------------------------------------------

echo ""
echo "=== Model Discovery ==="

STATUS=$(curl -sf -o /dev/null -w "%{http_code}" "${BASE}/v1/models")
assert_status "/v1/models returns 200" 200 "$STATUS"

BODY=$(curl -sf "${BASE}/v1/models")
assert_json_field "/v1/models has data array" "$BODY" "data"

# ---------------------------------------------------------------------------
# X-Request-Id Header
# ---------------------------------------------------------------------------

echo ""
echo "=== X-Request-Id ==="

HEADERS=$(curl -sf -I "${BASE}/healthz")
assert_contains "X-Request-Id present on /healthz" "$HEADERS" "x-request-id"

HEADERS=$(curl -sf -I "${BASE}/v1/models")
assert_contains "X-Request-Id present on /v1/models" "$HEADERS" "x-request-id"

# ---------------------------------------------------------------------------
# Error Format — /v1/chat/completions
# ---------------------------------------------------------------------------

echo ""
echo "=== Error Format (/v1/chat/completions) ==="

STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X POST "${BASE}/v1/chat/completions" \
    -H "Content-Type: application/json" -d '{}')
# Missing model/messages should return 400, not 500
assert_status "Empty body returns 400" 400 "$STATUS"

BODY=$(curl -s -X POST "${BASE}/v1/chat/completions" \
    -H "Content-Type: application/json" -d '{}')
assert_json_field "Error response is JSON with error field" "$BODY" "error"

# ---------------------------------------------------------------------------
# Error Format — /v1/messages
# ---------------------------------------------------------------------------

echo ""
echo "=== Error Format (/v1/messages) ==="

STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X POST "${BASE}/v1/messages" \
    -H "Content-Type: application/json" -d '{}')
assert_status "/v1/messages empty body returns 400" 400 "$STATUS"

BODY=$(curl -s -X POST "${BASE}/v1/messages" \
    -H "Content-Type: application/json" -d '{}')
assert_json_field "/v1/messages error is JSON" "$BODY" "error"

# ---------------------------------------------------------------------------
# Error Format — /v1/responses
# ---------------------------------------------------------------------------

echo ""
echo "=== Error Format (/v1/responses) ==="

STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X POST "${BASE}/v1/responses" \
    -H "Content-Type: application/json" -d '{}')
assert_status "/v1/responses empty body returns 400" 400 "$STATUS"

# ---------------------------------------------------------------------------
# Content-Type Validation
# ---------------------------------------------------------------------------

echo ""
echo "=== Content-Type Validation ==="

# POST without Content-Type should return 4xx (415 or 400), not crash
STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X POST "${BASE}/v1/chat/completions" \
    -d 'raw text')
if [ "$STATUS" -ge 400 ] && [ "$STATUS" -lt 500 ]; then
    echo "  PASS  Missing Content-Type returns 4xx (HTTP $STATUS)"
    PASS=$((PASS + 1))
else
    echo "  FAIL  Missing Content-Type should return 4xx, got HTTP $STATUS"
    FAIL=$((FAIL + 1))
fi

# ---------------------------------------------------------------------------
# Method Validation
# ---------------------------------------------------------------------------

echo ""
echo "=== Method Validation ==="

STATUS=$(curl -s -o /dev/null -w "%{http_code}" -X GET "${BASE}/v1/chat/completions")
if [ "$STATUS" -eq 405 ] || [ "$STATUS" -ge 400 ]; then
    echo "  PASS  GET /v1/chat/completions rejected (HTTP $STATUS)"
    PASS=$((PASS + 1))
else
    echo "  FAIL  GET /v1/chat/completions should be rejected, got HTTP $STATUS"
    FAIL=$((FAIL + 1))
fi

# ---------------------------------------------------------------------------
# 404 for unknown routes
# ---------------------------------------------------------------------------

echo ""
echo "=== Unknown Routes ==="

STATUS=$(curl -s -o /dev/null -w "%{http_code}" "${BASE}/v1/nonexistent")
assert_status "Unknown route returns 404" 404 "$STATUS"

# ---------------------------------------------------------------------------
# CORS headers
# ---------------------------------------------------------------------------

echo ""
echo "=== CORS ==="

HEADERS=$(curl -sf -I -X OPTIONS "${BASE}/healthz" \
    -H "Origin: http://localhost:3000" \
    -H "Access-Control-Request-Method: POST" 2>/dev/null || true)
if echo "$HEADERS" | grep -qi "access-control"; then
    echo "  PASS  CORS headers present on OPTIONS preflight"
    PASS=$((PASS + 1))
else
    echo "  SKIP  CORS headers not detected (may not be enabled)"
fi

# ---------------------------------------------------------------------------
# Results
# ---------------------------------------------------------------------------

echo ""
echo "==========================================="
echo "  Results:  ${PASS} passed, ${FAIL} failed"
echo "==========================================="

if [ "${FAIL}" -gt 0 ]; then
    exit 1
fi
