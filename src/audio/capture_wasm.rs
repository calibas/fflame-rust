//! WASM live audio capture using Web Audio API
//!
//! Provides real-time audio input capture via browser's getUserMedia API.
//! Uses ScriptProcessorNode (deprecated but widely supported) for audio processing.

use rustfft::{num_complex::Complex, FftPlanner};
use std::cell::{Cell, RefCell};
use std::f32::consts::PI;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{AudioContext, AudioContextOptions, MediaStream, ScriptProcessorNode};

use crate::signal::{Signal, SignalProducer};

/// Device name used to identify screen/tab audio capture via getDisplayMedia
const SCREEN_AUDIO_DEVICE: &str = "Screen/Tab Audio";

/// Live audio capture state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureState {
    /// Not capturing
    Stopped,
    /// Waiting for user permission
    RequestingPermission,
    /// Actively capturing audio
    Capturing,
    /// Permission denied or error occurred
    Error,
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
            fft_size: 1024,
            onset_threshold: 0.3,
        }
    }
}

/// Real-time audio analyzer
struct RealtimeAnalyzer {
    fft_size: usize,
    sample_rate: f32,
    window: Vec<f32>,
    buffer: Vec<f32>,
    buffer_pos: usize,
    fft_scratch: Vec<Complex<f32>>,
    prev_magnitudes: Vec<f32>,
    onset_threshold: f32,
    max_amplitude: f32,
    max_energy: f32,
    max_flux: f32,
}

impl RealtimeAnalyzer {
    fn new(fft_size: usize, sample_rate: f32, onset_threshold: f32) -> Self {
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

    fn process_samples(&mut self, samples: &[f32], signals: &Arc<AtomicSignals>) {
        for &sample in samples {
            self.buffer[self.buffer_pos] = sample;
            self.buffer_pos += 1;

            if self.buffer_pos >= self.fft_size {
                self.analyze_buffer(signals);
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

        // Compute energy bands
        let low_end = num_bins / 6;
        let mid_end = num_bins / 2;

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

        let total_energy = low_energy + mid_energy + high_energy + 0.0001;
        self.max_energy = self.max_energy.max(total_energy) * 0.999 + total_energy * 0.001;

        signals.set_energy_low((low_energy / self.max_energy * 3.0).clamp(0.0, 1.0));
        signals.set_energy_mid((mid_energy / self.max_energy * 3.0).clamp(0.0, 1.0));
        signals.set_energy_high((high_energy / self.max_energy * 3.0).clamp(0.0, 1.0));

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

        // Spectral flux
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

/// WASM Audio capture context holder
struct WasmCaptureContext {
    audio_context: AudioContext,
    processor_node: Option<ScriptProcessorNode>,
    // Keep source_node and stream alive to prevent browser GC from collecting them
    _source_node: Option<web_sys::MediaStreamAudioSourceNode>,
    _stream: Option<MediaStream>,
    _closure: Option<Closure<dyn FnMut(web_sys::AudioProcessingEvent)>>,
}

/// Live audio capture controller for WASM
///
/// Captures audio from browser microphone and provides real-time signals.
pub struct AudioCapture {
    /// Current capture state
    state: CaptureState,

    /// Atomic signals (shared with audio processing)
    signals: Arc<AtomicSignals>,

    /// Whether capture is active
    active: Arc<AtomicBool>,

    /// WASM audio context (kept in RefCell for interior mutability)
    context: Rc<RefCell<Option<WasmCaptureContext>>>,

    /// Sample rate of capture device
    sample_rate: u32,

    /// Capture configuration
    config: CaptureConfig,

    /// Error message if any
    error_message: Option<String>,

    /// Shared state for async callbacks to signal errors back to the UI.
    /// WASM is single-threaded so Rc<Cell<>> is safe here.
    callback_state: Rc<Cell<Option<CaptureState>>>,
    callback_error: Rc<RefCell<Option<String>>>,
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
            context: Rc::new(RefCell::new(None)),
            sample_rate: 44100,
            config,
            error_message: None,
            callback_state: Rc::new(Cell::new(None)),
            callback_error: Rc::new(RefCell::new(None)),
        }
    }

    /// Get the current capture state.
    ///
    /// Checks shared callback state first — async callbacks may have
    /// signaled an error after `start_device()` returned.
    pub fn state(&self) -> CaptureState {
        if let Some(cb_state) = self.callback_state.get() {
            return cb_state;
        }
        self.state
    }

    /// Check if currently capturing.
    pub fn is_capturing(&self) -> bool {
        self.state() == CaptureState::Capturing && self.active.load(Ordering::Relaxed)
    }

    /// Get error message if capture failed.
    pub fn error_message(&self) -> Option<String> {
        if let Some(ref msg) = self.error_message {
            return Some(msg.clone());
        }
        self.callback_error.borrow().clone()
    }

    /// List available input devices for WASM.
    ///
    /// Returns a default microphone option plus a screen/tab audio option
    /// (which uses getDisplayMedia for capturing audio from other tabs or system audio).
    pub fn list_devices() -> Vec<String> {
        vec![
            "Default".to_string(),
            SCREEN_AUDIO_DEVICE.to_string(),
        ]
    }

    /// Start capturing from the default input device.
    ///
    /// This is an async operation that will request microphone permission.
    /// Call `poll_state()` to check when capture has started.
    pub fn start(&mut self) -> Result<(), CaptureError> {
        self.start_device(None)
    }

    /// Start capturing from the specified device.
    ///
    /// If device_name is "Screen/Tab Audio", uses getDisplayMedia to capture
    /// audio from other browser tabs or system audio (Windows/ChromeOS).
    /// Otherwise, uses getUserMedia for microphone capture.
    pub fn start_device(&mut self, device_name: Option<&str>) -> Result<(), CaptureError> {
        if self.state == CaptureState::Capturing {
            return Ok(());
        }

        self.state = CaptureState::RequestingPermission;
        self.error_message = None;
        self.callback_state.set(None);
        *self.callback_error.borrow_mut() = None;

        let is_screen_audio = device_name
            .map(|name| name == SCREEN_AUDIO_DEVICE)
            .unwrap_or(false);

        // Get window and navigator
        let window = web_sys::window().ok_or(CaptureError::NoInputDevice)?;
        let navigator = window.navigator();
        let media_devices = navigator
            .media_devices()
            .map_err(|_| CaptureError::NoInputDevice)?;

        // Clone values for the async closure
        let signals = self.signals.clone();
        let active = self.active.clone();
        let context_holder = self.context.clone();
        let fft_size = self.config.fft_size;
        let onset_threshold = self.config.onset_threshold;
        let cb_state = self.callback_state.clone();
        let cb_error = self.callback_error.clone();

        // Get the media stream promise based on capture type
        let promise = if is_screen_audio {
            // Use getDisplayMedia for screen/tab audio capture.
            // Spec requires video: true, but we immediately stop the video track.
            let mut constraints = web_sys::DisplayMediaStreamConstraints::new();
            constraints.audio(&JsValue::TRUE);
            constraints.video(&JsValue::TRUE);

            media_devices
                .get_display_media_with_constraints(&constraints)
                .map_err(|_| CaptureError::StreamError("Failed to request display media".to_string()))?
        } else {
            // Use getUserMedia for microphone capture
            let mut constraints = web_sys::MediaStreamConstraints::new();
            constraints.audio(&JsValue::TRUE);
            constraints.video(&JsValue::FALSE);

            media_devices
                .get_user_media_with_constraints(&constraints)
                .map_err(|_| CaptureError::StreamError("Failed to request media".to_string()))?
        };

        // Handle the promise - shared success path for both capture types
        let success_callback = Closure::once(Box::new(move |stream: JsValue| {
            let stream: MediaStream = stream.dyn_into().unwrap();

            // For screen capture: stop video tracks immediately (we only need audio)
            if is_screen_audio {
                let video_tracks = stream.get_video_tracks();
                for i in 0..video_tracks.length() {
                    if let Some(track) = video_tracks.get(i).dyn_ref::<web_sys::MediaStreamTrack>() {
                        track.stop();
                    }
                }

                // Verify we actually got audio tracks.
                // Firefox doesn't support audio in getDisplayMedia — it silently drops the
                // audio constraint, resulting in 0 audio tracks.
                if stream.get_audio_tracks().length() == 0 {
                    let msg = "audio.not_supported";
                    log::error!("{}", msg);
                    cb_state.set(Some(CaptureState::Error));
                    *cb_error.borrow_mut() = Some(msg.to_string());
                    return;
                }
            }

            // Create AudioContext
            let audio_context = match AudioContext::new() {
                Ok(ctx) => ctx,
                Err(e) => {
                    log::error!("Failed to create AudioContext: {:?}", e);
                    return;
                }
            };

            // Resume AudioContext - browsers create it in "suspended" state due to autoplay policy.
            // Without this, onaudioprocess callbacks will never fire.
            if audio_context.state() == web_sys::AudioContextState::Suspended {
                let _ = audio_context.resume();
            }

            let sample_rate = audio_context.sample_rate();

            // Create source node from media stream
            let source_node = match audio_context.create_media_stream_source(&stream) {
                Ok(node) => node,
                Err(e) => {
                    log::error!("Failed to create source node: {:?}", e);
                    return;
                }
            };

            // Create ScriptProcessorNode for audio processing
            // Buffer size of 2048 gives good balance between latency and CPU usage
            let buffer_size = 2048;
            let processor_node = match audio_context
                .create_script_processor_with_buffer_size_and_number_of_input_channels_and_number_of_output_channels(
                    buffer_size,
                    1, // mono input
                    1, // mono output
                )
            {
                Ok(node) => node,
                Err(e) => {
                    log::error!("Failed to create processor node: {:?}", e);
                    return;
                }
            };

            // Set up the audio processing callback
            let signals_clone = signals.clone();
            let active_clone = active.clone();
            let analyzer = Rc::new(RefCell::new(RealtimeAnalyzer::new(
                fft_size,
                sample_rate,
                onset_threshold,
            )));

            let onaudioprocess = Closure::wrap(Box::new(move |event: web_sys::AudioProcessingEvent| {
                if !active_clone.load(Ordering::SeqCst) {
                    return;
                }

                // Get input buffer
                let input_buffer = match event.input_buffer() {
                    Ok(buf) => buf,
                    Err(_) => return,
                };

                // Get channel data
                let channel_data = match input_buffer.get_channel_data(0) {
                    Ok(data) => data,
                    Err(_) => return,
                };

                // Process samples
                let mut analyzer = analyzer.borrow_mut();
                analyzer.process_samples(&channel_data, &signals_clone);
            }) as Box<dyn FnMut(web_sys::AudioProcessingEvent)>);

            processor_node.set_onaudioprocess(Some(onaudioprocess.as_ref().unchecked_ref()));

            // Connect: source -> processor -> destination
            if let Err(e) = source_node.connect_with_audio_node(&processor_node) {
                log::error!("Failed to connect source to processor: {:?}", e);
                return;
            }

            if let Err(e) = processor_node.connect_with_audio_node(&audio_context.destination()) {
                log::error!("Failed to connect processor to destination: {:?}", e);
                return;
            }

            // Mark as active
            active.store(true, Ordering::SeqCst);

            // Store context, nodes, and closures to prevent browser GC
            *context_holder.borrow_mut() = Some(WasmCaptureContext {
                audio_context,
                processor_node: Some(processor_node),
                _source_node: Some(source_node),
                _stream: Some(stream),
                _closure: Some(onaudioprocess),
            });

            let source = if is_screen_audio { "screen/tab" } else { "microphone" };
            log::info!("WASM {} audio capture started successfully", source);
        }) as Box<dyn FnOnce(JsValue)>);

        let err_cb_state = self.callback_state.clone();
        let err_cb_error = self.callback_error.clone();
        let error_callback = Closure::once(Box::new(move |err: JsValue| {
            let msg = format!("Media capture error: {:?}", err);
            log::error!("{}", msg);
            err_cb_state.set(Some(CaptureState::Error));
            *err_cb_error.borrow_mut() = Some(msg);
        }) as Box<dyn FnOnce(JsValue)>);

        let _ = promise.then(&success_callback).catch(&error_callback);

        // Keep closures alive
        success_callback.forget();
        error_callback.forget();

        // We don't know if it succeeded yet - state will be updated in callback
        // For now, assume it will work
        self.state = CaptureState::Capturing;
        Ok(())
    }

    /// Stop capturing.
    pub fn stop(&mut self) {
        self.active.store(false, Ordering::SeqCst);

        // Close the audio context
        if let Some(ctx) = self.context.borrow_mut().take() {
            let _ = ctx.audio_context.close();
        }

        self.state = CaptureState::Stopped;
        self.callback_state.set(None);
        *self.callback_error.borrow_mut() = None;
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

    /// Get list of available live signal names.
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

    /// Create a signal producer bridge that shares atomic signals with this capture.
    pub fn create_producer(&self) -> Box<dyn SignalProducer> {
        Box::new(LiveSignalBridge {
            signals: self.signals.clone(),
            active: self.active.clone(),
        })
    }
}

/// Lightweight bridge for sharing live capture signals with SignalManager.
struct LiveSignalBridge {
    signals: Arc<AtomicSignals>,
    active: Arc<AtomicBool>,
}

// SAFETY: On WASM, everything is single-threaded. The Arc<AtomicSignals> and Arc<AtomicBool>
// are inherently Send+Sync via their atomic internals.
unsafe impl Send for LiveSignalBridge {}

impl SignalProducer for LiveSignalBridge {
    fn signal_names(&self) -> Vec<String> {
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

    fn get_live_value(&self, name: &str) -> Option<f32> {
        if !self.active.load(Ordering::Relaxed) {
            return None;
        }
        match name {
            "live_amplitude" => Some(self.signals.get_amplitude()),
            "live_energy_low" => Some(self.signals.get_energy_low()),
            "live_energy_mid" => Some(self.signals.get_energy_mid()),
            "live_energy_high" => Some(self.signals.get_energy_high()),
            "live_spectral_centroid" => Some(self.signals.get_spectral_centroid()),
            "live_spectral_flux" => Some(self.signals.get_spectral_flux()),
            "live_onset" => Some(self.signals.get_onset()),
            _ => None,
        }
    }

    fn get_signal(&self, _name: &str) -> Option<Signal> {
        None
    }

    fn is_active(&self) -> bool {
        self.active.load(Ordering::Relaxed)
    }
}

impl Default for AudioCapture {
    fn default() -> Self {
        Self::new()
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
    /// Permission denied by user
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
