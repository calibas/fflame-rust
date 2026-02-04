//! Live audio capture with real-time analysis
//!
//! Provides real-time audio input capture and analysis for live audio-reactive animations.
//! Uses cpal for cross-platform audio input and lock-free atomics for thread-safe signal transfer.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, Stream, StreamConfig};
use rustfft::{num_complex::Complex, FftPlanner};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::f32::consts::PI;

/// Live audio capture state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureState {
    /// Not capturing
    Stopped,
    /// Actively capturing audio
    Capturing,
}

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
            fft_size: 1024, // Smaller for lower latency (~23ms at 44.1kHz)
            onset_threshold: 0.3,
        }
    }
}

/// Real-time audio analyzer running on audio thread
struct RealtimeAnalyzer {
    fft_size: usize,
    sample_rate: u32,
    window: Vec<f32>,
    buffer: Vec<f32>,
    buffer_pos: usize,
    fft_scratch: Vec<Complex<f32>>,
    prev_magnitudes: Vec<f32>,
    onset_threshold: f32,
    // Normalization state (exponential moving average of max values)
    max_amplitude: f32,
    max_energy: f32,
    max_flux: f32,
}

impl RealtimeAnalyzer {
    fn new(fft_size: usize, sample_rate: u32, onset_threshold: f32) -> Self {
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
            max_energy: 0.1,
            max_flux: 0.1,
        }
    }

    /// Process incoming samples and update atomic signals when buffer is full
    fn process_samples(&mut self, samples: &[f32], signals: &Arc<AtomicSignals>) {
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
        let rms: f32 = (self.buffer.iter().map(|s| s * s).sum::<f32>() / self.fft_size as f32).sqrt();
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

        // Compute energy bands
        let low_end = num_bins / 6;      // ~0-730 Hz at 44.1kHz
        let mid_end = num_bins / 2;       // ~730-11kHz
        // high is the rest                // ~11kHz-22kHz

        let low_energy: f32 = magnitudes[..low_end].iter().map(|m| m * m).sum::<f32>().sqrt();
        let mid_energy: f32 = magnitudes[low_end..mid_end].iter().map(|m| m * m).sum::<f32>().sqrt();
        let high_energy: f32 = magnitudes[mid_end..].iter().map(|m| m * m).sum::<f32>().sqrt();

        let total_energy = low_energy + mid_energy + high_energy + 0.0001;
        self.max_energy = self.max_energy.max(total_energy) * 0.999 + total_energy * 0.001;

        signals.set_energy_low((low_energy / self.max_energy * 3.0).clamp(0.0, 1.0));
        signals.set_energy_mid((mid_energy / self.max_energy * 3.0).clamp(0.0, 1.0));
        signals.set_energy_high((high_energy / self.max_energy * 3.0).clamp(0.0, 1.0));

        // Spectral centroid
        let freq_resolution = self.sample_rate as f32 / self.fft_size as f32;
        let total_magnitude: f32 = magnitudes.iter().sum();
        let centroid = if total_magnitude > 1e-10 {
            let weighted: f32 = magnitudes
                .iter()
                .enumerate()
                .map(|(i, m)| m * i as f32 * freq_resolution)
                .sum();
            weighted / total_magnitude / (self.sample_rate as f32 / 2.0)
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
        let onset = if normalized_flux > self.onset_threshold { 1.0 } else { 0.0 };
        signals.set_onset(onset);

        // Store magnitudes for next frame
        self.prev_magnitudes.copy_from_slice(&magnitudes);
    }
}

/// Live audio capture controller
///
/// Captures audio from input device and provides real-time signals.
/// Implements `SignalProducer` for integration with animation system.
pub struct AudioCapture {
    /// Current capture state
    state: CaptureState,

    /// Atomic signals (shared with audio thread)
    signals: Arc<AtomicSignals>,

    /// Whether capture is active
    active: Arc<AtomicBool>,

    /// Audio input stream (kept alive while capturing)
    _stream: Option<Stream>,

    /// Sample rate of capture device
    sample_rate: u32,

    /// Capture configuration
    config: CaptureConfig,
}

impl AudioCapture {
    /// Create a new audio capture with default settings.
    pub fn new() -> Self {
        Self::with_config(CaptureConfig::default())
    }

    /// Create a new audio capture with custom configuration.
    pub fn with_config(config: CaptureConfig) -> Self {
        Self {
            state: CaptureState::Stopped,
            signals: Arc::new(AtomicSignals::new()),
            active: Arc::new(AtomicBool::new(false)),
            _stream: None,
            sample_rate: 44100,
            config,
        }
    }

    /// Get the current capture state.
    pub fn state(&self) -> CaptureState {
        self.state
    }

    /// Check if currently capturing.
    pub fn is_capturing(&self) -> bool {
        self.state == CaptureState::Capturing
    }

    /// List available input devices.
    pub fn list_devices() -> Vec<String> {
        let host = cpal::default_host();
        host.input_devices()
            .map(|devices| {
                devices
                    .filter_map(|d| d.name().ok())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Start capturing from the default input device.
    pub fn start(&mut self) -> Result<(), CaptureError> {
        self.start_device(None)
    }

    /// Start capturing from a specific device by name.
    pub fn start_device(&mut self, device_name: Option<&str>) -> Result<(), CaptureError> {
        if self.state == CaptureState::Capturing {
            return Ok(()); // Already capturing
        }

        let host = cpal::default_host();

        let device = if let Some(name) = device_name {
            host.input_devices()
                .map_err(|e| CaptureError::DeviceError(e.to_string()))?
                .find(|d| d.name().ok().as_deref() == Some(name))
                .ok_or_else(|| CaptureError::DeviceNotFound(name.to_string()))?
        } else {
            host.default_input_device()
                .ok_or(CaptureError::NoInputDevice)?
        };

        let supported_config = device
            .default_input_config()
            .map_err(|e| CaptureError::ConfigError(e.to_string()))?;

        self.sample_rate = supported_config.sample_rate().0;

        let signals = self.signals.clone();
        let active = self.active.clone();
        let fft_size = self.config.fft_size;
        let onset_threshold = self.config.onset_threshold;
        let sample_rate = self.sample_rate;

        active.store(true, Ordering::SeqCst);

        let stream = match supported_config.sample_format() {
            SampleFormat::F32 => self.build_stream_f32(
                &device,
                &supported_config.into(),
                signals,
                active,
                fft_size,
                sample_rate,
                onset_threshold,
            )?,
            SampleFormat::I16 => self.build_stream_i16(
                &device,
                &supported_config.into(),
                signals,
                active,
                fft_size,
                sample_rate,
                onset_threshold,
            )?,
            SampleFormat::U16 => self.build_stream_u16(
                &device,
                &supported_config.into(),
                signals,
                active,
                fft_size,
                sample_rate,
                onset_threshold,
            )?,
            _ => return Err(CaptureError::UnsupportedFormat),
        };

        stream
            .play()
            .map_err(|e| CaptureError::StreamError(e.to_string()))?;

        self._stream = Some(stream);
        self.state = CaptureState::Capturing;
        Ok(())
    }

    /// Stop capturing.
    pub fn stop(&mut self) {
        self.active.store(false, Ordering::SeqCst);
        self._stream = None;
        self.state = CaptureState::Stopped;
    }

    /// Build the input stream for f32 samples.
    fn build_stream_f32(
        &self,
        device: &Device,
        config: &StreamConfig,
        signals: Arc<AtomicSignals>,
        active: Arc<AtomicBool>,
        fft_size: usize,
        sample_rate: u32,
        onset_threshold: f32,
    ) -> Result<Stream, CaptureError> {
        let channels = config.channels as usize;
        let mut analyzer = RealtimeAnalyzer::new(fft_size, sample_rate, onset_threshold);
        let mut mono_buffer = Vec::with_capacity(1024);

        let stream = device
            .build_input_stream(
                config,
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    if !active.load(Ordering::SeqCst) {
                        return;
                    }

                    mono_buffer.clear();
                    for frame in data.chunks(channels) {
                        let sum: f32 = frame.iter().sum();
                        mono_buffer.push(sum / channels as f32);
                    }

                    analyzer.process_samples(&mono_buffer, &signals);
                },
                |err| {
                    log::error!("Audio capture error: {}", err);
                },
                None,
            )
            .map_err(|e| CaptureError::StreamError(e.to_string()))?;

        Ok(stream)
    }

    /// Build the input stream for i16 samples.
    fn build_stream_i16(
        &self,
        device: &Device,
        config: &StreamConfig,
        signals: Arc<AtomicSignals>,
        active: Arc<AtomicBool>,
        fft_size: usize,
        sample_rate: u32,
        onset_threshold: f32,
    ) -> Result<Stream, CaptureError> {
        let channels = config.channels as usize;
        let mut analyzer = RealtimeAnalyzer::new(fft_size, sample_rate, onset_threshold);
        let mut mono_buffer = Vec::with_capacity(1024);

        let stream = device
            .build_input_stream(
                config,
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    if !active.load(Ordering::SeqCst) {
                        return;
                    }

                    mono_buffer.clear();
                    for frame in data.chunks(channels) {
                        let sum: f32 = frame.iter().map(|&s| s as f32 / 32768.0).sum();
                        mono_buffer.push(sum / channels as f32);
                    }

                    analyzer.process_samples(&mono_buffer, &signals);
                },
                |err| {
                    log::error!("Audio capture error: {}", err);
                },
                None,
            )
            .map_err(|e| CaptureError::StreamError(e.to_string()))?;

        Ok(stream)
    }

    /// Build the input stream for u16 samples.
    fn build_stream_u16(
        &self,
        device: &Device,
        config: &StreamConfig,
        signals: Arc<AtomicSignals>,
        active: Arc<AtomicBool>,
        fft_size: usize,
        sample_rate: u32,
        onset_threshold: f32,
    ) -> Result<Stream, CaptureError> {
        let channels = config.channels as usize;
        let mut analyzer = RealtimeAnalyzer::new(fft_size, sample_rate, onset_threshold);
        let mut mono_buffer = Vec::with_capacity(1024);

        let stream = device
            .build_input_stream(
                config,
                move |data: &[u16], _: &cpal::InputCallbackInfo| {
                    if !active.load(Ordering::SeqCst) {
                        return;
                    }

                    mono_buffer.clear();
                    for frame in data.chunks(channels) {
                        let sum: f32 = frame.iter().map(|&s| (s as f32 - 32768.0) / 32768.0).sum();
                        mono_buffer.push(sum / channels as f32);
                    }

                    analyzer.process_samples(&mono_buffer, &signals);
                },
                |err| {
                    log::error!("Audio capture error: {}", err);
                },
                None,
            )
            .map_err(|e| CaptureError::StreamError(e.to_string()))?;

        Ok(stream)
    }

    // === Signal accessors for direct reading ===

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
}

impl Default for AudioCapture {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioCapture {
    /// Get list of available live signal names.
    ///
    /// These signals are prefixed with "live_" to distinguish from offline analysis signals.
    pub fn signal_names(&self) -> Vec<String> {
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

    /// Get current live value for a signal by name.
    ///
    /// Returns None if not capturing or signal name is unknown.
    pub fn get_live_value(&self, name: &str) -> Option<f32> {
        if !self.is_capturing() {
            return None;
        }

        match name {
            "live_amplitude" => Some(self.amplitude()),
            "live_energy_low" => Some(self.energy_low()),
            "live_energy_mid" => Some(self.energy_mid()),
            "live_energy_high" => Some(self.energy_high()),
            "live_spectral_centroid" => Some(self.spectral_centroid()),
            "live_spectral_flux" => Some(self.spectral_flux()),
            "live_onset" => Some(self.onset()),
            _ => None,
        }
    }
}

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
        }
    }
}

impl std::error::Error for CaptureError {}

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
    fn test_capture_creation() {
        let capture = AudioCapture::new();
        assert_eq!(capture.state(), CaptureState::Stopped);
        assert!(!capture.is_capturing());
    }

    #[test]
    fn test_capture_live_signals() {
        let capture = AudioCapture::new();
        let names = capture.signal_names();

        assert!(names.contains(&"live_amplitude".to_string()));
        assert!(names.contains(&"live_energy_low".to_string()));
        assert!(names.contains(&"live_onset".to_string()));

        // Not capturing, so live values should be None
        assert!(capture.get_live_value("live_amplitude").is_none());
    }

    #[test]
    fn test_list_devices() {
        // Just verify it doesn't crash - devices may not be available in CI
        let _devices = AudioCapture::list_devices();
    }
}
