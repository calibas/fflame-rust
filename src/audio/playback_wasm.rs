//! WASM audio playback using Web Audio API
//!
//! Provides audio file playback synced to the animation timeline.
//! Uses AudioBufferSourceNode for efficient browser-based playback.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{AudioBuffer, AudioBufferSourceNode, AudioContext};

use super::AudioData;

/// Audio playback state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    /// No audio loaded or playback stopped
    Stopped,
    /// Currently playing
    Playing,
    /// Paused at current position
    Paused,
}

/// Audio playback controller for WASM
///
/// Manages audio output using Web Audio API.
pub struct AudioPlayer {
    /// Audio data to play
    audio_data: Option<AudioData>,

    /// Current playback state
    state: PlaybackState,

    /// Current playback position in samples (updated during playback)
    position: Arc<AtomicU64>,

    /// Whether playback should continue
    playing: Arc<AtomicBool>,

    /// Set to true when playback reaches the end
    ended: Arc<AtomicBool>,

    /// Web Audio context
    audio_context: Option<AudioContext>,

    /// Current source node (recreated for each play)
    source_node: Option<AudioBufferSourceNode>,

    /// Cached AudioBuffer for the loaded audio
    audio_buffer: Option<AudioBuffer>,

    /// Sample rate of loaded audio
    sample_rate: u32,

    /// Number of channels
    channels: u16,

    /// Time when playback started (for position tracking)
    start_time: f64,

    /// Offset when playback started (for resume after pause)
    start_offset: f64,
}

impl AudioPlayer {
    /// Create a new audio player with no audio loaded.
    pub fn new() -> Self {
        Self {
            audio_data: None,
            state: PlaybackState::Stopped,
            position: Arc::new(AtomicU64::new(0)),
            playing: Arc::new(AtomicBool::new(false)),
            ended: Arc::new(AtomicBool::new(false)),
            audio_context: None,
            source_node: None,
            audio_buffer: None,
            sample_rate: 44100,
            channels: 2,
            start_time: 0.0,
            start_offset: 0.0,
        }
    }

    /// Initialize the audio context (requires user gesture on first call)
    fn ensure_context(&mut self) -> Result<&AudioContext, PlaybackError> {
        if self.audio_context.is_none() {
            let context = AudioContext::new()
                .map_err(|e| PlaybackError::ContextError(format!("{:?}", e)))?;
            self.audio_context = Some(context);
        }
        Ok(self.audio_context.as_ref().unwrap())
    }

    /// Load audio data for playback.
    pub fn load(&mut self, audio_data: AudioData) {
        self.stop();
        self.sample_rate = audio_data.sample_rate;
        self.channels = audio_data.channels;

        // Create AudioBuffer from the audio data
        if let Ok(context) = self.ensure_context() {
            let num_frames = audio_data.samples.len() / audio_data.channels as usize;

            if let Ok(buffer) = context.create_buffer(
                audio_data.channels as u32,
                num_frames as u32,
                audio_data.sample_rate as f32,
            ) {
                // Deinterleave and copy samples to each channel
                for ch in 0..audio_data.channels as usize {
                    let mut channel_samples = Vec::with_capacity(num_frames);
                    for frame in 0..num_frames {
                        let idx = frame * audio_data.channels as usize + ch;
                        if idx < audio_data.samples.len() {
                            channel_samples.push(audio_data.samples[idx]);
                        }
                    }
                    // Copy samples to the AudioBuffer channel
                    if let Err(e) = buffer.copy_to_channel(&channel_samples, ch as i32) {
                        log::error!("Failed to copy to channel {}: {:?}", ch, e);
                    }
                }
                self.audio_buffer = Some(buffer);
                log::info!("Created AudioBuffer: {} frames, {} channels, {} Hz",
                    num_frames, audio_data.channels, audio_data.sample_rate);
            } else {
                log::error!("Failed to create AudioBuffer");
            }
        }

        self.audio_data = Some(audio_data);
        self.position.store(0, Ordering::SeqCst);
    }

    /// Check if audio is loaded.
    pub fn has_audio(&self) -> bool {
        self.audio_data.is_some() && self.audio_buffer.is_some()
    }

    /// Get the current playback state.
    pub fn state(&mut self) -> PlaybackState {
        // Check if playback ended naturally
        if self.state == PlaybackState::Playing && self.ended.load(Ordering::SeqCst) {
            self.state = PlaybackState::Stopped;
            self.playing.store(false, Ordering::SeqCst);
            self.start_offset = 0.0;
            self.source_node = None;
            log::info!("Playback ended");
        }
        self.state
    }

    /// Get the current playback position in seconds.
    pub fn position_seconds(&self) -> f64 {
        // If ended, return 0 (will be at start for next play)
        if self.ended.load(Ordering::SeqCst) {
            return 0.0;
        }

        if self.state == PlaybackState::Playing {
            if let Some(ref context) = self.audio_context {
                let current_time = context.current_time();
                let position = self.start_offset + (current_time - self.start_time);

                // Clamp to duration
                if let Some(duration) = self.duration() {
                    return position.min(duration);
                }
                return position;
            }
        }
        self.start_offset
    }

    /// Get the duration of loaded audio in seconds.
    pub fn duration(&self) -> Option<f64> {
        self.audio_data.as_ref().map(|d| d.duration())
    }

    /// Start or resume playback.
    pub fn play(&mut self) -> Result<(), PlaybackError> {
        if self.audio_buffer.is_none() {
            return Err(PlaybackError::NoAudio);
        }

        if self.state == PlaybackState::Playing {
            return Ok(()); // Already playing
        }

        // Reset ended flag
        self.ended.store(false, Ordering::SeqCst);

        // Ensure context exists
        self.ensure_context()?;

        // Now borrow context immutably for the rest of the operations
        let context = self.audio_context.as_ref().unwrap();

        // Resume audio context if it's suspended (browser autoplay policy)
        if context.state() == web_sys::AudioContextState::Suspended {
            let _ = context.resume();
        }

        // Create a new source node (they can only be used once)
        let source = context.create_buffer_source()
            .map_err(|e| PlaybackError::StreamError(format!("{:?}", e)))?;

        source.set_buffer(self.audio_buffer.as_ref());

        // Set up onended callback to detect when playback finishes
        let ended_flag = self.ended.clone();
        let onended = Closure::wrap(Box::new(move || {
            ended_flag.store(true, Ordering::SeqCst);
        }) as Box<dyn Fn()>);
        source.set_onended(Some(onended.as_ref().unchecked_ref()));
        onended.forget(); // Leak closure - will be cleaned up when source is dropped

        // Connect to destination
        source.connect_with_audio_node(&context.destination())
            .map_err(|e| PlaybackError::StreamError(format!("{:?}", e)))?;

        // Start playback from the current offset
        let offset = self.start_offset;
        let _ = source.start_with_when_and_grain_offset(0.0, offset);

        self.start_time = context.current_time();
        self.source_node = Some(source);
        self.playing.store(true, Ordering::SeqCst);
        self.state = PlaybackState::Playing;

        log::info!("Started playback at offset {:.2}s", offset);
        Ok(())
    }

    /// Pause playback (retains position).
    pub fn pause(&mut self) {
        if self.state == PlaybackState::Playing {
            // Calculate current position before stopping
            self.start_offset = self.position_seconds();

            // Detach onended callback before stopping to prevent stale async callback
            if let Some(ref source) = self.source_node {
                source.set_onended(None);
                let _ = source.stop();
            }
            self.source_node = None;

            self.playing.store(false, Ordering::SeqCst);
            self.state = PlaybackState::Paused;
            log::info!("Paused at {:.2}s", self.start_offset);
        }
    }

    /// Stop playback and reset position.
    pub fn stop(&mut self) {
        // Detach onended callback before stopping to prevent stale async callback
        if let Some(ref source) = self.source_node {
            source.set_onended(None);
            let _ = source.stop();
        }
        self.source_node = None;

        self.playing.store(false, Ordering::SeqCst);
        self.ended.store(false, Ordering::SeqCst);
        self.position.store(0, Ordering::SeqCst);
        self.start_offset = 0.0;
        self.start_time = 0.0;
        self.state = PlaybackState::Stopped;
    }

    /// Seek to a specific time in seconds.
    pub fn seek(&mut self, time: f64) {
        let was_playing = self.state == PlaybackState::Playing;

        if was_playing {
            // Detach onended callback before stopping to prevent the old source's
            // async callback from setting ended=true after the new source starts
            if let Some(ref source) = self.source_node {
                source.set_onended(None);
                let _ = source.stop();
            }
            self.source_node = None;
        }

        // Update offset
        let duration = self.duration().unwrap_or(0.0);
        self.start_offset = time.clamp(0.0, duration);

        // Resume if was playing
        if was_playing {
            let _ = self.play();
        }
    }

    /// Sync playback position to animation time.
    ///
    /// Call this from the animation loop to keep audio in sync.
    /// If the difference is too large, it will seek to the correct position.
    pub fn sync_to_time(&mut self, animation_time: f64) {
        let current_pos = self.position_seconds();
        let diff = (animation_time - current_pos).abs();

        // If more than 100ms out of sync, seek to correct position
        if diff > 0.1 {
            self.seek(animation_time);
        }
    }
}

impl Default for AudioPlayer {
    fn default() -> Self {
        Self::new()
    }
}

/// Errors that can occur during playback.
#[derive(Debug)]
pub enum PlaybackError {
    /// No audio data loaded
    NoAudio,
    /// Error creating audio context
    ContextError(String),
    /// Error with audio stream
    StreamError(String),
}

impl std::fmt::Display for PlaybackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlaybackError::NoAudio => write!(f, "No audio loaded"),
            PlaybackError::ContextError(s) => write!(f, "Audio context error: {}", s),
            PlaybackError::StreamError(s) => write!(f, "Audio stream error: {}", s),
        }
    }
}

impl std::error::Error for PlaybackError {}
