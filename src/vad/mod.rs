//! Voice Activity Detection module
//!
//! Provides VAD to filter silence-only recordings before transcription,
//! preventing Whisper hallucinations when processing silence.
//!
//! Uses an energy-based approach that analyzes audio amplitude to detect
//! speech presence. This works well for filtering out completely silent
//! or near-silent recordings without requiring external model downloads.

mod energy;

use crate::config::Config;
use crate::error::VadError;

pub use energy::EnergyVad;

/// Result of voice activity detection
#[derive(Debug, Clone)]
pub struct VadResult {
    /// Whether speech was detected in the audio
    pub has_speech: bool,
    /// Estimated duration of speech in seconds
    pub speech_duration_secs: f32,
    /// Ratio of speech to total audio duration (0.0 - 1.0)
    pub speech_ratio: f32,
    /// RMS energy level of the audio (for debugging)
    pub rms_energy: f32,
}

/// Trait for voice activity detection implementations
pub trait VoiceActivityDetector: Send + Sync {
    /// Detect speech in audio samples
    ///
    /// # Arguments
    /// * `samples` - Audio samples at 16kHz mono (f32, normalized to -1.0 to 1.0)
    ///
    /// # Returns
    /// * `Ok(VadResult)` - Detection result with speech metrics
    /// * `Err(VadError)` - If detection fails
    fn detect(&self, samples: &[f32]) -> Result<VadResult, VadError>;
}

/// Create a VAD instance based on configuration
///
/// Returns None if VAD is disabled
pub fn create_vad(config: &Config) -> Option<Box<dyn VoiceActivityDetector>> {
    if !config.vad.enabled {
        return None;
    }

    Some(Box::new(EnergyVad::new(&config.vad)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vad_result_defaults() {
        let result = VadResult {
            has_speech: false,
            speech_duration_secs: 0.0,
            speech_ratio: 0.0,
            rms_energy: 0.0,
        };
        assert!(!result.has_speech);
        assert_eq!(result.speech_duration_secs, 0.0);
        assert_eq!(result.speech_ratio, 0.0);
    }

    #[test]
    fn test_create_vad_disabled() {
        let config = Config::default();
        // VAD is disabled by default
        assert!(!config.vad.enabled);
        let vad = create_vad(&config);
        assert!(vad.is_none());
    }

    #[test]
    fn test_create_vad_enabled() {
        let mut config = Config::default();
        config.vad.enabled = true;
        let vad = create_vad(&config);
        assert!(vad.is_some());
    }
}
