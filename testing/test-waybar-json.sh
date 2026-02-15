#!/bin/bash
# Smoke test: Waybar JSON contract validation
# Validates that `voxtype status --format json` produces valid JSON
# with the expected stable fields (NFR2 rétrocompatibilité).
#
# Usage: ./test-waybar-json.sh [path-to-voxtype-binary]
#
# Can run without a live daemon — tests the binary's output directly.

set -e

VOXTYPE="${1:-voxtype}"
PASS=0
FAIL=0
TOTAL=0

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

log_pass() {
    echo -e "${GREEN}[PASS]${NC} $1"
    ((PASS++)) || true
    ((TOTAL++)) || true
}

log_fail() {
    echo -e "${RED}[FAIL]${NC} $1"
    ((FAIL++)) || true
    ((TOTAL++)) || true
}

log_info() {
    echo -e "${YELLOW}[INFO]${NC} $1"
}

# Check if jq is available
if ! command -v jq &> /dev/null; then
    echo "ERROR: jq is required for this test. Install with: sudo apt install jq"
    exit 1
fi

# Check if voxtype binary exists
if ! command -v "$VOXTYPE" &> /dev/null; then
    echo "ERROR: voxtype binary not found at '$VOXTYPE'"
    exit 1
fi

log_info "Testing voxtype binary: $($VOXTYPE --version 2>&1 || echo 'unknown')"
echo ""

# ============================================================
# Test 1: Basic JSON validity
# ============================================================
log_info "=== Test: Basic JSON output ==="

JSON_OUTPUT=$($VOXTYPE status --format json 2>/dev/null || echo '{}')

if echo "$JSON_OUTPUT" | jq . > /dev/null 2>&1; then
    log_pass "Output is valid JSON"
else
    log_fail "Output is NOT valid JSON: $JSON_OUTPUT"
fi

# ============================================================
# Test 2: Required fields present (text, alt, class, tooltip)
# ============================================================
log_info "=== Test: Required fields ==="

for field in text alt class tooltip; do
    if echo "$JSON_OUTPUT" | jq -e ".$field" > /dev/null 2>&1; then
        VALUE=$(echo "$JSON_OUTPUT" | jq -r ".$field")
        log_pass "Field '$field' present (value: \"$VALUE\")"
    else
        log_fail "Field '$field' MISSING from JSON output"
    fi
done

# ============================================================
# Test 3: Field types are strings
# ============================================================
log_info "=== Test: Field types ==="

for field in text alt class tooltip; do
    TYPE=$(echo "$JSON_OUTPUT" | jq -r ".$field | type" 2>/dev/null)
    if [ "$TYPE" = "string" ]; then
        log_pass "Field '$field' is type string"
    else
        log_fail "Field '$field' is type '$TYPE', expected 'string'"
    fi
done

# ============================================================
# Test 4: class value is one of the valid states
# ============================================================
log_info "=== Test: class value validation ==="

CLASS=$(echo "$JSON_OUTPUT" | jq -r ".class" 2>/dev/null)
case "$CLASS" in
    idle|recording|transcribing|stopped)
        log_pass "class='$CLASS' is a valid state"
        ;;
    *)
        log_fail "class='$CLASS' is NOT a valid state (expected: idle, recording, transcribing, stopped)"
        ;;
esac

# ============================================================
# Test 5: alt value matches class
# ============================================================
log_info "=== Test: alt/class consistency ==="

ALT=$(echo "$JSON_OUTPUT" | jq -r ".alt" 2>/dev/null)
if [ "$ALT" = "$CLASS" ]; then
    log_pass "alt ('$ALT') matches class ('$CLASS')"
else
    log_fail "alt ('$ALT') does NOT match class ('$CLASS')"
fi

# ============================================================
# Test 6: text field is non-empty (icon should be present)
# ============================================================
log_info "=== Test: text field non-empty ==="

TEXT=$(echo "$JSON_OUTPUT" | jq -r ".text" 2>/dev/null)
if [ -n "$TEXT" ] && [ "$TEXT" != "null" ]; then
    log_pass "text field is non-empty: '$TEXT'"
else
    # Empty text is valid for 'stopped' state
    if [ "$CLASS" = "stopped" ]; then
        log_pass "text field is empty (valid for stopped state)"
    else
        log_fail "text field is empty for state '$CLASS'"
    fi
fi

# ============================================================
# Test 7: Extended fields with --extended flag
# ============================================================
log_info "=== Test: Extended JSON output ==="

EXT_OUTPUT=$($VOXTYPE status --format json --extended 2>/dev/null || echo '{}')

if echo "$EXT_OUTPUT" | jq . > /dev/null 2>&1; then
    log_pass "Extended output is valid JSON"
else
    log_fail "Extended output is NOT valid JSON: $EXT_OUTPUT"
fi

for field in model device backend; do
    if echo "$EXT_OUTPUT" | jq -e ".$field" > /dev/null 2>&1; then
        VALUE=$(echo "$EXT_OUTPUT" | jq -r ".$field")
        TYPE=$(echo "$EXT_OUTPUT" | jq -r ".$field | type")
        if [ "$TYPE" = "string" ]; then
            log_pass "Extended field '$field' present and is string (value: \"$VALUE\")"
        else
            log_fail "Extended field '$field' is type '$TYPE', expected 'string'"
        fi
    else
        log_fail "Extended field '$field' MISSING from extended output"
    fi
done

# ============================================================
# Test 8: Extended output still has standard fields
# ============================================================
log_info "=== Test: Extended output includes standard fields ==="

for field in text alt class tooltip; do
    if echo "$EXT_OUTPUT" | jq -e ".$field" > /dev/null 2>&1; then
        log_pass "Standard field '$field' still present in extended output"
    else
        log_fail "Standard field '$field' MISSING from extended output"
    fi
done

# ============================================================
# Test 9: Standard output does NOT have extended fields
# ============================================================
log_info "=== Test: Standard output excludes extended fields ==="

for field in model device backend; do
    if echo "$JSON_OUTPUT" | jq -e ".$field" > /dev/null 2>&1; then
        log_fail "Extended field '$field' should NOT be in standard (non-extended) output"
    else
        log_pass "Extended field '$field' correctly absent from standard output"
    fi
done

# ============================================================
# Summary
# ============================================================
echo ""
echo "============================================"
echo -e "  Results: ${GREEN}$PASS passed${NC}, ${RED}$FAIL failed${NC} (${TOTAL} total)"
echo "============================================"

if [ "$FAIL" -gt 0 ]; then
    exit 1
else
    echo -e "${GREEN}All Waybar JSON contract tests passed!${NC}"
    exit 0
fi
