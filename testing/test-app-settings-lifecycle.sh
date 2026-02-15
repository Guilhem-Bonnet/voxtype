#!/usr/bin/env bash
# =============================================================================
# Smoke test: Voxtype App Settings lifecycle
#
# Validates:
# 1. 'ui --settings' flag exists in CLI
# 2. 'voxtype config' produces output
# 3. Settings source files exist
# 4. GUI lifecycle (skipped without display/gtk4-dev)
#
# Requires: voxtype binary
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

command -v "$VOXTYPE" >/dev/null 2>&1 || { echo "voxtype binary not found"; exit 1; }

info "Testing voxtype binary: $($VOXTYPE --version 2>&1 || echo 'unknown')"

# === Test 1: --settings flag accepted ===
info "=== Test 1: 'ui --settings' flag ==="
HELP_OUTPUT=$(timeout 5 $VOXTYPE ui --help 2>&1 || true)
if echo "$HELP_OUTPUT" | grep -q "\-\-settings"; then
    pass "'--settings' flag documented in ui --help"
else
    fail "'--settings' flag not found in ui --help"
fi

# === Test 2: voxtype config produces output ===
info "=== Test 2: 'voxtype config' output ==="
CONFIG_OUTPUT=$(timeout 5 $VOXTYPE config 2>&1 || true)
if [ -n "$CONFIG_OUTPUT" ] && echo "$CONFIG_OUTPUT" | grep -qi "model\|hotkey\|audio\|output"; then
    pass "'voxtype config' produces configuration output"
else
    fail "'voxtype config' did not produce expected output"
fi

# === Test 3: Settings source structure ===
info "=== Test 3: Source structure ==="
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
if [ -f "$PROJECT_DIR/src/gui/settings.rs" ]; then
    pass "Settings module source file exists"
else
    fail "Settings module source file missing"
fi

# Check that settings module has all 5 sections
for section in "build_state_page" "build_audio_page" "build_transcription_page" "build_shortcuts_page" "build_diagnostic_page"; do
    if grep -q "$section" "$PROJECT_DIR/src/gui/settings.rs" 2>/dev/null; then
        pass "Settings section '$section' defined"
    else
        fail "Settings section '$section' missing"
    fi
done

# === Test 4: Update check function exists ===
info "=== Test 4: Update check ==="
if grep -q "check_for_updates" "$PROJECT_DIR/src/gui/settings.rs" 2>/dev/null; then
    pass "Update check function defined"
else
    fail "Update check function missing"
fi

# === Test 5: GUI lifecycle (requires display + GTK4) ===
info "=== Test 5: GUI lifecycle (display-dependent) ==="
if [ -z "${DISPLAY:-}" ] && [ -z "${WAYLAND_DISPLAY:-}" ]; then
    skip "No display server — GUI lifecycle tests skipped"
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
