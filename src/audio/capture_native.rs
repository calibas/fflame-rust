//! Desktop live audio capture using cpal
//!
//! Platform-specific capture implementation. All shared types (AtomicSignals,
//! CaptureConfig, CaptureError, RealtimeAnalyzer, etc.) live in capture_common.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, Stream, StreamConfig};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use super::capture_common::{
    impl_capture_signal_accessors, AtomicSignals, CaptureConfig, CaptureError, CaptureState,
    RealtimeAnalyzer,
};

/// Special device name for system audio loopback capture
const LOOPBACK_DEVICE_NAME: &str = "System Output (Loopback)";

/// Live audio capture controller
///
/// Captures audio from input device and provides real-time signals.
/// Implements signal accessors via `impl_capture_signal_accessors!()` macro.
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

    /// List available capture devices (input devices + system output loopback).
    pub fn list_devices() -> Vec<String> {
        let host = cpal::default_host();
        let mut devices: Vec<String> = host
            .input_devices()
            .map(|devices| devices.filter_map(|d| d.name().ok()).collect())
            .unwrap_or_default();

        // Add system output (loopback) if an output device is available
        if host.default_output_device().is_some() {
            devices.push(LOOPBACK_DEVICE_NAME.to_string());
        }

        devices
    }

    /// Start capturing from the default input device.
    pub fn start(&mut self) -> Result<(), CaptureError> {
        self.start_device(None)
    }

    /// Start capturing from a specific device by name.
    ///
    /// If device_name is the loopback device name, captures from the default output device.
    pub fn start_device(&mut self, device_name: Option<&str>) -> Result<(), CaptureError> {
        if self.state == CaptureState::Capturing {
            return Ok(()); // Already capturing
        }

        let host = cpal::default_host();

        let is_loopback = device_name == Some(LOOPBACK_DEVICE_NAME);

        let device = if is_loopback {
            host.default_output_device()
                .ok_or(CaptureError::NoInputDevice)?
        } else if let Some(name) = device_name {
            host.input_devices()
                .map_err(|e| CaptureError::DeviceError(e.to_string()))?
                .find(|d| d.name().ok().as_deref() == Some(name))
                .ok_or_else(|| CaptureError::DeviceNotFound(name.to_string()))?
        } else {
            host.default_input_device()
                .ok_or(CaptureError::NoInputDevice)?
        };

        let supported_config = if is_loopback {
            device
                .default_output_config()
                .map_err(|e| CaptureError::ConfigError(e.to_string()))?
        } else {
            device
                .default_input_config()
                .map_err(|e| CaptureError::ConfigError(e.to_string()))?
        };

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

    // === Signal accessors (generated by macro) ===
    impl_capture_signal_accessors!();

    // === Private: cpal stream builders ===

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
        let mut analyzer =
            RealtimeAnalyzer::new(fft_size, sample_rate as f32, onset_threshold);
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
        let mut analyzer =
            RealtimeAnalyzer::new(fft_size, sample_rate as f32, onset_threshold);
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
        let mut analyzer =
            RealtimeAnalyzer::new(fft_size, sample_rate as f32, onset_threshold);
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
                        let sum: f32 =
                            frame.iter().map(|&s| (s as f32 - 32768.0) / 32768.0).sum();
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
}

impl Default for AudioCapture {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
