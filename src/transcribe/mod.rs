//! Speech-to-text transcription module
//!
//! Provides local transcription using whisper.cpp via the whisper-rs crate.

pub mod whisper;

use crate::config::WhisperConfig;
use crate::error::TranscribeError;

/// Trait for speech-to-text implementations
pub trait Transcriber: Send + Sync {
    /// Transcribe audio samples to text
    /// Input: f32 samples, mono, 16kHz
    fn transcribe(&self, samples: &[f32]) -> Result<String, TranscribeError>;
}

/// Factory function to create transcriber
pub fn create_transcriber(
    config: &WhisperConfig,
) -> Result<Box<dyn Transcriber>, TranscribeError> {
    Ok(Box::new(whisper::WhisperTranscriber::new(config)?))
}
