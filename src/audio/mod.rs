//! Audio capture module
//!
//! Provides audio recording capabilities using cpal, which works with
//! PipeWire, PulseAudio, and ALSA backends.

pub mod cpal_capture;
pub mod feedback;

use crate::config::AudioConfig;
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

    /// Get the current audio level (RMS, 0.0–1.0).
    /// Updated in real-time by the audio capture callback.
    /// Returns 0.0 when not recording.
    fn current_level(&self) -> f32;
}

/// Factory function to create audio capture
pub fn create_capture(config: &AudioConfig) -> Result<Box<dyn AudioCapture>, AudioError> {
    Ok(Box::new(cpal_capture::CpalCapture::new(config)?))
}
