//! Shared types and logic for audio capture (native + WASM)
//!
//! Contains:
//! - CaptureState enum (with WASM-only variants behind cfg)
//! - AtomicSignals for lock-free audio thread → main thread transfer
//! - CaptureConfig for capture settings
//! - CaptureError enum
//! - RealtimeAnalyzer for FFT-based real-time analysis
//! - LiveSignalBridge implementing SignalProducer
//! - Macro for generating identical signal accessor impls

use rustfft::{num_complex::Complex, FftPlanner};
use std::f32::consts::PI;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use crate::signal::{Signal, SignalProducer};

// ─── CaptureState ────────────────────────────────────────────────────────────

/// Live audio capture state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureState {
    /// Not capturing
    Stopped,
    /// Waiting for user permission (WASM only — browsers require async permission)
    #[cfg(target_arch = "wasm32")]
    RequestingPermission,
    /// Actively capturing audio
    Capturing,
    /// Permission denied or error occurred (WASM only)
    #[cfg(target_arch = "wasm32")]
    Error,
}

// ─── AtomicSignals ───────────────────────────────────────────────────────────

/// Atomic signal values for lock-free audio thread → main thread transfer
///
/// All values are stored as u32 bit patterns of f32 values.
/// This allows atomic updates without locks.
#[derive(Default)]
pub struct AtomicSignals {
    pub amplitude: AtomicU32,
    pub energy_low: AtomicU32,
    pub energy_mid: AtomicU32,
    pub energy_high: AtomicU32,
    pub spectral_centroid: AtomicU32,
    pub spectral_flux: AtomicU32,
    pub onset: AtomicU32,
}

impl AtomicSignals {
    pub fn new() -> Self {
        Self::default()
    }

    fn store_f32(atomic: &AtomicU32, value: f32) {
        atomic.store(value.to_bits(), Ordering::Release);
    }

    fn load_f32(atomic: &AtomicU32) -> f32 {
        f32::from_bits(atomic.load(Ordering::Acquire))
    }

    pub fn set_amplitude(&self, value: f32) {
        Self::store_f32(&self.amplitude, value);
    }

    pub fn get_amplitude(&self) -> f32 {
        Self::load_f32(&self.amplitude)
    }

    pub fn set_energy_low(&self, value: f32) {
        Self::store_f32(&self.energy_low, value);
    }

    pub fn get_energy_low(&self) -> f32 {
        Self::load_f32(&self.energy_low)
    }

    pub fn set_energy_mid(&self, value: f32) {
        Self::store_f32(&self.energy_mid, value);
    }

    pub fn get_energy_mid(&self) -> f32 {
        Self::load_f32(&self.energy_mid)
    }

    pub fn set_energy_high(&self, value: f32) {
        Self::store_f32(&self.energy_high, value);
    }

    pub fn get_energy_high(&self) -> f32 {
        Self::load_f32(&self.energy_high)
    }

    pub fn set_spectral_centroid(&self, value: f32) {
        Self::store_f32(&self.spectral_centroid, value);
    }

    pub fn get_spectral_centroid(&self) -> f32 {
        Self::load_f32(&self.spectral_centroid)
    }

    pub fn set_spectral_flux(&self, value: f32) {
        Self::store_f32(&self.spectral_flux, value);
    }

    pub fn get_spectral_flux(&self) -> f32 {
        Self::load_f32(&self.spectral_flux)
    }

    pub fn set_onset(&self, value: f32) {
        Self::store_f32(&self.onset, value);
    }

    pub fn get_onset(&self) -> f32 {
        Self::load_f32(&self.onset)
    }
}

// ─── CaptureConfig ───────────────────────────────────────────────────────────

/// Configuration for live capture
#[derive(Clone, Debug)]
pub struct CaptureConfig {
    /// FFT size for analysis (power of 2)
    pub fft_size: usize,
    /// Onset detection threshold (0.0-1.0)
    pub onset_threshold: f32,
}

impl Default for CaptureConfig {
    fn default() -> Self {
        Self {
            fft_size: 1024, // ~23ms at 44.1kHz
            onset_threshold: 0.3,
        }
    }
}

// ─── CaptureError ────────────────────────────────────────────────────────────

/// Errors that can occur during capture.
#[derive(Debug)]
pub enum CaptureError {
    /// No audio input device available
    NoInputDevice,
    /// Device not found by name
    DeviceNotFound(String),
    /// Error enumerating devices
    DeviceError(String),
    /// Error configuring audio device
    ConfigError(String),
    /// Error with audio stream
    StreamError(String),
    /// Unsupported sample format
    UnsupportedFormat,
    /// Permission denied by user (WASM — never constructed on desktop, but kept
    /// so consumer code can match exhaustively on both platforms)
    PermissionDenied,
}

impl std::fmt::Display for CaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CaptureError::NoInputDevice => write!(f, "No audio input device available"),
            CaptureError::DeviceNotFound(s) => write!(f, "Audio device not found: {}", s),
            CaptureError::DeviceError(s) => write!(f, "Device error: {}", s),
            CaptureError::ConfigError(s) => write!(f, "Config error: {}", s),
            CaptureError::StreamError(s) => write!(f, "Stream error: {}", s),
            CaptureError::UnsupportedFormat => write!(f, "Unsupported audio format"),
            CaptureError::PermissionDenied => write!(f, "Microphone permission denied"),
        }
    }
}

impl std::error::Error for CaptureError {}

// ─── RealtimeAnalyzer ────────────────────────────────────────────────────────

/// Real-time audio analyzer running on audio thread
///
/// Performs windowed FFT analysis and extracts energy bands, spectral features,
/// and onset detection from incoming audio samples.
pub(crate) struct RealtimeAnalyzer {
    fft_size: usize,
    sample_rate: f32,
    window: Vec<f32>,
    buffer: Vec<f32>,
    buffer_pos: usize,
    fft_scratch: Vec<Complex<f32>>,
    prev_magnitudes: Vec<f32>,
    onset_threshold: f32,
    // Normalization state (exponential moving average of max values)
    max_amplitude: f32,
    max_energy_low: f32,
    max_energy_mid: f32,
    max_energy_high: f32,
    max_flux: f32,
}

impl RealtimeAnalyzer {
    pub fn new(fft_size: usize, sample_rate: f32, onset_threshold: f32) -> Self {
        let window: Vec<f32> = (0..fft_size)
            .map(|i| 0.5 * (1.0 - (2.0 * PI * i as f32 / (fft_size - 1) as f32).cos()))
            .collect();

        Self {
            fft_size,
            sample_rate,
            window,
            buffer: vec![0.0; fft_size],
            buffer_pos: 0,
            fft_scratch: vec![Complex::new(0.0, 0.0); fft_size],
            prev_magnitudes: vec![0.0; fft_size / 2 + 1],
            onset_threshold,
            max_amplitude: 0.1,
            max_energy_low: 0.1,
            max_energy_mid: 0.1,
            max_energy_high: 0.1,
            max_flux: 0.1,
        }
    }

    /// Process incoming samples and update atomic signals when buffer is full
    pub fn process_samples(&mut self, samples: &[f32], signals: &Arc<AtomicSignals>) {
        for &sample in samples {
            self.buffer[self.buffer_pos] = sample;
            self.buffer_pos += 1;

            if self.buffer_pos >= self.fft_size {
                self.analyze_buffer(signals);
                // Overlap by 50% for smoother updates
                let half = self.fft_size / 2;
                self.buffer.copy_within(half.., 0);
                self.buffer_pos = half;
            }
        }
    }

    fn analyze_buffer(&mut self, signals: &Arc<AtomicSignals>) {
        // Compute RMS amplitude
        let rms: f32 =
            (self.buffer.iter().map(|s| s * s).sum::<f32>() / self.fft_size as f32).sqrt();
        self.max_amplitude = self.max_amplitude.max(rms) * 0.999 + rms * 0.001;
        let amplitude = (rms / self.max_amplitude).clamp(0.0, 1.0);
        signals.set_amplitude(amplitude);

        // Apply window and compute FFT
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(self.fft_size);

        for i in 0..self.fft_size {
            self.fft_scratch[i] = Complex::new(self.buffer[i] * self.window[i], 0.0);
        }
        fft.process(&mut self.fft_scratch);

        // Compute magnitude spectrum
        let num_bins = self.fft_size / 2 + 1;
        let magnitudes: Vec<f32> = self.fft_scratch[..num_bins]
            .iter()
            .map(|c| c.norm())
            .collect();

        // Compute energy bands using perceptual frequency boundaries
        // Low: 20-250 Hz (kick, bass), Mid: 250-4000 Hz (vocals, snare), High: 4000+ Hz (cymbals, air)
        let freq_per_bin = self.sample_rate / self.fft_size as f32;
        let low_end = ((250.0 / freq_per_bin) as usize).clamp(1, num_bins - 2);
        let mid_end = ((4000.0 / freq_per_bin) as usize).clamp(low_end + 1, num_bins - 1);

        let low_energy: f32 = magnitudes[..low_end]
            .iter()
            .map(|m| m * m)
            .sum::<f32>()
            .sqrt();
        let mid_energy: f32 = magnitudes[low_end..mid_end]
            .iter()
            .map(|m| m * m)
            .sum::<f32>()
            .sqrt();
        let high_energy: f32 = magnitudes[mid_end..].iter().map(|m| m * m).sum::<f32>().sqrt();

        // Per-band independent normalization via EMA max tracking
        self.max_energy_low = self.max_energy_low.max(low_energy) * 0.999 + low_energy * 0.001;
        self.max_energy_mid = self.max_energy_mid.max(mid_energy) * 0.999 + mid_energy * 0.001;
        self.max_energy_high =
            self.max_energy_high.max(high_energy) * 0.999 + high_energy * 0.001;

        signals.set_energy_low((low_energy / self.max_energy_low).clamp(0.0, 1.0));
        signals.set_energy_mid((mid_energy / self.max_energy_mid).clamp(0.0, 1.0));
        signals.set_energy_high((high_energy / self.max_energy_high).clamp(0.0, 1.0));

        // Spectral centroid
        let freq_resolution = self.sample_rate / self.fft_size as f32;
        let total_magnitude: f32 = magnitudes.iter().sum();
        let centroid = if total_magnitude > 1e-10 {
            let weighted: f32 = magnitudes
                .iter()
                .enumerate()
                .map(|(i, m)| m * i as f32 * freq_resolution)
                .sum();
            weighted / total_magnitude / (self.sample_rate / 2.0)
        } else {
            0.0
        };
        signals.set_spectral_centroid(centroid.clamp(0.0, 1.0));

        // Spectral flux (half-wave rectified)
        let flux: f32 = magnitudes
            .iter()
            .zip(self.prev_magnitudes.iter())
            .map(|(curr, prev)| (curr - prev).max(0.0))
            .sum();

        self.max_flux = self.max_flux.max(flux) * 0.995 + flux * 0.005;
        let normalized_flux = (flux / self.max_flux).clamp(0.0, 1.0);
        signals.set_spectral_flux(normalized_flux);

        // Onset detection
        let onset = if normalized_flux > self.onset_threshold {
            1.0
        } else {
            0.0
        };
        signals.set_onset(onset);

        // Store magnitudes for next frame
        self.prev_magnitudes.copy_from_slice(&magnitudes);
    }
}

// ─── LiveSignalBridge ────────────────────────────────────────────────────────

/// Lightweight bridge for sharing live capture signals with SignalManager.
///
/// Shares the same atomic signals as AudioCapture via Arc, allowing
/// SignalManager to read live values without owning the capture stream.
pub(crate) struct LiveSignalBridge {
    pub signals: Arc<AtomicSignals>,
    pub active: Arc<AtomicBool>,
}

// SAFETY: On WASM, everything is single-threaded. The Arc<AtomicSignals> and
// Arc<AtomicBool> are inherently Send+Sync via their atomic internals.
// On desktop this impl is redundant but harmless.
#[cfg(target_arch = "wasm32")]
unsafe impl Send for LiveSignalBridge {}

impl SignalProducer for LiveSignalBridge {
    fn signal_names(&self) -> Vec<String> {
        live_signal_names()
    }

    fn get_live_value(&self, name: &str) -> Option<f32> {
        if !self.active.load(Ordering::Relaxed) {
            return None;
        }
        get_live_signal_value(&self.signals, name)
    }

    fn get_signal(&self, _name: &str) -> Option<Signal> {
        None // Live-only, no buffered signals
    }

    fn is_active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }
}

// ─── Helper functions ────────────────────────────────────────────────────────

/// Canonical list of live signal names (prefixed with "live_").
pub(crate) fn live_signal_names() -> Vec<String> {
    vec![
        "live_amplitude".to_string(),
        "live_energy_low".to_string(),
        "live_energy_mid".to_string(),
        "live_energy_high".to_string(),
        "live_spectral_centroid".to_string(),
        "live_spectral_flux".to_string(),
        "live_onset".to_string(),
    ]
}

/// Look up a live signal value by name from atomic signals.
pub(crate) fn get_live_signal_value(signals: &AtomicSignals, name: &str) -> Option<f32> {
    match name {
        "live_amplitude" => Some(signals.get_amplitude()),
        "live_energy_low" => Some(signals.get_energy_low()),
        "live_energy_mid" => Some(signals.get_energy_mid()),
        "live_energy_high" => Some(signals.get_energy_high()),
        "live_spectral_centroid" => Some(signals.get_spectral_centroid()),
        "live_spectral_flux" => Some(signals.get_spectral_flux()),
        "live_onset" => Some(signals.get_onset()),
        _ => None,
    }
}

// ─── Macro ───────────────────────────────────────────────────────────────────

/// Generates the identical signal accessor methods for `AudioCapture`.
///
/// Both native and WASM `AudioCapture` structs must have:
/// - `signals: Arc<AtomicSignals>`
/// - `active: Arc<AtomicBool>`
///
/// This macro generates:
/// - Individual signal accessors (amplitude, energy_low, …)
/// - `signal_names()` → Vec<String>
/// - `get_live_value(name)` → Option<f32>
/// - `create_producer()` → Box<dyn SignalProducer>
macro_rules! impl_capture_signal_accessors {
    () => {
        /// Get current amplitude (0-1).
        pub fn amplitude(&self) -> f32 {
            self.signals.get_amplitude()
        }

        /// Get current low frequency energy (0-1).
        pub fn energy_low(&self) -> f32 {
            self.signals.get_energy_low()
        }

        /// Get current mid frequency energy (0-1).
        pub fn energy_mid(&self) -> f32 {
            self.signals.get_energy_mid()
        }

        /// Get current high frequency energy (0-1).
        pub fn energy_high(&self) -> f32 {
            self.signals.get_energy_high()
        }

        /// Get current spectral centroid (0-1).
        pub fn spectral_centroid(&self) -> f32 {
            self.signals.get_spectral_centroid()
        }

        /// Get current spectral flux (0-1).
        pub fn spectral_flux(&self) -> f32 {
            self.signals.get_spectral_flux()
        }

        /// Get current onset trigger (0 or 1).
        pub fn onset(&self) -> f32 {
            self.signals.get_onset()
        }

        /// Get list of available live signal names.
        pub fn signal_names(&self) -> Vec<String> {
            $crate::audio::capture_common::live_signal_names()
        }

        /// Get current live value for a signal by name.
        ///
        /// Returns None if not capturing or signal name is unknown.
        pub fn get_live_value(&self, name: &str) -> Option<f32> {
            if !self.is_capturing() {
                return None;
            }
            $crate::audio::capture_common::get_live_signal_value(&self.signals, name)
        }

        /// Create a signal producer bridge that shares atomic signals with this capture.
        ///
        /// The bridge can be registered with SignalManager so live capture signals
        /// appear in the animation track signal dropdown.
        pub fn create_producer(&self) -> Box<dyn $crate::signal::SignalProducer> {
            Box::new($crate::audio::capture_common::LiveSignalBridge {
                signals: self.signals.clone(),
                active: self.active.clone(),
            })
        }
    };
}

pub(crate) use impl_capture_signal_accessors;

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atomic_signals() {
        let signals = AtomicSignals::new();

        signals.set_amplitude(0.75);
        assert!((signals.get_amplitude() - 0.75).abs() < 0.001);

        signals.set_energy_low(0.5);
        assert!((signals.get_energy_low() - 0.5).abs() < 0.001);

        signals.set_onset(1.0);
        assert!((signals.get_onset() - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_live_signal_names() {
        let names = live_signal_names();
        assert_eq!(names.len(), 7);
        assert!(names.contains(&"live_amplitude".to_string()));
        assert!(names.contains(&"live_energy_low".to_string()));
        assert!(names.contains(&"live_onset".to_string()));
    }

    #[test]
    fn test_get_live_signal_value() {
        let signals = AtomicSignals::new();
        signals.set_amplitude(0.42);
        assert_eq!(get_live_signal_value(&signals, "live_amplitude"), Some(0.42));
        assert_eq!(get_live_signal_value(&signals, "unknown"), None);
    }
}
