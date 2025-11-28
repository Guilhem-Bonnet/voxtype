# Claude Code Tasks for Voxtype

This document provides a task breakdown for building voxtype in a multi-agent coding environment like Claude Code. Tasks are ordered by dependency and can be parallelized where noted.

## Phase 1: Foundation (Sequential)

### Task 1.1: Verify Dependencies and Cargo.toml
```
Review Cargo.toml and verify all crate versions exist on crates.io.
Check whisper-rs, evdev, cpal, and other dependencies for compatibility.
Run `cargo check` to identify any immediate issues.
```

### Task 1.2: Build Core Error Types
```
File: src/error.rs
- Implement all error types with thiserror
- Ensure error messages guide users to solutions
- Add From implementations for external error types
Run: cargo check
```

### Task 1.3: Build Configuration Module
```
File: src/config.rs  
- Implement Config struct and loading logic
- Add TOML parsing with defaults
- Add environment variable overrides
- Add unit tests for config parsing
Run: cargo test config
```

### Task 1.4: Build State Machine
```
File: src/state.rs
- Implement State enum and Event enum
- Implement transition logic
- Add comprehensive unit tests
Run: cargo test state
```

## Phase 2: Input/Output Modules (Parallelizable)

### Task 2.1: Hotkey Module (evdev)
```
Files: src/hotkey/mod.rs, src/hotkey/evdev_listener.rs
- Implement HotkeyListener trait
- Implement EvdevListener with key parsing
- Test with: sudo evtest (to find key codes)
- Handle permission errors gracefully
Run: cargo check
Manual test: Run with elevated permissions to verify key detection
```

### Task 2.2: Audio Module (cpal)
```
Files: src/audio/mod.rs, src/audio/cpal_capture.rs
- Implement AudioCapture trait
- Implement CpalCapture with resampling
- Handle mono/stereo conversion
- Test audio device enumeration
Run: cargo check
Manual test: Record a short sample and save to WAV
```

### Task 2.3: Output Module (ydotool + clipboard)
```
Files: src/output/mod.rs, src/output/ydotool.rs, src/output/clipboard.rs
- Implement TextOutput trait
- Implement YdotoolOutput
- Implement ClipboardOutput  
- Implement fallback chain logic
Run: cargo check
Manual test: Test with echo "test" | wl-copy and ydotool type "test"
```

## Phase 3: Transcription (Requires Model)

### Task 3.1: Whisper Module
```
Files: src/transcribe/mod.rs, src/transcribe/whisper.rs
- Implement Transcriber trait
- Implement WhisperTranscriber
- Handle model path resolution
- Add model download URL helper
Note: This requires downloading a whisper model to test
Run: cargo check
Manual test: Transcribe a test WAV file
```

## Phase 4: Integration

### Task 4.1: Daemon Module
```
File: src/daemon.rs
- Wire all components together
- Implement main event loop with tokio::select!
- Handle state transitions
- Add graceful shutdown
Run: cargo check
```

### Task 4.2: Main Entry Point
```
File: src/main.rs
- Implement CLI with clap
- Implement daemon command
- Implement transcribe command
- Implement setup command
- Implement config command
Run: cargo build
```

## Phase 5: Testing and Polish

### Task 5.1: Integration Testing
```
- Test full workflow: hotkey -> record -> transcribe -> output
- Test fallback chain (disable ydotool, verify clipboard works)
- Test error conditions (missing model, no audio device)
- Test configuration overrides
```

### Task 5.2: Documentation
```
- Update README with any discovered requirements
- Document troubleshooting steps
- Add examples to config file
```

## Build Commands

```bash
# Development build
cargo build

# Release build (optimized, includes whisper.cpp)
cargo build --release

# Run tests
cargo test

# Check without building
cargo check

# Format code
cargo fmt

# Lint
cargo clippy
```

## Test Environment Setup

```bash
# Required for hotkey detection
sudo usermod -aG input $USER
# Logout and login

# Required for typing output
systemctl --user enable --now ydotool

# Download whisper model
mkdir -p ~/.local/share/voxtype/models
wget https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.en.bin \
  -O ~/.local/share/voxtype/models/ggml-base.en.bin
```

## Common Issues and Solutions

### "Cannot open input device"
User not in input group. Run: `sudo usermod -aG input $USER` then logout/login.

### "ydotool daemon not running"
Start the daemon: `systemctl --user enable --now ydotool`

### "Model not found"
Download the model or run `voxtype setup --download`

### Audio device not found
Check PipeWire/PulseAudio is running: `pactl info`

### whisper-rs build fails
May need cmake and a C++ compiler:
```bash
# Fedora
sudo dnf install cmake gcc-c++

# Ubuntu
sudo apt install cmake g++
```

## Module Dependencies Graph

```
main.rs
  ├── config.rs
  ├── daemon.rs
  │     ├── hotkey/
  │     │     └── evdev_listener.rs
  │     ├── audio/
  │     │     └── cpal_capture.rs
  │     ├── transcribe/
  │     │     └── whisper.rs
  │     ├── output/
  │     │     ├── ydotool.rs
  │     │     └── clipboard.rs
  │     └── state.rs
  └── error.rs
```

## Parallel Task Assignments

For a multi-agent setup, these tasks can be parallelized:

**Agent A**: Tasks 1.2, 1.3, 1.4 (Foundation)
**Agent B**: Task 2.1 (Hotkey)
**Agent C**: Task 2.2 (Audio)  
**Agent D**: Task 2.3 (Output)

After Phase 2, Agent A can take Task 3.1 (Whisper), then all agents converge for Phase 4-5.
