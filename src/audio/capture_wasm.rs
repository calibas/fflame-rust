//! WASM live audio capture using Web Audio API
//!
//! Platform-specific capture implementation. All shared types (AtomicSignals,
//! CaptureConfig, CaptureError, RealtimeAnalyzer, etc.) live in capture_common.
//!
//! Uses ScriptProcessorNode (deprecated but widely supported) for audio processing.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{AudioContext, MediaStream, ScriptProcessorNode};

use super::capture_common::{
    impl_capture_signal_accessors, AtomicSignals, CaptureConfig, CaptureError, CaptureState,
    RealtimeAnalyzer,
};

/// Device name used to identify screen/tab audio capture via getDisplayMedia
const SCREEN_AUDIO_DEVICE: &str = "Screen/Tab Audio";

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
/// Captures audio from browser microphone or screen/tab and provides real-time signals.
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
                .map_err(|_| {
                    CaptureError::StreamError("Failed to request display media".to_string())
                })?
        } else {
            // Use getUserMedia for microphone capture.
            //
            // `audio: true` accepts the browser's voice-call defaults:
            // automatic gain control, noise suppression and echo
            // cancellation are all ON. For a visualiser that is exactly
            // wrong — AGC continuously renormalises the level, so a
            // sustained loud passage fades and silence creeps upward, and
            // the fractal responds to the compressor rather than to the
            // music. Ask for the raw signal instead, and let the in-app
            // gain be the only thing scaling it.
            //
            // These are requests, not guarantees: a browser or OS that
            // will not honour one simply ignores it, which is why the
            // in-app gain still has to exist.
            let audio_constraints = js_sys::Object::new();
            for (key, value) in [
                ("autoGainControl", false),
                ("noiseSuppression", false),
                ("echoCancellation", false),
            ] {
                let _ = js_sys::Reflect::set(
                    &audio_constraints,
                    &JsValue::from_str(key),
                    &JsValue::from_bool(value),
                );
            }
            let mut constraints = web_sys::MediaStreamConstraints::new();
            constraints.audio(&audio_constraints);
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
                    if let Some(track) =
                        video_tracks.get(i).dyn_ref::<web_sys::MediaStreamTrack>()
                    {
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

            let onaudioprocess =
                Closure::wrap(Box::new(move |event: web_sys::AudioProcessingEvent| {
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
                })
                    as Box<dyn FnMut(web_sys::AudioProcessingEvent)>);

            processor_node.set_onaudioprocess(Some(onaudioprocess.as_ref().unchecked_ref()));

            // Connect: source -> processor -> destination
            if let Err(e) = source_node.connect_with_audio_node(&processor_node) {
                log::error!("Failed to connect source to processor: {:?}", e);
                return;
            }

            if let Err(e) =
                processor_node.connect_with_audio_node(&audio_context.destination())
            {
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

            let source = if is_screen_audio {
                "screen/tab"
            } else {
                "microphone"
            };
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

    // === Signal accessors (generated by macro) ===
    impl_capture_signal_accessors!();
}

impl Default for AudioCapture {
    fn default() -> Self {
        Self::new()
    }
}
