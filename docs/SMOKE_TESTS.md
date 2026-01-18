# Smoke Tests

Run these tests after installing a new build to verify core functionality.

## Basic Verification

```bash
# Version and help
voxtype --version
voxtype --help
voxtype daemon --help
voxtype record --help
voxtype setup --help

# Show current config
voxtype config

# Check status
voxtype status
```

## Recording Cycle

```bash
# Basic record start/stop
voxtype record start
sleep 3
voxtype record stop

# Toggle mode
voxtype record toggle  # starts recording
sleep 3
voxtype record toggle  # stops and transcribes

# Cancel recording (should not transcribe)
voxtype record start
sleep 2
voxtype record cancel
# Verify no transcription in logs:
journalctl --user -u voxtype --since "30 seconds ago" | grep -i transcri
```

## CLI Overrides

```bash
# Output mode override (use --clipboard, --type, or --paste)
voxtype record start --clipboard
sleep 2
voxtype record stop
# Verify clipboard has text: wl-paste

# Model override (requires model to be downloaded)
# Note: --model flag is on the main command, not record subcommand
voxtype --model base.en record start
sleep 2
voxtype record stop
```

## GPU Isolation Mode

Tests subprocess-based GPU memory release (for laptops with hybrid graphics):

```bash
# 1. Enable gpu_isolation in config.toml:
#    [whisper]
#    gpu_isolation = true

# 2. Restart daemon
systemctl --user restart voxtype

# 3. Record and transcribe
voxtype record start && sleep 3 && voxtype record stop

# 4. Check logs for subprocess spawning:
journalctl --user -u voxtype --since "1 minute ago" | grep -i subprocess

# 5. Verify GPU memory is released after transcription:
#    (AMD) watch -n1 "cat /sys/class/drm/card*/device/mem_info_vram_used"
#    (NVIDIA) nvidia-smi
```

## On-Demand Model Loading

Tests loading model only when needed (reduces idle memory):

```bash
# 1. Enable on_demand_loading in config.toml:
#    [whisper]
#    on_demand_loading = true

# 2. Restart daemon
systemctl --user restart voxtype

# 3. Check memory before recording (model not loaded):
systemctl --user status voxtype | grep Memory

# 4. Record and transcribe
voxtype record start && sleep 3 && voxtype record stop

# 5. Check logs for model load/unload:
journalctl --user -u voxtype --since "1 minute ago" | grep -E "Loading|Unloading"
```

## Model Switching

```bash
# Download a different model if not present
voxtype setup model  # Interactive selection

# Or specify directly
voxtype setup model small.en

# Test with different models (edit config.toml or use --model flag)
```

## Remote Transcription

```bash
# 1. Configure remote backend in config.toml:
#    [whisper]
#    backend = "remote"
#    remote_endpoint = "http://your-server:8080"

# 2. Restart and test
systemctl --user restart voxtype
voxtype record start && sleep 3 && voxtype record stop

# 3. Check logs for remote transcription:
journalctl --user -u voxtype --since "1 minute ago" | grep -i remote
```

## Output Drivers

```bash
# Test wtype (Wayland native)
# Should work by default on Wayland

# Test ydotool fallback (unset WAYLAND_DISPLAY or rename wtype)
sudo mv /usr/bin/wtype /usr/bin/wtype.bak
voxtype record start && sleep 2 && voxtype record stop
journalctl --user -u voxtype --since "30 seconds ago" | grep ydotool
sudo mv /usr/bin/wtype.bak /usr/bin/wtype

# Test clipboard mode
# Edit config.toml: mode = "clipboard"
systemctl --user restart voxtype
voxtype record start && sleep 2 && voxtype record stop
wl-paste  # Should show transcribed text

# Test paste mode
# Edit config.toml: mode = "paste"
systemctl --user restart voxtype
voxtype record start && sleep 2 && voxtype record stop
```

## Delay Options

```bash
# Test type delays (edit config.toml):
#    type_delay_ms = 50       # Inter-keystroke delay
#    pre_type_delay_ms = 200  # Pre-typing delay

systemctl --user restart voxtype
voxtype record start && sleep 2 && voxtype record stop

# Check debug logs for delay application:
journalctl --user -u voxtype --since "30 seconds ago" | grep -E "delay|sleeping"
```

## Audio Feedback

```bash
# Enable audio feedback in config.toml:
#    [audio.feedback]
#    enabled = true
#    theme = "default"
#    volume = 0.5

systemctl --user restart voxtype
voxtype record start  # Should hear start beep
sleep 2
voxtype record stop   # Should hear stop beep
```

## Compositor Hooks

```bash
# Verify hooks run (check Hyprland submap changes):
voxtype record start
hyprctl submap  # Should show voxtype_recording
sleep 2
voxtype record stop
hyprctl submap  # Should show empty (reset)
```

## Transcribe Command (File Input)

```bash
# Transcribe a WAV file directly (useful for testing without mic)
voxtype transcribe /path/to/audio.wav

# With model override
voxtype transcribe --model large-v3-turbo /path/to/audio.wav
```

## Multilingual Model Verification

Tests that non-.en models load correctly and detect language:

```bash
# Use a multilingual model (without .en suffix)
voxtype --model small record start
sleep 3
voxtype record stop

# Check logs for language auto-detection:
journalctl --user -u voxtype --since "30 seconds ago" | grep "auto-detected language"

# Verify model menu shows multilingual options:
echo "0" | voxtype setup model  # Should show tiny, base, small, medium (multilingual)
```

## Invalid Model Rejection

Verify bad model names warn and fall back to default:

```bash
# Should warn, send notification, and fall back to default model
voxtype --model nonexistent record start
sleep 2
voxtype record cancel

# Expected behavior:
# 1. Warning logged: "Unknown model 'nonexistent', using default model 'base.en'"
# 2. Desktop notification via notify-send
# 3. Recording proceeds with the default model

# Check logs for warning:
journalctl --user -u voxtype --since "30 seconds ago" | grep -i "unknown model"

# The setup --set command should still reject invalid models:
voxtype setup model --set nonexistent
# Expected: error about model not installed
```

## GPU Backend Switching

Test transitions between CPU and GPU backends (engine-aware):

```bash
# Check current status
voxtype setup gpu

# Whisper mode (symlink points to voxtype-vulkan or voxtype-avx*)
# --enable switches to Vulkan, --disable switches to best CPU
ls -la /usr/bin/voxtype  # Verify current symlink
sudo voxtype setup gpu --enable   # Switch to Vulkan
ls -la /usr/bin/voxtype  # Should point to voxtype-vulkan
sudo voxtype setup gpu --disable  # Switch to best CPU (avx512 or avx2)
ls -la /usr/bin/voxtype  # Should point to voxtype-avx512 or voxtype-avx2

# Parakeet mode (symlink points to voxtype-parakeet-*)
# --enable switches to CUDA, --disable switches to best Parakeet CPU
sudo ln -sf /usr/lib/voxtype/voxtype-parakeet-avx512 /usr/bin/voxtype
sudo voxtype setup gpu --enable   # Switch to Parakeet CUDA
ls -la /usr/bin/voxtype  # Should point to voxtype-parakeet-cuda
sudo voxtype setup gpu --disable  # Switch to best Parakeet CPU
ls -la /usr/bin/voxtype  # Should point to voxtype-parakeet-avx512

# Restore to Whisper Vulkan for normal use
sudo ln -sf /usr/lib/voxtype/voxtype-vulkan /usr/bin/voxtype
```

## Parakeet Backend Switching

Test switching between Whisper and Parakeet engines:

```bash
# Check current status
voxtype setup parakeet

# Enable Parakeet (switches symlink to best parakeet binary)
sudo voxtype setup parakeet --enable
ls -la /usr/bin/voxtype  # Should point to voxtype-parakeet-cuda or voxtype-parakeet-avx*

# Disable Parakeet (switches back to equivalent Whisper binary)
sudo voxtype setup parakeet --disable
ls -la /usr/bin/voxtype  # Should point to voxtype-vulkan or voxtype-avx*

# Verify systemd service was updated
grep ExecStart ~/.config/systemd/user/voxtype.service
```

## Engine Switching via Model Selection

Test that selecting a model from a different engine updates config correctly:

```bash
# Start with Whisper engine configured
grep engine ~/.config/voxtype/config.toml  # Should show engine = "whisper" or be absent

# Select a Parakeet model (requires --features parakeet build)
voxtype setup model  # Choose a parakeet-tdt model
grep engine ~/.config/voxtype/config.toml  # Should show engine = "parakeet"
grep -A2 "\[parakeet\]" ~/.config/voxtype/config.toml  # Should show model name

# Select a Whisper model
voxtype setup model  # Choose a Whisper model (e.g., base.en)
grep engine ~/.config/voxtype/config.toml  # Should show engine = "whisper"

# Verify star indicator shows current model
voxtype setup model  # Current model should have * prefix
```

## Waybar JSON Output

Test the status follower with JSON format for Waybar integration:

```bash
# Should output JSON status updates (Ctrl+C to stop)
timeout 3 voxtype status --follow --format json || true

# Expected output format:
# {"text":"idle","class":"idle","tooltip":"Voxtype: idle"}

# Test during recording:
voxtype record start &
sleep 1
timeout 2 voxtype status --follow --format json || true
voxtype record cancel
```

## Single Instance Enforcement

Verify only one daemon can run at a time:

```bash
# With daemon already running via systemd, try starting another:
voxtype daemon
# Should fail with error about existing instance / PID lock

# Check PID file:
cat ~/.local/share/voxtype/voxtype.pid
ps aux | grep voxtype
```

## Post-Processing Command

Tests LLM cleanup if configured:

```bash
# 1. Configure post-processing in config.toml:
#    [output]
#    post_process_command = "your-llm-cleanup-script"

# 2. Restart daemon
systemctl --user restart voxtype

# 3. Record and transcribe
voxtype record start && sleep 3 && voxtype record stop

# 4. Check logs for post-processing:
journalctl --user -u voxtype --since "1 minute ago" | grep -i "post.process"
```

## Config Validation

Verify malformed config files produce clear errors:

```bash
# Backup current config
cp ~/.config/voxtype/config.toml ~/.config/voxtype/config.toml.bak

# Test with invalid TOML syntax
echo "invalid toml [[[" >> ~/.config/voxtype/config.toml
voxtype config  # Should show parse error with line number

# Test with unknown field (should warn but continue)
echo 'unknown_field = "value"' >> ~/.config/voxtype/config.toml
voxtype config

# Restore config
mv ~/.config/voxtype/config.toml.bak ~/.config/voxtype/config.toml
```

## Signal Handling

Test direct signal control of the daemon:

```bash
# Get daemon PID
DAEMON_PID=$(cat ~/.local/share/voxtype/voxtype.pid)

# Start recording via SIGUSR1
kill -USR1 $DAEMON_PID
voxtype status  # Should show "recording"
sleep 2

# Stop recording via SIGUSR2
kill -USR2 $DAEMON_PID
voxtype status  # Should show "transcribing" then "idle"

# Check logs:
journalctl --user -u voxtype --since "30 seconds ago" | grep -E "USR1|USR2|signal"
```

## Rapid Successive Recordings

Stress test with quick start/stop cycles:

```bash
# Run multiple quick recordings in succession
for i in {1..5}; do
    echo "Recording $i..."
    voxtype record start
    sleep 1
    voxtype record cancel
done

# Verify daemon is still healthy
voxtype status
journalctl --user -u voxtype --since "1 minute ago" | grep -iE "error|panic"
```

## Long Recording

Test recording near the max_duration_secs limit:

```bash
# Check current max duration
voxtype config | grep max_duration

# Start a long recording (default max is 60s)
# The daemon should auto-stop at the limit
voxtype record start
echo "Recording... will auto-stop at max_duration_secs"
# Wait or manually stop before limit:
sleep 10
voxtype record stop

# To test auto-cutoff, set max_duration_secs = 5 in config and record longer
```

## Service Restart Cycle

Test systemd service restarts:

```bash
# Multiple restart cycles
for i in {1..3}; do
    echo "Restart cycle $i..."
    systemctl --user restart voxtype
    sleep 2
    voxtype status
done

# Verify clean restarts in logs:
journalctl --user -u voxtype --since "1 minute ago" | grep -E "Starting|Ready|shutdown"
```

## Quick Smoke Test Script

```bash
#!/bin/bash
# quick-smoke-test.sh - Run after new build install

set -e
echo "=== Voxtype Smoke Tests ==="

echo -n "Version: "
voxtype --version

echo -n "Status: "
voxtype status

echo "Recording 3 seconds..."
voxtype record start
sleep 3
voxtype record stop
echo "Done."

echo ""
echo "Check logs:"
journalctl --user -u voxtype --since "30 seconds ago" --no-pager | tail -10
```
