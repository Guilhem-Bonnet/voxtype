#!/usr/bin/env bash
# =============================================================================
# Smoke test: Voxtype Recording Overlay lifecycle
#
# Validates:
# 1. 'ui' subcommand registered in CLI
# 2. 'voxtype ui' without gui feature shows helpful error
# 3. Level field absent from JSON during idle
# 4. Standard Waybar fields still present
# 5. GUI source structure in place
# 6. GUI lifecycle (skipped without display server)
#
# Requires: jq, voxtype binary
# =============================================================================

set -uo pipefail

PASS=0
FAIL=0
SKIP=0

pass() { echo -e "\033[32m[PASS]\033[0m $1"; ((PASS++)); }
fail() { echo -e "\033[31m[FAIL]\033[0m $1"; ((FAIL++)); }
skip() { echo -e "\033[33m[SKIP]\033[0m $1"; ((SKIP++)); }
info() { echo -e "\033[34m[INFO]\033[0m $1"; }

VOXTYPE="${VOXTYPE:-voxtype}"

# Check prerequisites
command -v jq >/dev/null 2>&1 || { echo "jq is required but not installed"; exit 1; }
command -v "$VOXTYPE" >/dev/null 2>&1 || { echo "voxtype binary not found"; exit 1; }

info "Testing voxtype binary: $($VOXTYPE --version 2>&1 || echo 'unknown')"

# === Test 1: voxtype ui subcommand exists ===
info "=== Test 1: 'ui' subcommand registered ==="
HELP_OUTPUT=$($VOXTYPE --help 2>&1 || true)
if echo "$HELP_OUTPUT" | grep -qw "ui"; then
    pass "Subcommand 'ui' is registered in --help"
else
    fail "Subcommand 'ui' not found in --help"
fi

# === Test 2: voxtype ui without gui feature shows helpful error ===
info "=== Test 2: 'ui' without gui feature ==="
UI_OUTPUT=$(timeout 5 $VOXTYPE ui 2>&1 || true)
if echo "$UI_OUTPUT" | grep -qi "gui\|non disponible\|recompil"; then
    pass "'voxtype ui' shows helpful message about GUI feature"
elif echo "$UI_OUTPUT" | grep -qi "unrecognized"; then
    fail "'voxtype ui' subcommand not compiled into binary"
else
    fail "'voxtype ui' unexpected output: $UI_OUTPUT"
fi

# === Test 3: Level field in JSON during idle ===
info "=== Test 3: Level field in JSON ==="
JSON_IDLE=$(timeout 5 $VOXTYPE status --format json 2>/dev/null || echo '{}')
if echo "$JSON_IDLE" | jq -e '.level' >/dev/null 2>&1; then
    LEVEL_VAL=$(echo "$JSON_IDLE" | jq -r '.level')
    if [ "$LEVEL_VAL" = "null" ] || [ -z "$LEVEL_VAL" ]; then
        pass "level field absent/null during idle"
    else
        fail "level field should be absent during idle, got: $LEVEL_VAL"
    fi
else
    pass "level field absent from JSON during idle"
fi

# === Test 4: JSON still has standard 4 fields ===
info "=== Test 4: Standard fields still present ==="
for field in text alt class tooltip; do
    if echo "$JSON_IDLE" | jq -e ".$field" >/dev/null 2>&1; then
        pass "Field '$field' present in JSON"
    else
        fail "Field '$field' missing from JSON"
    fi
done

# === Test 5: GUI source structure ===
info "=== Test 5: Source structure ==="
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
if [ -f "$PROJECT_DIR/src/gui/mod.rs" ] && [ -f "$PROJECT_DIR/src/gui/overlay.rs" ]; then
    pass "GUI source files present (mod.rs, overlay.rs)"
else
    fail "GUI source files missing"
fi

if grep -q 'gui = \[' "$PROJECT_DIR/Cargo.toml" 2>/dev/null; then
    pass "'gui' feature flag defined in Cargo.toml"
else
    fail "'gui' feature flag missing from Cargo.toml"
fi

# === Test 6: GUI lifecycle (requires display) ===
info "=== Test 6: GUI lifecycle (display-dependent) ==="
if [ -z "${DISPLAY:-}" ] && [ -z "${WAYLAND_DISPLAY:-}" ]; then
    skip "No display server detected — GUI lifecycle tests skipped"
    skip "Run on a desktop session to test overlay open/close"
else
    skip "GUI lifecycle tests not yet automated (requires gtk4-dev)"
fi

# === Summary ===
echo ""
echo "============================================"
TOTAL=$(($PASS + $FAIL + $SKIP))
echo "  Results: $PASS passed, $FAIL failed, $SKIP skipped ($TOTAL total)"
echo "============================================"

if [ $FAIL -gt 0 ]; then
    echo "Some tests FAILED!"
    exit 1
else
    echo "All executable tests passed!"
    exit 0
fi
