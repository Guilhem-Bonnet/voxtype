# Voxtype: Wayland Voice-to-Text Tool

## Project Overview

**Voxtype** is a push-to-talk voice-to-text daemon for Wayland Linux systems. It captures audio while a hotkey is held, transcribes it locally using whisper.cpp, and either types the result at the cursor position or copies it to the clipboard.

### Design Principles

1. **Minimal fragile dependencies**: Prefer kernel-level APIs (evdev, uinput) over compositor-specific protocols
2. **Graceful degradation**: Fallback chains for text injection
3. **Offline-first**: All transcription happens locally via whisper.cpp
4. **Single binary**: No runtime interpreters or external scripts

---

## Architecture

```
voxtype/
├── Cargo.toml
├── build.rs                    # whisper.cpp build configuration
├── config/
│   └── default.toml            # Default configuration
├── src/
│   ├── main.rs                 # Entry point, CLI parsing, daemon setup
│   ├── lib.rs                  # Library root, re-exports
│   ├── config.rs               # Configuration loading/parsing
│   ├── error.rs                # Error types (thiserror)
│   ├── state.rs                # State machine definition
│   ├── hotkey/
│   │   ├── mod.rs              # Hotkey detection trait + factory
│   │   └── evdev.rs            # evdev-based implementation
│   ├── audio/
│   │   ├── mod.rs              # Audio capture trait + factory
│   │   └── pipewire.rs         # PipeWire implementation
│   ├── transcribe/
│   │   ├── mod.rs              # Transcription trait + factory
│   │   └── whisper.rs          # whisper.cpp implementation
│   ├── output/
│   │   ├── mod.rs              # Text output trait + factory
│   │   ├── ydotool.rs          # ydotool client implementation
│   │   ├── clipboard.rs        # wl-copy fallback
│   │   └── wtype.rs            # wtype for wlroots (optional)
│   └── daemon.rs               # Main event loop orchestration
└── models/
    └── .gitkeep                # Whisper models downloaded here
```

---

## Module Specifications

### 1. `src/main.rs`

**Responsibilities:**
- Parse CLI arguments (clap)
- Load configuration
- Initialize logging (tracing)
- Spawn daemon or run one-shot mode
- Handle signals (SIGTERM, SIGINT)

**CLI Interface:**
```
voxtype [OPTIONS] [COMMAND]

Commands:
  daemon      Run as background daemon (default)
  transcribe  One-shot: transcribe a file or stdin
  setup       Interactive setup (download models, check permissions)

Options:
  -c, --config <PATH>    Config file path [default: ~/.config/voxtype/config.toml]
  -v, --verbose          Increase logging verbosity
  -q, --quiet            Suppress non-error output
  --clipboard            Force clipboard mode (no typing)
  --model <MODEL>        Override whisper model [tiny, base, small, medium, large]
  --hotkey <KEY>         Override hotkey [default: ScrollLock]
```

**Implementation Notes:**
- Use `tokio` for async runtime
- Use `clap` with derive macros for CLI
- Use `directories` crate for XDG paths

---

### 2. `src/config.rs`

**Configuration Structure:**
```rust
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub hotkey: HotkeyConfig,
    pub audio: AudioConfig,
    pub whisper: WhisperConfig,
    pub output: OutputConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HotkeyConfig {
    /// Key name (evdev KEY_* constant name without prefix)
    /// Examples: "SCROLLLOCK", "RIGHTALT", "PAUSE", "F24"
    pub key: String,
    
    /// Optional modifier keys that must also be held
    pub modifiers: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AudioConfig {
    /// PipeWire/PulseAudio device name, or "default"
    pub device: String,
    
    /// Sample rate (whisper expects 16000)
    pub sample_rate: u32,
    
    /// Maximum recording duration in seconds (safety limit)
    pub max_duration_secs: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct WhisperConfig {
    /// Model name: tiny, base, small, medium, large-v3
    /// Or absolute path to .bin file
    pub model: String,
    
    /// Language code (en, es, fr, auto, etc.)
    pub language: String,
    
    /// Translate to English (if language != en)
    pub translate: bool,
    
    /// Number of threads for inference
    pub threads: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct OutputConfig {
    /// Primary output mode: "type" or "clipboard"
    pub mode: OutputMode,
    
    /// Fall back to clipboard if typing fails
    pub fallback_to_clipboard: bool,
    
    /// Show desktop notification with transcribed text
    pub notification: bool,
    
    /// Delay between typed characters (ms), 0 for fastest
    pub type_delay_ms: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum OutputMode {
    Type,
    Clipboard,
}
```

**Default Configuration File (`config/default.toml`):**
```toml
[hotkey]
key = "SCROLLLOCK"
modifiers = []

[audio]
device = "default"
sample_rate = 16000
max_duration_secs = 60

[whisper]
model = "base.en"
language = "en"
translate = false
# threads = 4  # Uncomment to override auto-detection

[output]
mode = "type"
fallback_to_clipboard = true
notification = true
type_delay_ms = 0
```

**Implementation Notes:**
- Use `config` crate for layered config (defaults → file → env → CLI)
- Config file location: `$XDG_CONFIG_HOME/voxtype/config.toml`
- Models location: `$XDG_DATA_HOME/voxtype/models/`

---

### 3. `src/error.rs`

**Error Types:**
```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum VoxtypeError {
    #[error("Configuration error: {0}")]
    Config(#[from] config::ConfigError),
    
    #[error("Hotkey error: {0}")]
    Hotkey(#[from] HotkeyError),
    
    #[error("Audio capture error: {0}")]
    Audio(#[from] AudioError),
    
    #[error("Transcription error: {0}")]
    Transcribe(#[from] TranscribeError),
    
    #[error("Output error: {0}")]
    Output(#[from] OutputError),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Error, Debug)]
pub enum HotkeyError {
    #[error("Cannot open input device: {0}. Is the user in the 'input' group?")]
    DeviceAccess(String),
    
    #[error("Unknown key name: {0}")]
    UnknownKey(String),
    
    #[error("No keyboard device found")]
    NoKeyboard,
    
    #[error("evdev error: {0}")]
    Evdev(#[from] evdev::Error),
}

#[derive(Error, Debug)]
pub enum AudioError {
    #[error("PipeWire connection failed: {0}")]
    Connection(String),
    
    #[error("Audio device not found: {0}")]
    DeviceNotFound(String),
    
    #[error("Recording timeout: exceeded {0} seconds")]
    Timeout(u32),
    
    #[error("No audio captured")]
    EmptyRecording,
}

#[derive(Error, Debug)]
pub enum TranscribeError {
    #[error("Model not found: {0}. Run 'voxtype setup' to download models.")]
    ModelNotFound(String),
    
    #[error("Whisper initialization failed: {0}")]
    InitFailed(String),
    
    #[error("Transcription failed: {0}")]
    InferenceFailed(String),
}

#[derive(Error, Debug)]
pub enum OutputError {
    #[error("ydotool daemon not running. Start with: systemctl --user start ydotool")]
    YdotoolNotRunning,
    
    #[error("wl-copy not found in PATH")]
    WlCopyNotFound,
    
    #[error("Text injection failed: {0}")]
    InjectionFailed(String),
    
    #[error("All output methods failed")]
    AllMethodsFailed,
}

pub type Result<T> = std::result::Result<T, VoxtypeError>;
```

---

### 4. `src/state.rs`

**State Machine:**
```rust
use std::time::Instant;

/// Audio samples collected during recording
pub type AudioBuffer = Vec<f32>;

#[derive(Debug, Clone)]
pub enum State {
    /// Waiting for hotkey press
    Idle,
    
    /// Hotkey held, recording audio
    Recording {
        started_at: Instant,
    },
    
    /// Hotkey released, transcribing audio
    Transcribing {
        audio: AudioBuffer,
    },
    
    /// Transcription complete, outputting text
    Outputting {
        text: String,
    },
    
    /// Error state (will transition back to Idle)
    Error {
        message: String,
    },
}

#[derive(Debug, Clone)]
pub enum Event {
    /// Hotkey was pressed
    HotkeyPressed,
    
    /// Hotkey was released
    HotkeyReleased,
    
    /// Audio chunk received during recording
    AudioChunk(Vec<f32>),
    
    /// Recording complete with full audio buffer
    RecordingComplete(AudioBuffer),
    
    /// Transcription complete with text
    TranscriptionComplete(String),
    
    /// Text output complete
    OutputComplete,
    
    /// An error occurred
    Error(String),
    
    /// Recording timeout reached
    Timeout,
}

impl State {
    pub fn transition(self, event: Event) -> State {
        use State::*;
        use Event::*;
        
        match (self, event) {
            // Idle state transitions
            (Idle, HotkeyPressed) => Recording { 
                started_at: Instant::now() 
            },
            (Idle, _) => Idle, // Ignore other events in idle
            
            // Recording state transitions
            (Recording { .. }, HotkeyReleased) => Idle, // Will receive RecordingComplete
            (Recording { started_at }, RecordingComplete(audio)) => {
                if audio.is_empty() {
                    Error { message: "No audio captured".into() }
                } else {
                    Transcribing { audio }
                }
            },
            (Recording { .. }, Timeout) => Error { 
                message: "Recording timeout".into() 
            },
            (Recording { started_at }, _) => Recording { started_at }, // Stay recording
            
            // Transcribing state transitions
            (Transcribing { .. }, TranscriptionComplete(text)) => {
                if text.trim().is_empty() {
                    Error { message: "Transcription was empty".into() }
                } else {
                    Outputting { text }
                }
            },
            (Transcribing { .. }, Event::Error(msg)) => Error { message: msg },
            (Transcribing { audio }, _) => Transcribing { audio },
            
            // Outputting state transitions
            (Outputting { .. }, OutputComplete) => Idle,
            (Outputting { .. }, Event::Error(msg)) => Error { message: msg },
            (Outputting { text }, _) => Outputting { text },
            
            // Error state always transitions back to Idle
            (Error { .. }, _) => Idle,
        }
    }
    
    pub fn is_idle(&self) -> bool {
        matches!(self, State::Idle)
    }
    
    pub fn is_recording(&self) -> bool {
        matches!(self, State::Recording { .. })
    }
}
```

---

### 5. `src/hotkey/mod.rs` and `src/hotkey/evdev.rs`

**Trait Definition (`mod.rs`):**
```rust
use crate::error::HotkeyError;
use tokio::sync::mpsc;

/// Events emitted by the hotkey listener
#[derive(Debug, Clone)]
pub enum HotkeyEvent {
    Pressed,
    Released,
}

/// Trait for hotkey detection implementations
#[async_trait::async_trait]
pub trait HotkeyListener: Send + Sync {
    /// Start listening for hotkey events
    /// Returns a channel receiver for events
    async fn start(&mut self) -> Result<mpsc::Receiver<HotkeyEvent>, HotkeyError>;
    
    /// Stop listening
    async fn stop(&mut self) -> Result<(), HotkeyError>;
}

/// Factory function to create the appropriate hotkey listener
pub fn create_listener(config: &crate::config::HotkeyConfig) -> Result<Box<dyn HotkeyListener>, HotkeyError> {
    Ok(Box::new(evdev::EvdevListener::new(config)?))
}

pub mod evdev;
```

**evdev Implementation (`evdev.rs`):**
```rust
use super::{HotkeyEvent, HotkeyListener};
use crate::config::HotkeyConfig;
use crate::error::HotkeyError;
use evdev::{Device, InputEventKind, Key};
use std::collections::HashSet;
use std::path::PathBuf;
use tokio::sync::mpsc;

pub struct EvdevListener {
    target_key: Key,
    modifier_keys: HashSet<Key>,
    device_paths: Vec<PathBuf>,
    stop_signal: Option<tokio::sync::oneshot::Sender<()>>,
}

impl EvdevListener {
    pub fn new(config: &HotkeyConfig) -> Result<Self, HotkeyError> {
        let target_key = parse_key_name(&config.key)?;
        let modifier_keys = config.modifiers
            .iter()
            .map(|k| parse_key_name(k))
            .collect::<Result<HashSet<_>, _>>()?;
        
        let device_paths = find_keyboard_devices()?;
        
        if device_paths.is_empty() {
            return Err(HotkeyError::NoKeyboard);
        }
        
        Ok(Self {
            target_key,
            modifier_keys,
            device_paths,
            stop_signal: None,
        })
    }
}

#[async_trait::async_trait]
impl HotkeyListener for EvdevListener {
    async fn start(&mut self) -> Result<mpsc::Receiver<HotkeyEvent>, HotkeyError> {
        let (tx, rx) = mpsc::channel(32);
        let (stop_tx, stop_rx) = tokio::sync::oneshot::channel();
        self.stop_signal = Some(stop_tx);
        
        let target_key = self.target_key;
        let modifier_keys = self.modifier_keys.clone();
        let device_paths = self.device_paths.clone();
        
        tokio::spawn(async move {
            evdev_listener_task(device_paths, target_key, modifier_keys, tx, stop_rx).await;
        });
        
        Ok(rx)
    }
    
    async fn stop(&mut self) -> Result<(), HotkeyError> {
        if let Some(stop) = self.stop_signal.take() {
            let _ = stop.send(());
        }
        Ok(())
    }
}

async fn evdev_listener_task(
    device_paths: Vec<PathBuf>,
    target_key: Key,
    modifier_keys: HashSet<Key>,
    tx: mpsc::Sender<HotkeyEvent>,
    mut stop_rx: tokio::sync::oneshot::Receiver<()>,
) {
    // Open all keyboard devices
    let mut devices: Vec<Device> = device_paths
        .iter()
        .filter_map(|path| {
            Device::open(path)
                .map_err(|e| tracing::warn!("Failed to open {:?}: {}", path, e))
                .ok()
        })
        .collect();
    
    if devices.is_empty() {
        tracing::error!("No keyboard devices could be opened");
        return;
    }
    
    // Track modifier state
    let mut active_modifiers: HashSet<Key> = HashSet::new();
    
    loop {
        tokio::select! {
            _ = &mut stop_rx => {
                tracing::debug!("Hotkey listener stopping");
                break;
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(10)) => {
                for device in &mut devices {
                    if let Ok(events) = device.fetch_events() {
                        for event in events {
                            if let InputEventKind::Key(key) = event.kind() {
                                let value = event.value();
                                
                                // Track modifiers
                                if modifier_keys.contains(&key) {
                                    if value == 1 {
                                        active_modifiers.insert(key);
                                    } else if value == 0 {
                                        active_modifiers.remove(&key);
                                    }
                                }
                                
                                // Check target key
                                if key == target_key {
                                    let modifiers_satisfied = modifier_keys
                                        .iter()
                                        .all(|m| active_modifiers.contains(m));
                                    
                                    if modifiers_satisfied {
                                        let event = match value {
                                            1 => Some(HotkeyEvent::Pressed),
                                            0 => Some(HotkeyEvent::Released),
                                            _ => None, // Ignore key repeat (value == 2)
                                        };
                                        
                                        if let Some(e) = event {
                                            if tx.send(e).await.is_err() {
                                                return; // Channel closed
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Find all keyboard input devices
fn find_keyboard_devices() -> Result<Vec<PathBuf>, HotkeyError> {
    let mut keyboards = Vec::new();
    
    for entry in std::fs::read_dir("/dev/input")
        .map_err(|e| HotkeyError::DeviceAccess(e.to_string()))? 
    {
        let entry = entry.map_err(|e| HotkeyError::DeviceAccess(e.to_string()))?;
        let path = entry.path();
        
        if path.file_name()
            .and_then(|n| n.to_str())
            .map(|n| n.starts_with("event"))
            .unwrap_or(false)
        {
            if let Ok(device) = Device::open(&path) {
                // Check if device has keyboard capabilities
                if device.supported_keys()
                    .map(|keys| keys.contains(Key::KEY_A))
                    .unwrap_or(false)
                {
                    keyboards.push(path);
                }
            }
        }
    }
    
    Ok(keyboards)
}

/// Parse a key name string to evdev Key
fn parse_key_name(name: &str) -> Result<Key, HotkeyError> {
    // Handle common names
    let normalized = name.to_uppercase();
    let key_name = if normalized.starts_with("KEY_") {
        normalized
    } else {
        format!("KEY_{}", normalized)
    };
    
    // Try to match against known keys
    // This is a subset - expand as needed
    match key_name.as_str() {
        "KEY_SCROLLLOCK" => Ok(Key::KEY_SCROLLLOCK),
        "KEY_PAUSE" => Ok(Key::KEY_PAUSE),
        "KEY_RIGHTALT" => Ok(Key::KEY_RIGHTALT),
        "KEY_LEFTALT" => Ok(Key::KEY_LEFTALT),
        "KEY_RIGHTCTRL" => Ok(Key::KEY_RIGHTCTRL),
        "KEY_LEFTCTRL" => Ok(Key::KEY_LEFTCTRL),
        "KEY_RIGHTSHIFT" => Ok(Key::KEY_RIGHTSHIFT),
        "KEY_LEFTSHIFT" => Ok(Key::KEY_LEFTSHIFT),
        "KEY_LEFTMETA" => Ok(Key::KEY_LEFTMETA),
        "KEY_RIGHTMETA" => Ok(Key::KEY_RIGHTMETA),
        "KEY_F13" => Ok(Key::KEY_F13),
        "KEY_F14" => Ok(Key::KEY_F14),
        "KEY_F15" => Ok(Key::KEY_F15),
        "KEY_F16" => Ok(Key::KEY_F16),
        "KEY_F17" => Ok(Key::KEY_F17),
        "KEY_F18" => Ok(Key::KEY_F18),
        "KEY_F19" => Ok(Key::KEY_F19),
        "KEY_F20" => Ok(Key::KEY_F20),
        "KEY_F21" => Ok(Key::KEY_F21),
        "KEY_F22" => Ok(Key::KEY_F22),
        "KEY_F23" => Ok(Key::KEY_F23),
        "KEY_F24" => Ok(Key::KEY_F24),
        _ => Err(HotkeyError::UnknownKey(name.to_string())),
    }
}
```

---

### 6. `src/audio/mod.rs` and `src/audio/pipewire.rs`

**Trait Definition (`mod.rs`):**
```rust
use crate::error::AudioError;
use tokio::sync::mpsc;

/// Trait for audio capture implementations
#[async_trait::async_trait]
pub trait AudioCapture: Send + Sync {
    /// Start capturing audio
    /// Returns a channel receiver for audio chunks (f32 samples, mono, 16kHz)
    async fn start(&mut self) -> Result<mpsc::Receiver<Vec<f32>>, AudioError>;
    
    /// Stop capturing and return all recorded samples
    async fn stop(&mut self) -> Result<Vec<f32>, AudioError>;
}

/// Factory function to create audio capture
pub fn create_capture(config: &crate::config::AudioConfig) -> Result<Box<dyn AudioCapture>, AudioError> {
    Ok(Box::new(pipewire::PipeWireCapture::new(config)?))
}

pub mod pipewire;
```

**PipeWire Implementation (`pipewire.rs`):**
```rust
use super::AudioCapture;
use crate::config::AudioConfig;
use crate::error::AudioError;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

/// PipeWire-based audio capture
/// 
/// Note: This implementation uses the `cpal` crate which provides
/// cross-platform audio I/O and works with PipeWire via its ALSA
/// or native backends. This is more portable than raw pipewire-rs.
pub struct PipeWireCapture {
    config: AudioConfig,
    samples: Arc<Mutex<Vec<f32>>>,
    stream: Option<cpal::Stream>,
    stop_signal: Option<tokio::sync::oneshot::Sender<()>>,
}

impl PipeWireCapture {
    pub fn new(config: &AudioConfig) -> Result<Self, AudioError> {
        Ok(Self {
            config: config.clone(),
            samples: Arc::new(Mutex::new(Vec::new())),
            stream: None,
            stop_signal: None,
        })
    }
}

#[async_trait::async_trait]
impl AudioCapture for PipeWireCapture {
    async fn start(&mut self) -> Result<mpsc::Receiver<Vec<f32>>, AudioError> {
        use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
        
        let host = cpal::default_host();
        
        let device = if self.config.device == "default" {
            host.default_input_device()
        } else {
            host.input_devices()
                .map_err(|e| AudioError::Connection(e.to_string()))?
                .find(|d| d.name().map(|n| n == self.config.device).unwrap_or(false))
        }.ok_or_else(|| AudioError::DeviceNotFound(self.config.device.clone()))?;
        
        let supported_config = device.default_input_config()
            .map_err(|e| AudioError::Connection(e.to_string()))?;
        
        let (tx, rx) = mpsc::channel(64);
        let samples = self.samples.clone();
        let target_sample_rate = self.config.sample_rate;
        let source_sample_rate = supported_config.sample_rate().0;
        
        let stream_config = cpal::StreamConfig {
            channels: 1, // Mono
            sample_rate: supported_config.sample_rate(),
            buffer_size: cpal::BufferSize::Default,
        };
        
        let err_fn = |err| tracing::error!("Audio stream error: {}", err);
        
        let stream = device.build_input_stream(
            &stream_config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                // Resample to 16kHz if necessary
                let resampled = if source_sample_rate != target_sample_rate {
                    resample(data, source_sample_rate, target_sample_rate)
                } else {
                    data.to_vec()
                };
                
                // Store samples
                if let Ok(mut buffer) = samples.lock() {
                    buffer.extend_from_slice(&resampled);
                }
                
                // Send chunk for streaming (if needed)
                let _ = tx.try_send(resampled);
            },
            err_fn,
            None,
        ).map_err(|e| AudioError::Connection(e.to_string()))?;
        
        stream.play().map_err(|e| AudioError::Connection(e.to_string()))?;
        self.stream = Some(stream);
        
        Ok(rx)
    }
    
    async fn stop(&mut self) -> Result<Vec<f32>, AudioError> {
        // Drop the stream to stop recording
        self.stream.take();
        
        let samples = self.samples.lock()
            .map_err(|_| AudioError::EmptyRecording)?
            .drain(..)
            .collect::<Vec<_>>();
        
        if samples.is_empty() {
            return Err(AudioError::EmptyRecording);
        }
        
        Ok(samples)
    }
}

/// Simple linear resampling (for better quality, use rubato or libsamplerate)
fn resample(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate {
        return samples.to_vec();
    }
    
    let ratio = to_rate as f64 / from_rate as f64;
    let new_len = (samples.len() as f64 * ratio) as usize;
    let mut output = Vec::with_capacity(new_len);
    
    for i in 0..new_len {
        let src_idx = i as f64 / ratio;
        let idx = src_idx as usize;
        let frac = src_idx - idx as f64;
        
        let sample = if idx + 1 < samples.len() {
            samples[idx] * (1.0 - frac as f32) + samples[idx + 1] * frac as f32
        } else {
            samples.get(idx).copied().unwrap_or(0.0)
        };
        
        output.push(sample);
    }
    
    output
}
```

---

### 7. `src/transcribe/mod.rs` and `src/transcribe/whisper.rs`

**Trait Definition (`mod.rs`):**
```rust
use crate::error::TranscribeError;

/// Trait for speech-to-text implementations
pub trait Transcriber: Send + Sync {
    /// Transcribe audio samples to text
    /// Input: f32 samples, mono, 16kHz
    fn transcribe(&self, samples: &[f32]) -> Result<String, TranscribeError>;
}

/// Factory function to create transcriber
pub fn create_transcriber(config: &crate::config::WhisperConfig) -> Result<Box<dyn Transcriber>, TranscribeError> {
    Ok(Box::new(whisper::WhisperTranscriber::new(config)?))
}

pub mod whisper;
```

**Whisper Implementation (`whisper.rs`):**
```rust
use super::Transcriber;
use crate::config::WhisperConfig;
use crate::error::TranscribeError;
use std::path::PathBuf;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

pub struct WhisperTranscriber {
    ctx: WhisperContext,
    language: String,
    translate: bool,
}

impl WhisperTranscriber {
    pub fn new(config: &WhisperConfig) -> Result<Self, TranscribeError> {
        let model_path = resolve_model_path(&config.model)?;
        
        tracing::info!("Loading whisper model from {:?}", model_path);
        
        let ctx = WhisperContext::new_with_params(
            model_path.to_str().unwrap(),
            WhisperContextParameters::default(),
        ).map_err(|e| TranscribeError::InitFailed(e.to_string()))?;
        
        Ok(Self {
            ctx,
            language: config.language.clone(),
            translate: config.translate,
        })
    }
}

impl Transcriber for WhisperTranscriber {
    fn transcribe(&self, samples: &[f32]) -> Result<String, TranscribeError> {
        let mut state = self.ctx.create_state()
            .map_err(|e| TranscribeError::InferenceFailed(e.to_string()))?;
        
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        
        // Configure parameters
        params.set_language(Some(&self.language));
        params.set_translate(self.translate);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_suppress_blank(true);
        params.set_suppress_non_speech_tokens(true);
        
        // Single segment mode for short recordings
        params.set_single_segment(true);
        
        // Run inference
        state.full(params, samples)
            .map_err(|e| TranscribeError::InferenceFailed(e.to_string()))?;
        
        // Collect all segments
        let num_segments = state.full_n_segments()
            .map_err(|e| TranscribeError::InferenceFailed(e.to_string()))?;
        
        let mut text = String::new();
        for i in 0..num_segments {
            if let Ok(segment) = state.full_get_segment_text(i) {
                text.push_str(&segment);
            }
        }
        
        Ok(text.trim().to_string())
    }
}

/// Resolve model name to file path
fn resolve_model_path(model: &str) -> Result<PathBuf, TranscribeError> {
    // If it's already a path, use it directly
    let path = PathBuf::from(model);
    if path.exists() {
        return Ok(path);
    }
    
    // Otherwise, look in the data directory
    let data_dir = directories::ProjectDirs::from("", "", "voxtype")
        .map(|d| d.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    
    let model_name = match model {
        "tiny" | "tiny.en" => "ggml-tiny.en.bin",
        "base" | "base.en" => "ggml-base.en.bin",
        "small" | "small.en" => "ggml-small.en.bin",
        "medium" | "medium.en" => "ggml-medium.en.bin",
        "large" | "large-v3" => "ggml-large-v3.bin",
        other => other,
    };
    
    let model_path = data_dir.join("models").join(model_name);
    
    if model_path.exists() {
        Ok(model_path)
    } else {
        Err(TranscribeError::ModelNotFound(model_path.display().to_string()))
    }
}
```

---

### 8. `src/output/mod.rs` and implementations

**Trait Definition (`mod.rs`):**
```rust
use crate::error::OutputError;

/// Trait for text output implementations
#[async_trait::async_trait]
pub trait TextOutput: Send + Sync {
    /// Output text (type it or copy to clipboard)
    async fn output(&self, text: &str) -> Result<(), OutputError>;
    
    /// Check if this output method is available
    async fn is_available(&self) -> bool;
    
    /// Human-readable name for logging
    fn name(&self) -> &'static str;
}

/// Factory function that returns a fallback chain of output methods
pub fn create_output_chain(config: &crate::config::OutputConfig) -> Vec<Box<dyn TextOutput>> {
    let mut chain: Vec<Box<dyn TextOutput>> = Vec::new();
    
    match config.mode {
        crate::config::OutputMode::Type => {
            chain.push(Box::new(ydotool::YdotoolOutput::new(config.type_delay_ms)));
            if config.fallback_to_clipboard {
                chain.push(Box::new(clipboard::ClipboardOutput::new(config.notification)));
            }
        }
        crate::config::OutputMode::Clipboard => {
            chain.push(Box::new(clipboard::ClipboardOutput::new(config.notification)));
        }
    }
    
    chain
}

pub mod ydotool;
pub mod clipboard;
```

**ydotool Implementation (`ydotool.rs`):**
```rust
use super::TextOutput;
use crate::error::OutputError;
use std::process::Stdio;
use tokio::process::Command;

pub struct YdotoolOutput {
    delay_ms: u32,
}

impl YdotoolOutput {
    pub fn new(delay_ms: u32) -> Self {
        Self { delay_ms }
    }
}

#[async_trait::async_trait]
impl TextOutput for YdotoolOutput {
    async fn output(&self, text: &str) -> Result<(), OutputError> {
        let mut cmd = Command::new("ydotool");
        cmd.arg("type");
        
        if self.delay_ms > 0 {
            cmd.arg("--key-delay").arg(self.delay_ms.to_string());
        }
        
        cmd.arg("--").arg(text);
        
        let output = cmd
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| OutputError::InjectionFailed(e.to_string()))?;
        
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            if stderr.contains("socket") || stderr.contains("connect") {
                return Err(OutputError::YdotoolNotRunning);
            }
            return Err(OutputError::InjectionFailed(stderr.to_string()));
        }
        
        Ok(())
    }
    
    async fn is_available(&self) -> bool {
        // Check if ydotool is in PATH and daemon is running
        Command::new("ydotool")
            .arg("type")
            .arg("")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .map(|s| s.success())
            .unwrap_or(false)
    }
    
    fn name(&self) -> &'static str {
        "ydotool"
    }
}
```

**Clipboard Implementation (`clipboard.rs`):**
```rust
use super::TextOutput;
use crate::error::OutputError;
use std::process::Stdio;
use tokio::process::Command;
use tokio::io::AsyncWriteExt;

pub struct ClipboardOutput {
    notify: bool,
}

impl ClipboardOutput {
    pub fn new(notify: bool) -> Self {
        Self { notify }
    }
}

#[async_trait::async_trait]
impl TextOutput for ClipboardOutput {
    async fn output(&self, text: &str) -> Result<(), OutputError> {
        // Use wl-copy
        let mut child = Command::new("wl-copy")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    OutputError::WlCopyNotFound
                } else {
                    OutputError::InjectionFailed(e.to_string())
                }
            })?;
        
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(text.as_bytes()).await
                .map_err(|e| OutputError::InjectionFailed(e.to_string()))?;
        }
        
        let status = child.wait().await
            .map_err(|e| OutputError::InjectionFailed(e.to_string()))?;
        
        if !status.success() {
            return Err(OutputError::InjectionFailed("wl-copy failed".to_string()));
        }
        
        // Send notification if enabled
        if self.notify {
            let preview = if text.len() > 50 {
                format!("{}...", &text[..50])
            } else {
                text.to_string()
            };
            
            let _ = Command::new("notify-send")
                .args(["Voxtype", &format!("Copied to clipboard: {}", preview)])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .await;
        }
        
        Ok(())
    }
    
    async fn is_available(&self) -> bool {
        Command::new("wl-copy")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .map(|s| s.success())
            .unwrap_or(false)
    }
    
    fn name(&self) -> &'static str {
        "clipboard (wl-copy)"
    }
}
```

---

### 9. `src/daemon.rs`

**Main Event Loop:**
```rust
use crate::config::Config;
use crate::error::{Result, VoxtypeError};
use crate::hotkey::{self, HotkeyEvent};
use crate::audio::{self, AudioCapture};
use crate::transcribe::{self, Transcriber};
use crate::output::{self, TextOutput};
use crate::state::{State, Event};
use std::sync::Arc;
use tokio::sync::mpsc;

pub struct Daemon {
    config: Config,
    state: State,
}

impl Daemon {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            state: State::Idle,
        }
    }
    
    pub async fn run(&mut self) -> Result<()> {
        tracing::info!("Starting voxtype daemon");
        
        // Initialize components
        let mut hotkey_listener = hotkey::create_listener(&self.config.hotkey)?;
        let output_chain = output::create_output_chain(&self.config.output);
        
        // Pre-load whisper model (takes a few seconds)
        tracing::info!("Loading whisper model...");
        let transcriber = Arc::new(transcribe::create_transcriber(&self.config.whisper)?);
        tracing::info!("Model loaded, ready for input");
        
        // Start hotkey listener
        let mut hotkey_rx = hotkey_listener.start().await?;
        
        // Audio capture (created fresh for each recording)
        let mut audio_capture: Option<Box<dyn AudioCapture>> = None;
        
        // Main event loop
        loop {
            tokio::select! {
                Some(hotkey_event) = hotkey_rx.recv() => {
                    match hotkey_event {
                        HotkeyEvent::Pressed => {
                            if self.state.is_idle() {
                                tracing::debug!("Hotkey pressed, starting recording");
                                
                                match audio::create_capture(&self.config.audio) {
                                    Ok(mut capture) => {
                                        if let Err(e) = capture.start().await {
                                            tracing::error!("Failed to start audio: {}", e);
                                            continue;
                                        }
                                        audio_capture = Some(capture);
                                        self.state = self.state.clone().transition(Event::HotkeyPressed);
                                    }
                                    Err(e) => {
                                        tracing::error!("Failed to create audio capture: {}", e);
                                    }
                                }
                            }
                        }
                        HotkeyEvent::Released => {
                            if self.state.is_recording() {
                                tracing::debug!("Hotkey released, stopping recording");
                                
                                if let Some(mut capture) = audio_capture.take() {
                                    match capture.stop().await {
                                        Ok(samples) => {
                                            let duration_secs = samples.len() as f32 / 16000.0;
                                            tracing::info!("Recorded {:.1}s of audio", duration_secs);
                                            
                                            // Transcribe in blocking task
                                            let transcriber = transcriber.clone();
                                            let text = tokio::task::spawn_blocking(move || {
                                                transcriber.transcribe(&samples)
                                            }).await
                                                .map_err(|e| VoxtypeError::Transcribe(
                                                    crate::error::TranscribeError::InferenceFailed(e.to_string())
                                                ))??;
                                            
                                            tracing::info!("Transcribed: {:?}", text);
                                            
                                            // Output text
                                            if !text.is_empty() {
                                                output_text(&output_chain, &text).await?;
                                            }
                                        }
                                        Err(e) => {
                                            tracing::warn!("Recording error: {}", e);
                                        }
                                    }
                                }
                                
                                self.state = State::Idle;
                            }
                        }
                    }
                }
                
                // Handle graceful shutdown
                _ = tokio::signal::ctrl_c() => {
                    tracing::info!("Received interrupt, shutting down");
                    break;
                }
            }
        }
        
        hotkey_listener.stop().await?;
        Ok(())
    }
}

async fn output_text(chain: &[Box<dyn TextOutput>], text: &str) -> Result<()> {
    for output in chain {
        if !output.is_available().await {
            tracing::debug!("{} not available, trying next", output.name());
            continue;
        }
        
        match output.output(text).await {
            Ok(()) => {
                tracing::debug!("Text output via {}", output.name());
                return Ok(());
            }
            Err(e) => {
                tracing::warn!("{} failed: {}, trying next", output.name(), e);
            }
        }
    }
    
    Err(crate::error::OutputError::AllMethodsFailed.into())
}
```

---

### 10. `src/main.rs`

**Entry Point:**
```rust
mod config;
mod error;
mod state;
mod hotkey;
mod audio;
mod transcribe;
mod output;
mod daemon;

use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
#[command(name = "voxtype")]
#[command(about = "Push-to-talk voice-to-text for Wayland")]
#[command(version)]
struct Cli {
    /// Config file path
    #[arg(short, long)]
    config: Option<PathBuf>,
    
    /// Increase verbosity (-v, -vv, -vvv)
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
    
    /// Quiet mode (errors only)
    #[arg(short, long)]
    quiet: bool,
    
    /// Force clipboard mode
    #[arg(long)]
    clipboard: bool,
    
    /// Override whisper model
    #[arg(long)]
    model: Option<String>,
    
    /// Override hotkey
    #[arg(long)]
    hotkey: Option<String>,
    
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run as daemon (default)
    Daemon,
    
    /// Transcribe audio file
    Transcribe {
        /// Audio file path (WAV, 16kHz mono)
        file: PathBuf,
    },
    
    /// Interactive setup
    Setup,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    
    // Initialize logging
    let log_level = if cli.quiet {
        "error"
    } else {
        match cli.verbose {
            0 => "info",
            1 => "debug",
            _ => "trace",
        }
    };
    
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(format!("voxtype={}", log_level)))
        .init();
    
    // Load configuration
    let mut config = config::load_config(cli.config.as_deref())?;
    
    // Apply CLI overrides
    if cli.clipboard {
        config.output.mode = config::OutputMode::Clipboard;
    }
    if let Some(model) = cli.model {
        config.whisper.model = model;
    }
    if let Some(hotkey) = cli.hotkey {
        config.hotkey.key = hotkey;
    }
    
    // Run command
    match cli.command.unwrap_or(Commands::Daemon) {
        Commands::Daemon => {
            let mut daemon = daemon::Daemon::new(config);
            daemon.run().await?;
        }
        Commands::Transcribe { file } => {
            transcribe_file(&config, &file).await?;
        }
        Commands::Setup => {
            run_setup(&config).await?;
        }
    }
    
    Ok(())
}

async fn transcribe_file(config: &config::Config, path: &PathBuf) -> anyhow::Result<()> {
    use hound::WavReader;
    
    let reader = WavReader::open(path)?;
    let spec = reader.spec();
    
    if spec.channels != 1 || spec.sample_rate != 16000 {
        anyhow::bail!("Audio must be mono 16kHz WAV. Got: {} channels, {} Hz", 
            spec.channels, spec.sample_rate);
    }
    
    let samples: Vec<f32> = reader
        .into_samples::<i16>()
        .filter_map(|s| s.ok())
        .map(|s| s as f32 / 32768.0)
        .collect();
    
    let transcriber = transcribe::create_transcriber(&config.whisper)?;
    let text = transcriber.transcribe(&samples)?;
    
    println!("{}", text);
    Ok(())
}

async fn run_setup(config: &config::Config) -> anyhow::Result<()> {
    println!("Voxtype Setup\n");
    
    // Check input group
    println!("Checking input group membership...");
    let groups = std::process::Command::new("groups")
        .output()?;
    let groups_str = String::from_utf8_lossy(&groups.stdout);
    if groups_str.contains("input") {
        println!("  ✓ User is in 'input' group");
    } else {
        println!("  ✗ User is NOT in 'input' group");
        println!("    Run: sudo usermod -aG input $USER");
        println!("    Then log out and back in");
    }
    
    // Check ydotool
    println!("\nChecking ydotool...");
    let ydotool = tokio::process::Command::new("which")
        .arg("ydotool")
        .output()
        .await?;
    if ydotool.status.success() {
        println!("  ✓ ydotool found");
        
        // Check daemon
        let daemon = tokio::process::Command::new("systemctl")
            .args(["--user", "is-active", "ydotool"])
            .output()
            .await?;
        if daemon.status.success() {
            println!("  ✓ ydotool daemon running");
        } else {
            println!("  ✗ ydotool daemon not running");
            println!("    Run: systemctl --user enable --now ydotool");
        }
    } else {
        println!("  ✗ ydotool not found");
        println!("    Install via your package manager");
    }
    
    // Check wl-copy
    println!("\nChecking wl-clipboard...");
    let wlcopy = tokio::process::Command::new("which")
        .arg("wl-copy")
        .output()
        .await?;
    if wlcopy.status.success() {
        println!("  ✓ wl-copy found");
    } else {
        println!("  ✗ wl-copy not found");
        println!("    Install wl-clipboard via your package manager");
    }
    
    // Check/download model
    println!("\nChecking whisper model...");
    let data_dir = directories::ProjectDirs::from("", "", "voxtype")
        .map(|d| d.data_dir().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));
    let models_dir = data_dir.join("models");
    let model_file = models_dir.join(format!("ggml-{}.bin", config.whisper.model));
    
    if model_file.exists() {
        println!("  ✓ Model found: {:?}", model_file);
    } else {
        println!("  ✗ Model not found: {:?}", model_file);
        println!("    Download from: https://huggingface.co/ggerganov/whisper.cpp/tree/main");
        println!("    Place in: {:?}", models_dir);
        
        // Offer to download
        println!("\n  Would you like to download the model now? (y/n)");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if input.trim().eq_ignore_ascii_case("y") {
            std::fs::create_dir_all(&models_dir)?;
            let url = format!(
                "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-{}.bin",
                config.whisper.model
            );
            println!("  Downloading from {}...", url);
            
            let response = reqwest::get(&url).await?;
            let bytes = response.bytes().await?;
            std::fs::write(&model_file, &bytes)?;
            println!("  ✓ Downloaded to {:?}", model_file);
        }
    }
    
    println!("\nSetup complete!");
    Ok(())
}
```

---

### 11. `Cargo.toml`

```toml
[package]
name = "voxtype"
version = "0.1.0"
edition = "2021"
authors = ["Your Name"]
description = "Push-to-talk voice-to-text for Wayland"
license = "MIT"
repository = "https://github.com/yourname/voxtype"

[dependencies]
# Async runtime
tokio = { version = "1", features = ["full", "signal"] }

# CLI
clap = { version = "4", features = ["derive"] }

# Configuration
config = "0.14"
serde = { version = "1", features = ["derive"] }
toml = "0.8"
directories = "5"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Error handling
thiserror = "1"
anyhow = "1"

# Async traits
async-trait = "0.1"

# Input handling (evdev)
evdev = "0.12"

# Audio
cpal = "0.15"
hound = "3"  # WAV file support

# Whisper
whisper-rs = "0.12"

# HTTP (for model download in setup)
reqwest = { version = "0.12", features = ["blocking"] }

[build-dependencies]
# None needed if using whisper-rs (it handles whisper.cpp build)

[profile.release]
lto = true
codegen-units = 1
strip = true
```

---

### 12. `src/lib.rs`

```rust
//! Voxtype: Push-to-talk voice-to-text for Wayland
//!
//! This library provides the core functionality for capturing audio,
//! transcribing it using whisper.cpp, and outputting the text.

pub mod config;
pub mod error;
pub mod state;
pub mod hotkey;
pub mod audio;
pub mod transcribe;
pub mod output;
pub mod daemon;

pub use config::Config;
pub use error::{Result, VoxtypeError};
pub use daemon::Daemon;
```

---

## Implementation Notes for Claude Code

### Build Order

1. Start with `error.rs` and `config.rs` (no dependencies on other modules)
2. Implement `state.rs` (pure logic, easily testable)
3. Implement `hotkey/evdev.rs` (can test independently)
4. Implement `audio/pipewire.rs` (can test with `cpal` examples)
5. Implement `transcribe/whisper.rs` (can test with WAV files)
6. Implement `output/ydotool.rs` and `output/clipboard.rs`
7. Wire everything together in `daemon.rs`
8. Complete `main.rs` with CLI

### Testing Strategy

```rust
// Unit tests in each module
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_state_transitions() {
        let state = State::Idle;
        let state = state.transition(Event::HotkeyPressed);
        assert!(matches!(state, State::Recording { .. }));
    }
}
```

### Key Dependencies to Verify

Before starting, verify these crates exist and have compatible versions:

- `whisper-rs` - Check https://crates.io/crates/whisper-rs
- `evdev` - Check https://crates.io/crates/evdev  
- `cpal` - Check https://crates.io/crates/cpal

### Environment Requirements

```bash
# Build dependencies (Fedora/RHEL)
sudo dnf install alsa-lib-devel pipewire-devel

# Build dependencies (Ubuntu/Debian)
sudo apt install libasound2-dev libpipewire-0.3-dev

# Runtime setup
sudo usermod -aG input $USER
systemctl --user enable --now ydotool
```
