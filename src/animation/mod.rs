//! Animation system for automated parameter changes over time
//!
//! This module provides keyframe-based animation with track interpolation,
//! as well as procedural track types (oscillators, circular motion).
//! During playback, the animation controller updates ConfigManager silently
//! (without creating undo points).

use crate::config::FractalConfig;
use serde::{Deserialize, Deserializer, Serialize};

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

    /// Embedded fractal configuration (makes animation self-contained and reproducible)
    /// When present, loading this animation also loads the base config.
    /// When absent, animation applies to whatever fractal is currently loaded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_config: Option<FractalConfig>,

    /// Total duration in seconds
    pub duration: f64,

    /// Parameter tracks (indexed by position, allowing multiple tracks with same target)
    /// Supports loading from both old HashMap format and new Vec format for backwards compatibility
    #[serde(deserialize_with = "deserialize_tracks")]
    pub tracks: Vec<Track>,

    /// Circular motion tracks (output X and Y to two parameters)
    #[serde(default)]
    pub circular_tracks: Vec<CircularTrack>,

    /// Looping behavior
    pub loop_mode: LoopMode,
}

/// Legacy track format (for backwards compatibility)
/// Old format stored tracks as HashMap with target as key
#[derive(Debug, Clone, Deserialize)]
struct LegacyTrack {
    source: TrackSource,
    #[serde(default)]
    interpolation: Interpolation,
}

/// Deserialize tracks from either Vec<Track> (new) or HashMap<String, LegacyTrack> (old)
fn deserialize_tracks<'de, D>(deserializer: D) -> Result<Vec<Track>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::{SeqAccess, MapAccess, Visitor};
    use std::fmt;

    struct TracksVisitor;

    impl<'de> Visitor<'de> for TracksVisitor {
        type Value = Vec<Track>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a sequence of tracks or a map of target -> track")
        }

        // New format: Vec<Track>
        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut tracks = Vec::new();
            while let Some(track) = seq.next_element::<Track>()? {
                tracks.push(track);
            }
            Ok(tracks)
        }

        // Old format: HashMap<String, LegacyTrack>
        fn visit_map<M>(self, mut map: M) -> Result<Self::Value, M::Error>
        where
            M: MapAccess<'de>,
        {
            let mut tracks = Vec::new();
            while let Some((target, legacy)) = map.next_entry::<String, LegacyTrack>()? {
                tracks.push(Track {
                    target,
                    source: legacy.source,
                    interpolation: legacy.interpolation,
                });
            }
            Ok(tracks)
        }
    }

    deserializer.deserialize_any(TracksVisitor)
}

/// Single parameter track
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    /// Target parameter path (ConfigPath string key)
    pub target: String,

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

impl Animation {
    /// Create empty animation (without embedded config)
    pub fn new(name: String, duration: f64) -> Self {
        Self {
            name,
            base_config: None,
            duration,
            tracks: Vec::new(),
            circular_tracks: Vec::new(),
            loop_mode: LoopMode::Once,
        }
    }

    /// Create animation with embedded fractal config (for reproducibility)
    pub fn with_config(name: String, duration: f64, config: FractalConfig) -> Self {
        Self {
            name,
            base_config: Some(config),
            duration,
            tracks: Vec::new(),
            circular_tracks: Vec::new(),
            loop_mode: LoopMode::Once,
        }
    }

    /// Set the base config (captures current fractal state)
    pub fn set_base_config(&mut self, config: FractalConfig) {
        self.base_config = Some(config);
    }

    /// Check if this animation has an embedded config
    pub fn has_base_config(&self) -> bool {
        self.base_config.is_some()
    }

    /// Add track (returns index of the new track)
    pub fn add_track(&mut self, track: Track) -> usize {
        let index = self.tracks.len();
        self.tracks.push(track);
        index
    }

    /// Add circular motion track (returns index of the new track)
    pub fn add_circular_track(&mut self, track: CircularTrack) -> usize {
        let index = self.circular_tracks.len();
        self.circular_tracks.push(track);
        index
    }

    /// Remove a track by index
    pub fn remove_track(&mut self, index: usize) -> Option<Track> {
        if index < self.tracks.len() {
            Some(self.tracks.remove(index))
        } else {
            None
        }
    }

    /// Remove a circular track by index
    pub fn remove_circular_track(&mut self, index: usize) -> Option<CircularTrack> {
        if index < self.circular_tracks.len() {
            Some(self.circular_tracks.remove(index))
        } else {
            None
        }
    }

    /// Update animation tracks when a transform is removed.
    ///
    /// Removes all tracks targeting the deleted transform and decrements
    /// indices for tracks targeting higher transforms.
    ///
    /// Returns the number of tracks that were removed.
    pub fn on_transform_removed(&mut self, removed_index: usize) -> usize {
        let prefix = format!("Transform.{}.", removed_index);
        let initial_count = self.tracks.len();

        // Remove tracks targeting the deleted transform
        self.tracks.retain(|track| !track.target.starts_with(&prefix));

        // Decrement indices for tracks targeting higher transforms
        for track in &mut self.tracks {
            if let Some(new_target) = decrement_transform_index(&track.target, removed_index) {
                track.target = new_target;
            }
        }

        // Same for circular tracks
        self.circular_tracks.retain(|track| {
            !track.target_x.starts_with(&prefix) && !track.target_y.starts_with(&prefix)
        });
        for track in &mut self.circular_tracks {
            if let Some(new_target) = decrement_transform_index(&track.target_x, removed_index) {
                track.target_x = new_target;
            }
            if let Some(new_target) = decrement_transform_index(&track.target_y, removed_index) {
                track.target_y = new_target;
            }
        }

        initial_count - self.tracks.len()
    }

    /// Update animation tracks when a color effect is removed.
    ///
    /// Removes all tracks targeting the deleted effect and decrements
    /// indices for tracks targeting higher effects.
    ///
    /// Returns the number of tracks that were removed.
    pub fn on_color_effect_removed(&mut self, removed_index: usize) -> usize {
        let prefix = format!("ColorEffect.{}.", removed_index);
        let initial_count = self.tracks.len();

        // Remove tracks targeting the deleted effect
        self.tracks.retain(|track| !track.target.starts_with(&prefix));

        // Decrement indices for tracks targeting higher effects
        for track in &mut self.tracks {
            if let Some(new_target) = decrement_effect_index(&track.target, "ColorEffect", removed_index) {
                track.target = new_target;
            }
        }

        // Circular tracks typically don't target effects, but handle them for completeness
        self.circular_tracks.retain(|track| {
            !track.target_x.starts_with(&prefix) && !track.target_y.starts_with(&prefix)
        });
        for track in &mut self.circular_tracks {
            if let Some(new_target) = decrement_effect_index(&track.target_x, "ColorEffect", removed_index) {
                track.target_x = new_target;
            }
            if let Some(new_target) = decrement_effect_index(&track.target_y, "ColorEffect", removed_index) {
                track.target_y = new_target;
            }
        }

        initial_count - self.tracks.len()
    }

    /// Update animation tracks when a density effect is removed.
    ///
    /// Removes all tracks targeting the deleted effect and decrements
    /// indices for tracks targeting higher effects.
    ///
    /// Returns the number of tracks that were removed.
    pub fn on_density_effect_removed(&mut self, removed_index: usize) -> usize {
        let prefix = format!("DensityEffect.{}.", removed_index);
        let initial_count = self.tracks.len();

        // Remove tracks targeting the deleted effect
        self.tracks.retain(|track| !track.target.starts_with(&prefix));

        // Decrement indices for tracks targeting higher effects
        for track in &mut self.tracks {
            if let Some(new_target) = decrement_effect_index(&track.target, "DensityEffect", removed_index) {
                track.target = new_target;
            }
        }

        // Circular tracks typically don't target effects, but handle them for completeness
        self.circular_tracks.retain(|track| {
            !track.target_x.starts_with(&prefix) && !track.target_y.starts_with(&prefix)
        });
        for track in &mut self.circular_tracks {
            if let Some(new_target) = decrement_effect_index(&track.target_x, "DensityEffect", removed_index) {
                track.target_x = new_target;
            }
            if let Some(new_target) = decrement_effect_index(&track.target_y, "DensityEffect", removed_index) {
                track.target_y = new_target;
            }
        }

        initial_count - self.tracks.len()
    }

    /// Update animation tracks when color effects are reordered.
    ///
    /// Remaps effect indices based on a move from old_index to new_index.
    pub fn on_color_effect_reordered(&mut self, old_index: usize, new_index: usize) {
        for track in &mut self.tracks {
            if let Some(new_target) = remap_effect_index(&track.target, "ColorEffect", old_index, new_index) {
                track.target = new_target;
            }
        }
        for track in &mut self.circular_tracks {
            if let Some(new_target) = remap_effect_index(&track.target_x, "ColorEffect", old_index, new_index) {
                track.target_x = new_target;
            }
            if let Some(new_target) = remap_effect_index(&track.target_y, "ColorEffect", old_index, new_index) {
                track.target_y = new_target;
            }
        }
    }

    /// Update animation tracks when density effects are reordered.
    ///
    /// Remaps effect indices based on a move from old_index to new_index.
    pub fn on_density_effect_reordered(&mut self, old_index: usize, new_index: usize) {
        for track in &mut self.tracks {
            if let Some(new_target) = remap_effect_index(&track.target, "DensityEffect", old_index, new_index) {
                track.target = new_target;
            }
        }
        for track in &mut self.circular_tracks {
            if let Some(new_target) = remap_effect_index(&track.target_x, "DensityEffect", old_index, new_index) {
                track.target_x = new_target;
            }
            if let Some(new_target) = remap_effect_index(&track.target_y, "DensityEffect", old_index, new_index) {
                track.target_y = new_target;
            }
        }
    }

    /// Get track by index
    pub fn get_track(&self, index: usize) -> Option<&Track> {
        self.tracks.get(index)
    }

    /// Get mutable track by index
    pub fn get_track_mut(&mut self, index: usize) -> Option<&mut Track> {
        self.tracks.get_mut(index)
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
    /// Create a new track with the given target and source
    pub fn new(target: String, source: TrackSource) -> Self {
        Self {
            target,
            source,
            interpolation: Interpolation::Linear,
        }
    }

    /// Create keyframe track with single keyframe (constant value)
    pub fn constant(target: String, value: serde_json::Value) -> Self {
        Self {
            target,
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

    /// Create keyframe track with two keyframes (start → end)
    pub fn linear(target: String, start_value: serde_json::Value, end_value: serde_json::Value, duration: f64) -> Self {
        Self {
            target,
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
        target: String,
        oscillator_type: OscillatorType,
        center: f64,
        amplitude: f64,
        frequency: f64,
    ) -> Self {
        Self {
            target,
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
        target: String,
        oscillator_type: OscillatorType,
        center: f64,
        amplitude: f64,
        frequency: f64,
        phase: f64,
    ) -> Self {
        Self {
            target,
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

/// Helper to decrement transform index in a path string if it's higher than removed_index.
///
/// Path format: "Transform.{N}.{field}..."
fn decrement_transform_index(path: &str, removed_index: usize) -> Option<String> {
    if let Some(rest) = path.strip_prefix("Transform.") {
        if let Some(dot_pos) = rest.find('.') {
            if let Ok(index) = rest[..dot_pos].parse::<usize>() {
                if index > removed_index {
                    return Some(format!("Transform.{}.{}", index - 1, &rest[dot_pos + 1..]));
                }
            }
        }
    }
    None
}

/// Helper to decrement effect index in a path string if it's higher than removed_index.
///
/// Path format: "{prefix}.{N}.{field}" where prefix is "ColorEffect" or "DensityEffect"
fn decrement_effect_index(path: &str, prefix: &str, removed_index: usize) -> Option<String> {
    let full_prefix = format!("{}.", prefix);
    if let Some(rest) = path.strip_prefix(&full_prefix) {
        if let Some(dot_pos) = rest.find('.') {
            if let Ok(index) = rest[..dot_pos].parse::<usize>() {
                if index > removed_index {
                    return Some(format!("{}.{}.{}", prefix, index - 1, &rest[dot_pos + 1..]));
                }
            }
        }
    }
    None
}

/// Helper to remap effect index when effects are reordered.
///
/// When an effect moves from old_index to new_index:
/// - The moved effect: old_index -> new_index
/// - Effects between shift up or down depending on direction
fn remap_effect_index(path: &str, prefix: &str, old_index: usize, new_index: usize) -> Option<String> {
    let full_prefix = format!("{}.", prefix);
    if let Some(rest) = path.strip_prefix(&full_prefix) {
        if let Some(dot_pos) = rest.find('.') {
            if let Ok(index) = rest[..dot_pos].parse::<usize>() {
                let new_idx = if index == old_index {
                    // This is the moved effect
                    new_index
                } else if old_index < new_index {
                    // Moving down: effects in (old_index, new_index] shift up by 1
                    if index > old_index && index <= new_index {
                        index - 1
                    } else {
                        return None; // No change needed
                    }
                } else {
                    // Moving up: effects in [new_index, old_index) shift down by 1
                    if index >= new_index && index < old_index {
                        index + 1
                    } else {
                        return None; // No change needed
                    }
                };
                return Some(format!("{}.{}.{}", prefix, new_idx, &rest[dot_pos + 1..]));
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_animation_json_roundtrip() {
        let mut anim = Animation::new("Test".into(), 10.0);
        anim.loop_mode = LoopMode::Loop;

        let track = Track::linear(
            "Zoom".into(),
            serde_json::json!(1.0),
            serde_json::json!(10.0),
            10.0,
        );
        anim.add_track(track);

        let json = anim.to_json().unwrap();
        let loaded = Animation::from_json(&json).unwrap();

        assert_eq!(loaded.name, "Test");
        assert_eq!(loaded.duration, 10.0);
        assert_eq!(loaded.loop_mode, LoopMode::Loop);
        assert_eq!(loaded.tracks.len(), 1);
        assert_eq!(loaded.tracks[0].target, "Zoom");
    }

    #[test]
    fn test_oscillator_track_json() {
        let mut anim = Animation::new("Oscillate".into(), 5.0);

        let track = Track::oscillator("Exposure".into(), OscillatorType::Sine, 1.0, 0.5, 2.0);
        anim.add_track(track);

        let json = anim.to_json().unwrap();
        let loaded = Animation::from_json(&json).unwrap();

        assert_eq!(loaded.tracks.len(), 1);
        let track = &loaded.tracks[0];
        assert_eq!(track.target, "Exposure");
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
        anim.add_track(Track::linear(
            "Zoom".into(),
            serde_json::json!(1.0),
            serde_json::json!(5.0),
            10.0,
        ));

        // Oscillator track
        anim.add_track(Track::oscillator(
            "Brightness".into(),
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
