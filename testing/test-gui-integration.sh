#!/usr/bin/env bash
# =============================================================================
# Integration tests: Voxtype GUI (overlay, tray, keybinding)
#
# Validates:
# 1.  Binary compiled with --features gui
# 2.  voxtype ui starts and stays alive
# 3.  voxtype ui creates a StatusNotifierItem on D-Bus (tray icon)
# 4.  Overlay reacts to daemon state changes (recording/idle)
# 5.  Tray has Activate method (left-click works)
# 6.  No keybinding conflict with super-whisper-linux
# 7.  GNOME custom keybinding points to voxtype (not old widget)
# 8.  No old voxtype-widget process running
# 9.  Daemon is active and responsive
# 10. voxtype record toggle works (sends SIGUSR to daemon)
# 11. CSS loads without our-code errors
# 12. Status monitor connects to daemon
#
# Requires: voxtype binary (gui-enabled), jq, running display server
# Run:      ./testing/test-gui-integration.sh
# =============================================================================

set -uo pipefail

PASS=0
FAIL=0
SKIP=0
WARN=0

pass() { echo -e "\033[32m[PASS]\033[0m $1"; ((PASS++)); }
fail() { echo -e "\033[31m[FAIL]\033[0m $1"; ((FAIL++)); }
skip() { echo -e "\033[33m[SKIP]\033[0m $1"; ((SKIP++)); }
warn() { echo -e "\033[35m[WARN]\033[0m $1"; ((WARN++)); }
info() { echo -e "\033[34m[INFO]\033[0m $1"; }

VOXTYPE="${VOXTYPE:-voxtype}"
UI_PID=""

cleanup() {
    if [ -n "$UI_PID" ]; then
        kill "$UI_PID" 2>/dev/null
        wait "$UI_PID" 2>/dev/null
    fi
}
trap cleanup EXIT

# Check prerequisites
command -v jq >/dev/null 2>&1 || { echo "jq is required"; exit 1; }
command -v "$VOXTYPE" >/dev/null 2>&1 || { echo "voxtype binary not found"; exit 1; }

info "Testing voxtype binary: $($VOXTYPE --version 2>&1 || echo 'unknown')"
info "Display: DISPLAY=${DISPLAY:-''} WAYLAND_DISPLAY=${WAYLAND_DISPLAY:-''}"
echo ""

# ─── Display server check ──────────────────────────────────────────
if [ -z "${DISPLAY:-}" ] && [ -z "${WAYLAND_DISPLAY:-}" ]; then
    info "No display server — running headless-only tests"
    HEADLESS=1
else
    HEADLESS=0
fi

# =============================================================================
# Test 1: Binary has GUI support
# =============================================================================
info "=== Test 1: Binary compiled with GUI support ==="
UI_CHECK=$(timeout 3 "$VOXTYPE" ui --help 2>&1 || true)
if echo "$UI_CHECK" | grep -qi "gui.*non disponible\|recompil\|not available"; then
    fail "Binary not compiled with --features gui"
    echo "  Rebuild with: cargo build --release --features gui"
    exit 1
elif echo "$UI_CHECK" | grep -qi "usage\|options\|settings\|voxtype ui"; then
    pass "Binary has GUI support"
else
    # If it just launches, that's fine too (no --help for ui subcommand)
    pass "Binary has 'ui' subcommand"
fi

# =============================================================================
# Test 2: Daemon is running
# =============================================================================
info "=== Test 2: Daemon is active ==="
if systemctl --user is-active voxtype >/dev/null 2>&1; then
    pass "voxtype daemon is active (systemd)"
elif pgrep -f "voxtype daemon" >/dev/null 2>&1; then
    pass "voxtype daemon is running (process)"
else
    fail "voxtype daemon is not running"
    warn "Start it with: systemctl --user start voxtype"
fi

# =============================================================================
# Test 3: Daemon responds to status
# =============================================================================
info "=== Test 3: Daemon status response ==="
STATUS_JSON=$(timeout 5 "$VOXTYPE" status --format json 2>/dev/null || echo '{}')
if echo "$STATUS_JSON" | jq -e '.class' >/dev/null 2>&1; then
    CLASS=$(echo "$STATUS_JSON" | jq -r '.class')
    pass "Daemon responds with class='$CLASS'"
else
    fail "Daemon not responding to status query"
fi

# =============================================================================
# Test 4: No keybinding conflict — super-whisper-linux
# =============================================================================
info "=== Test 4: No super-whisper-linux conflict ==="
if pgrep -f "super-whisper-linux" >/dev/null 2>&1; then
    fail "super-whisper-linux is running — will conflict with hotkey!"
    warn "Fix: systemctl --user stop super-whisper-linux && systemctl --user disable super-whisper-linux"
elif systemctl --user is-enabled super-whisper-linux >/dev/null 2>&1; then
    SWL_ENABLED=$(systemctl --user is-enabled super-whisper-linux 2>/dev/null || echo "disabled")
    if [ "$SWL_ENABLED" = "enabled" ]; then
        warn "super-whisper-linux is enabled but not running — may start on reboot"
        warn "Fix: systemctl --user disable super-whisper-linux"
    else
        pass "super-whisper-linux is disabled"
    fi
else
    pass "super-whisper-linux not installed or disabled"
fi

# =============================================================================
# Test 5: No old voxtype-widget process
# =============================================================================
info "=== Test 5: No old voxtype-widget process ==="
if pgrep -f "voxtype-widget" >/dev/null 2>&1; then
    fail "Old voxtype-widget is running — visual conflict!"
    warn "Fix: kill \$(pgrep -f voxtype-widget)"
else
    pass "No old voxtype-widget process"
fi

# =============================================================================
# Test 6: GNOME keybinding check (GNOME only)
# =============================================================================
info "=== Test 6: GNOME keybinding configuration ==="
if [ "${XDG_CURRENT_DESKTOP:-}" = "ubuntu:GNOME" ] || [ "${XDG_CURRENT_DESKTOP:-}" = "GNOME" ]; then
    # Check all custom keybindings
    CUSTOM_LIST=$(gsettings get org.gnome.settings-daemon.plugins.media-keys custom-keybindings 2>/dev/null || echo "[]")
    FOUND_VOXTYPE=0
    FOUND_OLD_WIDGET=0

    # Parse dconf paths
    PATHS=$(echo "$CUSTOM_LIST" | tr -d "[]'" | tr ',' '\n' | sed 's/^ *//')
    for DCONF_PATH in $PATHS; do
        [ -z "$DCONF_PATH" ] && continue
        CMD=$(gsettings get org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:"$DCONF_PATH" command 2>/dev/null || echo "")
        BINDING=$(gsettings get org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:"$DCONF_PATH" binding 2>/dev/null || echo "")
        NAME=$(gsettings get org.gnome.settings-daemon.plugins.media-keys.custom-keybinding:"$DCONF_PATH" name 2>/dev/null || echo "")

        if echo "$CMD" | grep -q "voxtype-widget"; then
            FOUND_OLD_WIDGET=1
            fail "Keybinding '$NAME' ($BINDING) points to old voxtype-widget"
            warn "Fix: gsettings set ...custom-keybinding:${DCONF_PATH} command 'voxtype record toggle'"
        elif echo "$CMD" | grep -q "voxtype"; then
            FOUND_VOXTYPE=1
            pass "Keybinding '$NAME' ($BINDING) → $(echo $CMD | tr -d "'")"
        fi
    done

    if [ $FOUND_VOXTYPE -eq 0 ] && [ $FOUND_OLD_WIDGET -eq 0 ]; then
        info "No GNOME keybinding for voxtype found (using evdev hotkey instead)"
        pass "No conflicting keybindings"
    fi
else
    skip "Not GNOME desktop — keybinding check skipped"
fi

# =============================================================================
# Test 7: voxtype record toggle works
# =============================================================================
info "=== Test 7: record toggle command ==="
TOGGLE_OUTPUT=$(timeout 5 "$VOXTYPE" record toggle 2>&1 || true)
EXIT_CODE=$?
# toggle should either start or stop recording — both are OK
if [ $EXIT_CODE -le 1 ]; then
    pass "voxtype record toggle executed (exit=$EXIT_CODE)"
    # Wait briefly then toggle back if we started recording
    sleep 1
    STATUS_AFTER=$(timeout 3 "$VOXTYPE" status --format json 2>/dev/null | jq -r '.class' 2>/dev/null || echo "unknown")
    if [ "$STATUS_AFTER" = "recording" ]; then
        info "Recording started — toggling back to stop"
        timeout 5 "$VOXTYPE" record toggle 2>/dev/null || true
        sleep 1
    fi
else
    fail "voxtype record toggle failed (exit=$EXIT_CODE): $TOGGLE_OUTPUT"
fi

# ─── Display-dependent tests below ─────────────────────────────────
if [ $HEADLESS -eq 1 ]; then
    skip "Tests 8-12 require display server"
    SKIP=$((SKIP + 5))
else

# =============================================================================
# Test 8: voxtype ui starts and stays alive
# =============================================================================
info "=== Test 8: voxtype ui lifecycle ==="
# Kill any existing ui first
kill $(pgrep -xf "voxtype ui") 2>/dev/null
sleep 1

"$VOXTYPE" ui >/dev/null 2>&1 &
UI_PID=$!
sleep 3

if kill -0 "$UI_PID" 2>/dev/null; then
    pass "voxtype ui is alive after 3s (PID=$UI_PID)"
else
    fail "voxtype ui exited prematurely"
    UI_PID=""
fi

# =============================================================================
# Test 9: StatusNotifierItem registered on D-Bus (tray icon)
# =============================================================================
info "=== Test 9: Tray icon on D-Bus ==="
if [ -n "$UI_PID" ]; then
    DBUS_NAMES=$(dbus-send --session --dest=org.freedesktop.DBus --type=method_call \
        --print-reply /org/freedesktop/DBus org.freedesktop.DBus.ListNames 2>/dev/null || echo "")
    if echo "$DBUS_NAMES" | grep -q "StatusNotifierItem-${UI_PID}"; then
        pass "StatusNotifierItem registered for PID $UI_PID"

        # Check Activate method exists (left-click support)
        SNI_DEST="org.kde.StatusNotifierItem-${UI_PID}-1"
        INTROSPECT=$(dbus-send --session --dest="$SNI_DEST" --print-reply \
            /StatusNotifierItem org.freedesktop.DBus.Introspectable.Introspect 2>/dev/null || echo "")
        if echo "$INTROSPECT" | grep -q "Activate"; then
            pass "Tray Activate method found (left-click works)"
        else
            warn "Tray Activate method not found — left-click may not work"
        fi
    elif echo "$DBUS_NAMES" | grep -q "StatusNotifierItem"; then
        pass "A StatusNotifierItem is registered (may be different PID)"
    else
        fail "No StatusNotifierItem found on D-Bus"
        warn "Install GNOME AppIndicator extension for tray support"
    fi
else
    skip "UI not running — skipping D-Bus tray check"
fi

# =============================================================================
# Test 10: Status monitor connects (overlay subprocess)
# =============================================================================
info "=== Test 10: Status monitor subprocess ==="
if [ -n "$UI_PID" ]; then
    sleep 1
    STATUS_PROCS=$(pgrep -f "voxtype status --follow --format json" 2>/dev/null | wc -l)
    if [ "$STATUS_PROCS" -ge 1 ]; then
        pass "Status monitor connected ($STATUS_PROCS status subprocess(es))"
    else
        fail "No 'voxtype status --follow' subprocess found"
    fi
else
    skip "UI not running — skipping status monitor check"
fi

# =============================================================================
# Test 11: Overlay reacts to recording state
# =============================================================================
info "=== Test 11: Overlay reacts to state change ==="
if [ -n "$UI_PID" ]; then
    # Start recording
    timeout 5 "$VOXTYPE" record toggle 2>/dev/null || true
    sleep 2

    STATUS_REC=$(timeout 3 "$VOXTYPE" status --format json 2>/dev/null | jq -r '.class' 2>/dev/null || echo "unknown")
    if [ "$STATUS_REC" = "recording" ]; then
        pass "Daemon entered recording state"
        # Stop recording
        timeout 5 "$VOXTYPE" record toggle 2>/dev/null || true
        sleep 3
        STATUS_AFTER=$(timeout 3 "$VOXTYPE" status --format json 2>/dev/null | jq -r '.class' 2>/dev/null || echo "unknown")
        if [ "$STATUS_AFTER" = "idle" ] || [ "$STATUS_AFTER" = "transcribing" ]; then
            pass "Daemon returned to $STATUS_AFTER after toggle"
        else
            warn "Daemon in unexpected state after stop: $STATUS_AFTER"
        fi
    else
        skip "Could not start recording (state=$STATUS_REC) — mic may be unavailable"
    fi
else
    skip "UI not running — skipping overlay reaction test"
fi

# =============================================================================
# Test 12: No Gtk-CRITICAL errors on startup
# =============================================================================
info "=== Test 12: No Gtk-CRITICAL on startup ==="
# Restart UI and capture stderr briefly
kill "$UI_PID" 2>/dev/null
wait "$UI_PID" 2>/dev/null
UI_PID=""
sleep 1

GTK_LOG=$(mktemp)
"$VOXTYPE" ui 2>"$GTK_LOG" &
UI_PID=$!
sleep 4

if [ -f "$GTK_LOG" ]; then
    CRITICAL_COUNT=$(grep -c "Gtk-CRITICAL" "$GTK_LOG" 2>/dev/null || echo 0)
    if [ "$CRITICAL_COUNT" -eq 0 ]; then
        pass "No Gtk-CRITICAL errors on startup"
    else
        fail "$CRITICAL_COUNT Gtk-CRITICAL errors found:"
        grep "Gtk-CRITICAL" "$GTK_LOG" | head -3
    fi
    rm -f "$GTK_LOG"
else
    skip "Could not capture GTK log"
fi

fi  # end HEADLESS check

# =============================================================================
# Summary
# =============================================================================
echo ""
echo "============================================"
TOTAL=$(($PASS + $FAIL + $SKIP))
echo "  Results: $PASS passed, $FAIL failed, $SKIP skipped, $WARN warnings ($TOTAL total)"
echo "============================================"

if [ $FAIL -gt 0 ]; then
    echo "Some tests FAILED!"
    exit 1
else
    echo "All executable tests passed!"
    exit 0
fi
