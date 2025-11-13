//! Animation system for automated parameter changes over time
//!
//! This module provides keyframe-based animation with track interpolation.
//! During playback, the animation controller updates ConfigManager silently
//! (without creating undo points).

use crate::config::{ConfigPath, ConfigValue};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

mod controller;
mod interpolation;

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

    /// Looping behavior
    pub loop_mode: LoopMode,
}

/// Single parameter track with keyframes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    /// Keyframes sorted by time
    pub keyframes: Vec<Keyframe>,

    /// Interpolation method between keyframes
    pub interpolation: Interpolation,
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

impl Animation {
    /// Create empty animation
    pub fn new(name: String, duration: f64) -> Self {
        Self {
            name,
            duration,
            tracks: HashMap::new(),
            loop_mode: LoopMode::Once,
        }
    }

    /// Add track for parameter
    pub fn add_track(&mut self, path: ConfigPath, track: Track) {
        // Convert ConfigPath to string for JSON serialization
        let path_str = format!("{:?}", path); // TODO: Better serialization
        self.tracks.insert(path_str, track);
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
    /// Create track with single keyframe
    pub fn constant(value: serde_json::Value) -> Self {
        Self {
            keyframes: vec![Keyframe {
                time: 0.0,
                value,
                easing: EasingFunction::Linear,
            }],
            interpolation: Interpolation::Linear,
        }
    }

    /// Create track with two keyframes (start → end)
    pub fn linear(start_value: serde_json::Value, end_value: serde_json::Value, duration: f64) -> Self {
        Self {
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
            interpolation: Interpolation::Linear,
        }
    }

    /// Add keyframe in time-sorted order
    pub fn add_keyframe(&mut self, keyframe: Keyframe) {
        self.keyframes.push(keyframe);
        self.keyframes.sort_by(|a, b| a.time.partial_cmp(&b.time).unwrap());
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
}
