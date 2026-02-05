//! Audio system for audio-reactive animations
//!
//! This module provides audio file decoding, analysis, playback, and live capture.
//! It integrates with the signal system to provide audio-derived signals for animation.
//!
//! ## Feature Flag
//!
//! This module requires the `audio` feature to be enabled:
//! ```toml
//! fractal_flame_wgpu = { features = ["audio"] }
//! ```
//!
//! ## Architecture
//!
//! - `AudioManager` - Central coordinator implementing `SignalProducer`
//! - `decode` - Audio file decoding via symphonia (MP3, WAV, FLAC, OGG)
//! - `analyzer` - STFT analysis, mel spectrogram, onset detection
//! - `playback` - Audio playback synced to animation timeline
//! - `capture` - Live audio input with real-time analysis
//!
//! ## Available Signals
//!
//! After calling `AudioManager::analyze()`, the following signals are available:
//! - `amplitude` - Overall RMS amplitude (0-1)
//! - `energy_low` - Low frequency (bass) energy (0-1)
//! - `energy_mid` - Mid frequency energy (0-1)
//! - `energy_high` - High frequency (treble) energy (0-1)
//! - `spectral_centroid` - Brightness indicator (0-1)
//! - `spectral_flux` - Rate of spectral change (0-1)
//! - `onset` - Transient/beat detection (0 or 1)

mod analyzer;
mod decode;

// Platform-specific audio playback implementations
#[cfg(not(target_arch = "wasm32"))]
mod playback;
#[cfg(target_arch = "wasm32")]
mod playback_wasm;

// Platform-specific audio capture implementations
#[cfg(not(target_arch = "wasm32"))]
mod capture_native;
#[cfg(target_arch = "wasm32")]
mod capture_wasm;

pub use analyzer::{AnalysisConfig, AudioAnalyzer};
pub use decode::{AudioData, AudioDecoder, DecodeError};

// Re-export platform-specific playback types
#[cfg(not(target_arch = "wasm32"))]
pub use playback::{AudioPlayer, PlaybackError, PlaybackState};
#[cfg(target_arch = "wasm32")]
pub use playback_wasm::{AudioPlayer, PlaybackError, PlaybackState};

// Re-export platform-specific capture types
#[cfg(not(target_arch = "wasm32"))]
pub use capture_native::{AtomicSignals, AudioCapture, CaptureConfig, CaptureError, CaptureState};
#[cfg(target_arch = "wasm32")]
pub use capture_wasm::{AtomicSignals, AudioCapture, CaptureConfig, CaptureError, CaptureState};

use crate::signal::{Signal, SignalProducer};
use std::collections::HashMap;
use std::path::Path;

/// Central coordinator for audio analysis functionality.
///
/// Implements `SignalProducer` to provide audio-derived signals to the animation system.
/// Signals are source-agnostic - the animation system doesn't know they come from audio.
///
/// Note: Audio playback is handled separately by `AudioPlayer` due to threading constraints.
/// The cpal `Stream` type is not `Send`-safe, so it cannot be embedded in `SignalProducer`.
pub struct AudioManager {
    /// Decoded audio data (if file loaded)
    audio_data: Option<AudioData>,

    /// Pre-computed signals from offline analysis
    signals: HashMap<String, Signal>,

    /// Whether analysis is in progress
    analyzing: bool,

    /// Analysis progress (0.0 to 1.0)
    analysis_progress: f32,
}

impl AudioManager {
    /// Create a new AudioManager with no audio loaded.
    pub fn new() -> Self {
        Self {
            audio_data: None,
            signals: HashMap::new(),
            analyzing: false,
            analysis_progress: 0.0,
        }
    }

    /// Load an audio file and decode it.
    ///
    /// Supports MP3, WAV, FLAC, and OGG formats via symphonia.
    /// After loading, call `analyze()` to compute signals.
    pub fn load_file(&mut self, path: &Path) -> Result<(), DecodeError> {
        let data = AudioDecoder::decode_file(path)?;
        self.audio_data = Some(data);
        self.signals.clear();
        self.analyzing = false;
        self.analysis_progress = 0.0;
        Ok(())
    }

    /// Load audio from raw bytes (for WASM file picker).
    ///
    /// The format is auto-detected from the data.
    pub fn load_bytes(&mut self, data: &[u8]) -> Result<(), DecodeError> {
        let audio = AudioDecoder::decode_bytes(data)?;
        self.audio_data = Some(audio);
        self.signals.clear();
        self.analyzing = false;
        self.analysis_progress = 0.0;
        Ok(())
    }

    /// Check if audio is loaded.
    pub fn has_audio(&self) -> bool {
        self.audio_data.is_some()
    }

    /// Get the loaded audio data (if any).
    pub fn audio_data(&self) -> Option<&AudioData> {
        self.audio_data.as_ref()
    }

    /// Get the duration of loaded audio in seconds.
    pub fn duration(&self) -> Option<f64> {
        self.audio_data.as_ref().map(|d| d.duration())
    }

    /// Get the sample rate of loaded audio.
    pub fn sample_rate(&self) -> Option<u32> {
        self.audio_data.as_ref().map(|d| d.sample_rate)
    }

    /// Get the number of channels.
    pub fn channels(&self) -> Option<u16> {
        self.audio_data.as_ref().map(|d| d.channels)
    }

    /// Check if analysis is in progress.
    pub fn is_analyzing(&self) -> bool {
        self.analyzing
    }

    /// Get analysis progress (0.0 to 1.0).
    pub fn analysis_progress(&self) -> f32 {
        self.analysis_progress
    }

    /// Run offline analysis to compute signals.
    ///
    /// This computes all available audio signals using FFT-based analysis:
    /// - `amplitude` - Overall RMS amplitude
    /// - `energy_low` - Low frequency (bass) energy
    /// - `energy_mid` - Mid frequency energy
    /// - `energy_high` - High frequency (treble) energy
    /// - `spectral_centroid` - Brightness indicator
    /// - `spectral_flux` - Rate of spectral change
    /// - `onset` - Transient/beat detection
    ///
    /// Call `is_analyzing()` and `analysis_progress()` to monitor progress.
    pub fn analyze(&mut self) {
        self.analyze_with_config(AnalysisConfig::default());
    }

    /// Run offline analysis with custom configuration.
    pub fn analyze_with_config(&mut self, config: AnalysisConfig) {
        let Some(ref audio) = self.audio_data else {
            return;
        };

        self.analyzing = true;
        self.analysis_progress = 0.0;

        // Convert to mono for analysis
        let samples = audio.to_mono();

        // Create analyzer and run analysis
        let analyzer = AudioAnalyzer::new(audio.sample_rate, config);

        // TODO: For background thread analysis, we'd need async/channels
        // For now, analysis is synchronous
        self.analysis_progress = 0.5; // Indicate we're processing

        let signals = analyzer.analyze(&samples);

        // Store all computed signals
        self.signals = signals;

        self.analyzing = false;
        self.analysis_progress = 1.0;
    }

    /// Clear all loaded audio and signals.
    pub fn clear(&mut self) {
        self.audio_data = None;
        self.signals.clear();
        self.analyzing = false;
        self.analysis_progress = 0.0;
    }

    /// Get a computed signal by name.
    pub fn get_signal(&self, name: &str) -> Option<&Signal> {
        self.signals.get(name)
    }

    /// List all available signal names.
    pub fn available_signals(&self) -> Vec<&str> {
        self.signals.keys().map(|s| s.as_str()).collect()
    }
}

impl Default for AudioManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SignalProducer for AudioManager {
    fn signal_names(&self) -> Vec<String> {
        self.signals.keys().cloned().collect()
    }

    fn get_live_value(&self, _name: &str) -> Option<f32> {
        // AudioManager doesn't support live values from file analysis
        // Live capture will be added in Phase 5
        None
    }

    fn get_signal(&self, name: &str) -> Option<Signal> {
        self.signals.get(name).cloned()
    }

    fn is_active(&self) -> bool {
        self.has_audio() && !self.signals.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audio_manager_creation() {
        let manager = AudioManager::new();
        assert!(!manager.has_audio());
        assert!(!manager.is_analyzing());
        assert_eq!(manager.analysis_progress(), 0.0);
    }

    #[test]
    fn test_audio_manager_signal_producer_trait() {
        let manager = AudioManager::new();
        assert!(manager.signal_names().is_empty());
        assert!(!manager.is_active());
        assert!(manager.get_signal("amplitude").is_none());
    }
}
