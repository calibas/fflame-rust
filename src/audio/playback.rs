//! Audio playback with timeline synchronization
//!
//! Provides audio file playback synced to the animation timeline.
//! Uses cpal for cross-platform audio output.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Sample, SampleFormat, Stream, StreamConfig};
// ringbuf reserved for future ring buffer playback improvements
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;

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

/// Audio playback controller
///
/// Manages audio output stream and synchronization with animation timeline.
pub struct AudioPlayer {
    /// Audio data to play
    audio_data: Option<Arc<AudioData>>,

    /// Current playback state
    state: PlaybackState,

    /// Current playback position in samples
    position: Arc<AtomicU64>,

    /// Whether playback should continue
    playing: Arc<AtomicBool>,

    /// Set to true when playback reaches the end
    ended: Arc<AtomicBool>,

    /// Audio output stream (kept alive while playing)
    _stream: Option<Stream>,

    /// Sample rate of loaded audio (source)
    sample_rate: u32,

    /// Sample rate of output device (set when stream is created)
    output_sample_rate: u32,

    /// Number of channels
    channels: u16,
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
            _stream: None,
            sample_rate: 44100,
            output_sample_rate: 44100,
            channels: 2,
        }
    }

    /// Load audio data for playback.
    pub fn load(&mut self, audio_data: AudioData) {
        self.stop();
        self.sample_rate = audio_data.sample_rate;
        self.channels = audio_data.channels;
        self.audio_data = Some(Arc::new(audio_data));
        self.position.store(0, Ordering::SeqCst);
    }

    /// Check if audio is loaded.
    pub fn has_audio(&self) -> bool {
        self.audio_data.is_some()
    }

    /// Get the current playback state.
    pub fn state(&mut self) -> PlaybackState {
        // Check if playback ended naturally
        if self.state == PlaybackState::Playing && self.ended.load(Ordering::SeqCst) {
            self.state = PlaybackState::Stopped;
            self.playing.store(false, Ordering::SeqCst);
            self.position.store(0, Ordering::SeqCst);
            self._stream = None;
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
        // pos is in output frames (device sample rate), not source frames
        let pos_frames = self.position.load(Ordering::SeqCst);
        pos_frames as f64 / self.output_sample_rate as f64
    }

    /// Get the duration of loaded audio in seconds.
    pub fn duration(&self) -> Option<f64> {
        self.audio_data.as_ref().map(|d| d.duration())
    }

    /// Start or resume playback.
    pub fn play(&mut self) -> Result<(), PlaybackError> {
        if self.audio_data.is_none() {
            return Err(PlaybackError::NoAudio);
        }

        if self.state == PlaybackState::Playing {
            return Ok(()); // Already playing
        }

        // Reset ended flag
        self.ended.store(false, Ordering::SeqCst);

        // If we were paused, just resume
        if self.state == PlaybackState::Paused && self._stream.is_some() {
            self.playing.store(true, Ordering::SeqCst);
            self.state = PlaybackState::Playing;
            return Ok(());
        }

        // Start new playback
        self.start_stream()?;
        self.state = PlaybackState::Playing;
        Ok(())
    }

    /// Pause playback (retains position).
    pub fn pause(&mut self) {
        if self.state == PlaybackState::Playing {
            self.playing.store(false, Ordering::SeqCst);
            self.state = PlaybackState::Paused;
        }
    }

    /// Stop playback and reset position.
    pub fn stop(&mut self) {
        self.playing.store(false, Ordering::SeqCst);
        self.ended.store(false, Ordering::SeqCst);
        self.position.store(0, Ordering::SeqCst);
        self._stream = None;
        self.state = PlaybackState::Stopped;
    }

    /// Seek to a specific time in seconds.
    pub fn seek(&mut self, time: f64) {
        // pos is in output frames (device sample rate)
        let frame_pos = (time * self.output_sample_rate as f64) as u64;
        // max_frames in output frame space
        let rate_ratio = self.sample_rate as f64 / self.output_sample_rate as f64;
        let max_source_frames = self
            .audio_data
            .as_ref()
            .map(|d| (d.samples.len() / d.channels as usize) as f64)
            .unwrap_or(0.0);
        let max_output_frames = (max_source_frames / rate_ratio) as u64;
        let clamped = frame_pos.min(max_output_frames);
        self.position.store(clamped, Ordering::SeqCst);

        // Clear ended flag when seeking before the end so the stream resumes output
        if clamped < max_output_frames {
            self.ended.store(false, Ordering::SeqCst);
        }
    }

    /// Sync playback position to animation time.
    ///
    /// Call this from the animation loop to keep audio in sync.
    /// If the difference is too large, it will seek to the correct position.
    /// If audio ended (e.g., animation looped), restarts playback automatically.
    pub fn sync_to_time(&mut self, animation_time: f64) {
        // Handle audio that has ended (animation looped past audio end)
        if self.state != PlaybackState::Playing || self.ended.load(Ordering::SeqCst) {
            self.seek(animation_time);
            // seek() clears `ended` when position is before the audio end
            if !self.ended.load(Ordering::SeqCst) && self.state != PlaybackState::Playing {
                // Stream was dropped by state() — restart it
                if let Err(e) = self.play() {
                    log::warn!("Failed to restart audio for loop: {:?}", e);
                }
            }
            return;
        }

        let current_pos = self.position_seconds();
        let diff = (animation_time - current_pos).abs();

        // If more than 100ms out of sync, seek to correct position
        if diff > 0.1 {
            self.seek(animation_time);
        }
    }

    /// Start the audio output stream.
    fn start_stream(&mut self) -> Result<(), PlaybackError> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or(PlaybackError::NoOutputDevice)?;

        let supported_config = device
            .default_output_config()
            .map_err(|e| PlaybackError::ConfigError(e.to_string()))?;

        // Store the actual output device sample rate for position tracking
        self.output_sample_rate = supported_config.sample_rate().0;

        let audio_data = self.audio_data.clone().unwrap();
        let position = self.position.clone();
        let playing = self.playing.clone();
        let channels = self.channels as usize;
        let source_rate = self.sample_rate;

        // Mark as playing
        playing.store(true, Ordering::SeqCst);

        let ended = self.ended.clone();

        let stream = match supported_config.sample_format() {
            SampleFormat::F32 => self.build_stream::<f32>(
                &device,
                &supported_config.into(),
                audio_data,
                position,
                playing,
                ended,
                channels,
                source_rate,
            )?,
            SampleFormat::I16 => self.build_stream::<i16>(
                &device,
                &supported_config.into(),
                audio_data,
                position,
                playing,
                ended,
                channels,
                source_rate,
            )?,
            SampleFormat::U16 => self.build_stream::<u16>(
                &device,
                &supported_config.into(),
                audio_data,
                position,
                playing,
                ended,
                channels,
                source_rate,
            )?,
            _ => return Err(PlaybackError::UnsupportedFormat),
        };

        stream
            .play()
            .map_err(|e| PlaybackError::StreamError(e.to_string()))?;

        self._stream = Some(stream);
        Ok(())
    }

    /// Build an audio stream for a specific sample format.
    fn build_stream<T: Sample + cpal::SizedSample + cpal::FromSample<f32>>(
        &self,
        device: &Device,
        config: &StreamConfig,
        audio_data: Arc<AudioData>,
        position: Arc<AtomicU64>,
        playing: Arc<AtomicBool>,
        ended: Arc<AtomicBool>,
        source_channels: usize,
        source_rate: u32,
    ) -> Result<Stream, PlaybackError> {
        let output_channels = config.channels as usize;
        let output_rate = config.sample_rate.0;

        // Simple resampling ratio (no interpolation for now)
        let rate_ratio = source_rate as f64 / output_rate as f64;

        let stream = device
            .build_output_stream(
                config,
                move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
                    if !playing.load(Ordering::SeqCst) {
                        // Output silence when paused
                        for sample in data.iter_mut() {
                            *sample = T::EQUILIBRIUM;
                        }
                        return;
                    }

                    // pos is in output frames
                    let mut pos = position.load(Ordering::SeqCst) as usize;
                    let samples = &audio_data.samples;
                    let total_source_frames = samples.len() / source_channels;

                    for frame in data.chunks_mut(output_channels) {
                        // Calculate which source frame to read (with resampling)
                        let source_frame = (pos as f64 * rate_ratio) as usize;

                        if source_frame >= total_source_frames {
                            // End of audio - output silence and signal ended
                            for sample in frame.iter_mut() {
                                *sample = T::EQUILIBRIUM;
                            }
                            // Only set ended once (when we first reach the end)
                            if !ended.load(Ordering::SeqCst) {
                                ended.store(true, Ordering::SeqCst);
                            }
                            continue;
                        }

                        let source_frame_start = source_frame * source_channels;

                        for (ch, sample) in frame.iter_mut().enumerate() {
                            let source_ch = ch % source_channels;
                            let idx = source_frame_start + source_ch;
                            let value = if idx < samples.len() {
                                samples[idx]
                            } else {
                                0.0
                            };
                            *sample = T::from_sample(value);
                        }

                        pos += 1; // Advance one output frame
                    }

                    position.store(pos as u64, Ordering::SeqCst);
                },
                |err| {
                    log::error!("Audio stream error: {}", err);
                },
                None,
            )
            .map_err(|e| PlaybackError::StreamError(e.to_string()))?;

        Ok(stream)
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
    /// No audio output device available
    NoOutputDevice,
    /// Error configuring audio device
    ConfigError(String),
    /// Error with audio stream
    StreamError(String),
    /// Unsupported sample format
    UnsupportedFormat,
}

impl std::fmt::Display for PlaybackError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlaybackError::NoAudio => write!(f, "No audio loaded"),
            PlaybackError::NoOutputDevice => write!(f, "No audio output device available"),
            PlaybackError::ConfigError(s) => write!(f, "Audio config error: {}", s),
            PlaybackError::StreamError(s) => write!(f, "Audio stream error: {}", s),
            PlaybackError::UnsupportedFormat => write!(f, "Unsupported audio format"),
        }
    }
}

impl std::error::Error for PlaybackError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_player_creation() {
        let mut player = AudioPlayer::new();
        assert!(!player.has_audio());
        assert_eq!(player.state(), PlaybackState::Stopped);
        assert_eq!(player.position_seconds(), 0.0);
    }

    #[test]
    fn test_player_load() {
        let mut player = AudioPlayer::new();
        let audio = AudioData {
            samples: vec![0.0; 44100 * 2], // 1 second stereo
            sample_rate: 44100,
            channels: 2,
        };

        player.load(audio);
        assert!(player.has_audio());
        assert!((player.duration().unwrap() - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_player_seek() {
        let mut player = AudioPlayer::new();
        let audio = AudioData {
            samples: vec![0.0; 44100 * 2 * 10], // 10 seconds stereo
            sample_rate: 44100,
            channels: 2,
        };

        player.load(audio);
        player.seek(5.0);
        assert!((player.position_seconds() - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_play_without_audio() {
        let mut player = AudioPlayer::new();
        let result = player.play();
        assert!(matches!(result, Err(PlaybackError::NoAudio)));
    }
}
