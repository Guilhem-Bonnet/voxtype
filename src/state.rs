//! State machine for voxtype daemon
//!
//! Defines the states and transitions for the push-to-talk workflow:
//! Idle → Recording → Transcribing → Outputting → Idle

use std::time::Instant;

/// Audio samples collected during recording (f32, mono, 16kHz)
pub type AudioBuffer = Vec<f32>;

/// Application state
#[derive(Debug, Clone)]
pub enum State {
    /// Waiting for hotkey press
    Idle,

    /// Hotkey held, recording audio
    Recording {
        /// When recording started
        started_at: Instant,
    },

    /// Hotkey released, transcribing audio
    Transcribing {
        /// Recorded audio samples
        audio: AudioBuffer,
    },

    /// Transcription complete, outputting text
    Outputting {
        /// Transcribed text
        text: String,
    },

    /// Error state (will transition back to Idle)
    Error {
        /// Error description
        message: String,
    },
}

/// Events that trigger state transitions
#[derive(Debug, Clone)]
pub enum Event {
    /// Hotkey was pressed
    HotkeyPressed,

    /// Hotkey was released
    HotkeyReleased,

    /// Audio recording complete with samples
    RecordingComplete(AudioBuffer),

    /// Transcription complete with text
    TranscriptionComplete(String),

    /// Text output complete
    OutputComplete,

    /// An error occurred
    Error(String),

    /// Recording timeout reached
    Timeout,

    /// Reset to idle (e.g., after error)
    Reset,
}

impl State {
    /// Create a new idle state
    pub fn new() -> Self {
        State::Idle
    }

    /// Process an event and return the new state
    pub fn transition(self, event: Event) -> State {
        match (&self, &event) {
            // === Idle state transitions ===
            (State::Idle, Event::HotkeyPressed) => State::Recording {
                started_at: Instant::now(),
            },
            (State::Idle, _) => State::Idle, // Ignore other events in idle

            // === Recording state transitions ===
            (State::Recording { .. }, Event::HotkeyReleased) => {
                // Wait for RecordingComplete event
                State::Recording {
                    started_at: Instant::now(),
                }
            }
            (State::Recording { .. }, Event::RecordingComplete(audio)) => {
                if audio.is_empty() {
                    State::Error {
                        message: "No audio captured".into(),
                    }
                } else {
                    State::Transcribing { audio: audio.clone() }
                }
            }
            (State::Recording { .. }, Event::Timeout) => State::Error {
                message: "Recording timeout exceeded".into(),
            },
            (State::Recording { .. }, Event::Error(msg)) => State::Error { message: msg.clone() },
            (State::Recording { started_at }, _) => State::Recording { started_at: *started_at },

            // === Transcribing state transitions ===
            (State::Transcribing { .. }, Event::TranscriptionComplete(text)) => {
                if text.trim().is_empty() {
                    // Empty transcription, go back to idle without error
                    tracing::debug!("Transcription was empty");
                    State::Idle
                } else {
                    State::Outputting { text: text.clone() }
                }
            }
            (State::Transcribing { .. }, Event::Error(msg)) => State::Error { message: msg.clone() },
            (State::Transcribing { audio }, _) => State::Transcribing { audio: audio.clone() },

            // === Outputting state transitions ===
            (State::Outputting { .. }, Event::OutputComplete) => State::Idle,
            (State::Outputting { .. }, Event::Error(msg)) => State::Error { message: msg.clone() },
            (State::Outputting { text }, _) => State::Outputting { text: text.clone() },

            // === Error state transitions ===
            (State::Error { .. }, Event::Reset) => State::Idle,
            (State::Error { .. }, Event::HotkeyPressed) => State::Idle, // Allow restart
            (State::Error { message }, _) => State::Error { message: message.clone() },
        }
    }

    /// Check if in idle state
    pub fn is_idle(&self) -> bool {
        matches!(self, State::Idle)
    }

    /// Check if in recording state
    pub fn is_recording(&self) -> bool {
        matches!(self, State::Recording { .. })
    }

    /// Check if in transcribing state
    pub fn is_transcribing(&self) -> bool {
        matches!(self, State::Transcribing { .. })
    }

    /// Check if in outputting state
    pub fn is_outputting(&self) -> bool {
        matches!(self, State::Outputting { .. })
    }

    /// Check if in error state
    pub fn is_error(&self) -> bool {
        matches!(self, State::Error { .. })
    }

    /// Get recording duration if currently recording
    pub fn recording_duration(&self) -> Option<std::time::Duration> {
        match self {
            State::Recording { started_at } => Some(started_at.elapsed()),
            _ => None,
        }
    }

    /// Get error message if in error state
    pub fn error_message(&self) -> Option<&str> {
        match self {
            State::Error { message } => Some(message),
            _ => None,
        }
    }
}

impl Default for State {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for State {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            State::Idle => write!(f, "Idle"),
            State::Recording { started_at } => {
                write!(f, "Recording ({:.1}s)", started_at.elapsed().as_secs_f32())
            }
            State::Transcribing { audio } => {
                let duration = audio.len() as f32 / 16000.0;
                write!(f, "Transcribing ({:.1}s of audio)", duration)
            }
            State::Outputting { text } => {
                let preview = if text.len() > 20 {
                    format!("{}...", &text[..20])
                } else {
                    text.clone()
                };
                write!(f, "Outputting: {:?}", preview)
            }
            State::Error { message } => write!(f, "Error: {}", message),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_idle_to_recording() {
        let state = State::Idle;
        let state = state.transition(Event::HotkeyPressed);
        assert!(state.is_recording());
    }

    #[test]
    fn test_recording_to_transcribing() {
        let state = State::Recording {
            started_at: Instant::now(),
        };
        let audio = vec![0.0f32; 16000]; // 1 second of silence
        let state = state.transition(Event::RecordingComplete(audio));
        assert!(state.is_transcribing());
    }

    #[test]
    fn test_empty_recording_to_error() {
        let state = State::Recording {
            started_at: Instant::now(),
        };
        let state = state.transition(Event::RecordingComplete(vec![]));
        assert!(state.is_error());
    }

    #[test]
    fn test_transcribing_to_outputting() {
        let state = State::Transcribing {
            audio: vec![0.0; 100],
        };
        let state = state.transition(Event::TranscriptionComplete("hello world".to_string()));
        assert!(state.is_outputting());
    }

    #[test]
    fn test_empty_transcription_to_idle() {
        let state = State::Transcribing {
            audio: vec![0.0; 100],
        };
        let state = state.transition(Event::TranscriptionComplete("   ".to_string()));
        assert!(state.is_idle());
    }

    #[test]
    fn test_outputting_to_idle() {
        let state = State::Outputting {
            text: "hello".to_string(),
        };
        let state = state.transition(Event::OutputComplete);
        assert!(state.is_idle());
    }

    #[test]
    fn test_error_reset() {
        let state = State::Error {
            message: "test error".to_string(),
        };
        let state = state.transition(Event::Reset);
        assert!(state.is_idle());
    }

    #[test]
    fn test_state_display() {
        let state = State::Idle;
        assert_eq!(format!("{}", state), "Idle");

        let state = State::Error {
            message: "test".to_string(),
        };
        assert!(format!("{}", state).contains("test"));
    }
}
