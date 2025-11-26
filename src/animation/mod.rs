//! Animation system for automated parameter changes over time
//!
//! This module provides keyframe-based animation with track interpolation,
//! as well as procedural track types (oscillators, circular motion).
//! During playback, the animation controller updates ConfigManager silently
//! (without creating undo points).

use crate::config::ConfigPath;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

mod controller;
mod interpolation;
pub mod export;

pub use controller::AnimationController;
pub use interpolation::{EasingFunction, Interpolation};

/// Complete animation definition with parameter tracks
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Animation {
    /// User-facing animation name
    pub name: String,

    /// Total duration in seconds
    pub duration: f64,

    /// Parameter tracks (ConfigPath → Track)
    pub tracks: HashMap<String, Track>, // String instead of ConfigPath for JSON serialization

    /// Circular motion tracks (output X and Y to two parameters)
    #[serde(default)]
    pub circular_tracks: Vec<CircularTrack>,

    /// Looping behavior
    pub loop_mode: LoopMode,
}

/// Single parameter track
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    /// Source of track values (keyframes or procedural)
    pub source: TrackSource,

    /// Interpolation method (for keyframe tracks)
    #[serde(default)]
    pub interpolation: Interpolation,
}

/// Source of track values
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TrackSource {
    /// Traditional keyframe animation
    Keyframes {
        keyframes: Vec<Keyframe>,
    },

    /// Sinusoidal oscillation (no keyframes needed)
    Oscillator {
        oscillator_type: OscillatorType,
        /// Center value (oscillates around this)
        center: f64,
        /// Peak deviation from center
        amplitude: f64,
        /// Cycles per second
        frequency: f64,
        /// Starting phase (0.0-1.0, where 1.0 = full cycle)
        #[serde(default)]
        phase: f64,
    },
}

/// Type of oscillator waveform
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OscillatorType {
    /// Smooth sine wave
    Sine,
    /// Linear up/down (triangle wave)
    Triangle,
    /// Linear ramp up, instant reset
    Sawtooth,
    /// Instant flip between min and max
    Square,
}

/// Circular motion track (outputs to TWO parameters)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircularTrack {
    /// ConfigPath string for X output (e.g., "PanX" or "TransformAffine.0.E")
    pub target_x: String,
    /// ConfigPath string for Y output
    pub target_y: String,
    /// Center X coordinate
    pub center_x: f64,
    /// Center Y coordinate
    pub center_y: f64,
    /// Radius of circular motion
    pub radius: f64,
    /// Revolutions per second (negative = clockwise)
    pub speed: f64,
    /// Starting angle in radians
    #[serde(default)]
    pub phase: f64,
}

/// Single keyframe defining parameter value at specific time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Keyframe {
    /// Time in seconds from animation start
    pub time: f64,

    /// Parameter value at this time
    /// TODO: This should be ConfigValue, but it doesn't implement Serialize yet
    /// For now, store as JSON Value and convert at runtime
    pub value: serde_json::Value,

    /// Easing function to next keyframe
    pub easing: EasingFunction,
}

/// Loop behavior at animation end
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoopMode {
    /// Stop at last frame
    Once,

    /// Restart from beginning
    Loop,

    /// Reverse direction at ends
    PingPong,
}

/// Playback state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}

/// Animation quality mode - controls rendering behavior during playback
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnimationQualityMode {
    /// Responsive mode: uses overwrite blending for smooth real-time preview
    /// - Each frame immediately updates the display
    /// - Lower quality but no latency
    /// - Good for previewing animations
    #[default]
    Responsive,

    /// High quality mode: uses batched accumulation (4 frames) like normal rendering
    /// - Accumulates samples across 4 frames before blending
    /// - Higher quality with smoother density transitions
    /// - Has ~4 frame latency (parameter changes visible 4 frames later)
    /// - Good for final renders or slow animations
    HighQuality,
}

impl Animation {
    /// Create empty animation
    pub fn new(name: String, duration: f64) -> Self {
        Self {
            name,
            duration,
            tracks: HashMap::new(),
            circular_tracks: Vec::new(),
            loop_mode: LoopMode::Once,
        }
    }

    /// Add track for parameter using ConfigPath
    pub fn add_track(&mut self, path: ConfigPath, track: Track) {
        // Convert ConfigPath to string for JSON serialization
        let path_str = format!("{:?}", path); // TODO: Better serialization
        self.tracks.insert(path_str, track);
    }

    /// Add track for parameter using string key
    pub fn add_track_str(&mut self, path: String, track: Track) {
        self.tracks.insert(path, track);
    }

    /// Add circular motion track
    pub fn add_circular_track(&mut self, track: CircularTrack) {
        self.circular_tracks.push(track);
    }

    /// Load from JSON file
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Save to JSON string
    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

impl Track {
    /// Create track with single keyframe (constant value)
    pub fn constant(value: serde_json::Value) -> Self {
        Self {
            source: TrackSource::Keyframes {
                keyframes: vec![Keyframe {
                    time: 0.0,
                    value,
                    easing: EasingFunction::Linear,
                }],
            },
            interpolation: Interpolation::Linear,
        }
    }

    /// Create track with two keyframes (start → end)
    pub fn linear(start_value: serde_json::Value, end_value: serde_json::Value, duration: f64) -> Self {
        Self {
            source: TrackSource::Keyframes {
                keyframes: vec![
                    Keyframe {
                        time: 0.0,
                        value: start_value,
                        easing: EasingFunction::Linear,
                    },
                    Keyframe {
                        time: duration,
                        value: end_value,
                        easing: EasingFunction::Linear,
                    },
                ],
            },
            interpolation: Interpolation::Linear,
        }
    }

    /// Create oscillator track
    pub fn oscillator(
        oscillator_type: OscillatorType,
        center: f64,
        amplitude: f64,
        frequency: f64,
    ) -> Self {
        Self {
            source: TrackSource::Oscillator {
                oscillator_type,
                center,
                amplitude,
                frequency,
                phase: 0.0,
            },
            interpolation: Interpolation::Linear, // Not used for oscillators
        }
    }

    /// Create oscillator track with phase offset
    pub fn oscillator_with_phase(
        oscillator_type: OscillatorType,
        center: f64,
        amplitude: f64,
        frequency: f64,
        phase: f64,
    ) -> Self {
        Self {
            source: TrackSource::Oscillator {
                oscillator_type,
                center,
                amplitude,
                frequency,
                phase,
            },
            interpolation: Interpolation::Linear,
        }
    }

    /// Add keyframe in time-sorted order (only works for Keyframes source)
    pub fn add_keyframe(&mut self, keyframe: Keyframe) {
        if let TrackSource::Keyframes { ref mut keyframes } = self.source {
            keyframes.push(keyframe);
            keyframes.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
        }
    }

    /// Get keyframes (if this is a keyframe track)
    pub fn keyframes(&self) -> Option<&[Keyframe]> {
        match &self.source {
            TrackSource::Keyframes { keyframes } => Some(keyframes),
            _ => None,
        }
    }
}

impl CircularTrack {
    /// Create a new circular motion track
    pub fn new(
        target_x: String,
        target_y: String,
        center_x: f64,
        center_y: f64,
        radius: f64,
        speed: f64,
    ) -> Self {
        Self {
            target_x,
            target_y,
            center_x,
            center_y,
            radius,
            speed,
            phase: 0.0,
        }
    }

    /// Create circular track with starting phase
    pub fn with_phase(mut self, phase: f64) -> Self {
        self.phase = phase;
        self
    }

    /// Evaluate position at given time
    pub fn evaluate(&self, time: f64) -> (f64, f64) {
        let angle = time * self.speed * 2.0 * std::f64::consts::PI + self.phase;
        let x = self.center_x + self.radius * angle.cos();
        let y = self.center_y + self.radius * angle.sin();
        (x, y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_animation_json_roundtrip() {
        let mut anim = Animation::new("Test".into(), 10.0);
        anim.loop_mode = LoopMode::Loop;

        let track = Track::linear(
            serde_json::json!(1.0),
            serde_json::json!(10.0),
            10.0,
        );
        anim.tracks.insert("Zoom".into(), track);

        let json = anim.to_json().unwrap();
        let loaded = Animation::from_json(&json).unwrap();

        assert_eq!(loaded.name, "Test");
        assert_eq!(loaded.duration, 10.0);
        assert_eq!(loaded.loop_mode, LoopMode::Loop);
        assert_eq!(loaded.tracks.len(), 1);
    }

    #[test]
    fn test_oscillator_track_json() {
        let mut anim = Animation::new("Oscillate".into(), 5.0);

        let track = Track::oscillator(OscillatorType::Sine, 1.0, 0.5, 2.0);
        anim.tracks.insert("Exposure".into(), track);

        let json = anim.to_json().unwrap();
        let loaded = Animation::from_json(&json).unwrap();

        assert_eq!(loaded.tracks.len(), 1);
        let track = loaded.tracks.get("Exposure").unwrap();
        match &track.source {
            TrackSource::Oscillator { oscillator_type, center, amplitude, frequency, .. } => {
                assert_eq!(*oscillator_type, OscillatorType::Sine);
                assert_eq!(*center, 1.0);
                assert_eq!(*amplitude, 0.5);
                assert_eq!(*frequency, 2.0);
            }
            _ => panic!("Expected Oscillator track"),
        }
    }

    #[test]
    fn test_circular_track_json() {
        let mut anim = Animation::new("Circle".into(), 10.0);

        anim.add_circular_track(CircularTrack::new(
            "PanX".into(),
            "PanY".into(),
            0.5, -0.5,
            0.2,
            0.1,
        ));

        let json = anim.to_json().unwrap();
        let loaded = Animation::from_json(&json).unwrap();

        assert_eq!(loaded.circular_tracks.len(), 1);
        let ct = &loaded.circular_tracks[0];
        assert_eq!(ct.target_x, "PanX");
        assert_eq!(ct.target_y, "PanY");
        assert_eq!(ct.center_x, 0.5);
        assert_eq!(ct.center_y, -0.5);
        assert_eq!(ct.radius, 0.2);
        assert_eq!(ct.speed, 0.1);
    }

    #[test]
    fn test_full_animation_json() {
        // Test a complete animation with all track types
        let mut anim = Animation::new("Full Test".into(), 10.0);
        anim.loop_mode = LoopMode::PingPong;

        // Keyframe track
        anim.tracks.insert("Zoom".into(), Track::linear(
            serde_json::json!(1.0),
            serde_json::json!(5.0),
            10.0,
        ));

        // Oscillator track
        anim.tracks.insert("Brightness".into(), Track::oscillator(
            OscillatorType::Triangle,
            1.0,
            0.3,
            0.5,
        ));

        // Circular track
        anim.add_circular_track(CircularTrack::new(
            "PanX".into(),
            "PanY".into(),
            0.0, 0.0,
            0.5,
            0.25,
        ));

        let json = anim.to_json().unwrap();
        println!("Animation JSON:\n{}", json);

        let loaded = Animation::from_json(&json).unwrap();
        assert_eq!(loaded.name, "Full Test");
        assert_eq!(loaded.duration, 10.0);
        assert_eq!(loaded.loop_mode, LoopMode::PingPong);
        assert_eq!(loaded.tracks.len(), 2);
        assert_eq!(loaded.circular_tracks.len(), 1);
    }
}
