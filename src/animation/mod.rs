//! Animation system for automated parameter changes over time
//!
//! This module provides keyframe-based animation with track interpolation,
//! During playback, the animation controller updates ConfigManager silently
//! (without creating undo points).

use crate::config::{EditingTarget, FractalConfig};
use serde::{Deserialize, Deserializer, Serialize};

mod controller;
mod interpolation;
pub mod export;

pub use controller::AnimationController;
pub use interpolation::{EasingFunction, Interpolation};
pub use export::AudioExportConfig;

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

    /// Signal generator configs (procedural waveforms)
    /// Regenerated into SignalManager on load; no binary data in .anim files.
    #[serde(default)]
    pub generators: Vec<crate::signal::generator::GeneratorConfig>,

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
    #[serde(default)]
    flame_target: EditingTarget,
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
                    flame_target: legacy.flame_target,
                    source: legacy.source,
                    interpolation: legacy.interpolation,
                    bound: TrackBinding::default(),
                });
            }
            Ok(tracks)
        }
    }

    deserializer.deserialize_any(TracksVisitor)
}

/// Identifies which list-item a track is bound to (post-rebind).
///
/// Runtime-only. Set by `Animation::bind_to_config` after load and
/// kept current by the rebind hook on every structural change to the
/// referenced list. The hook uses these IDs to find where the item
/// *now* lives in its pool, then rewrites the index inside
/// `Track.target` to match. Apply-path code never reads this — it
/// always parses `Track.target` and indexes directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TargetBinding {
    /// References `flame.transforms[index]` of the bound flame.
    Transform(u64),
    /// References `flame.linked_transforms[index]` of the bound flame.
    Linked(u64),
    /// References `flame.final_transforms[index]` of the bound flame.
    Final(u64),
    /// References `config.color_effects[index]` (FractalConfig-level,
    /// not per-flame).
    ColorEffect(u64),
    /// References `config.density_effects[index]` (FractalConfig-level).
    DensityEffect(u64),
}

/// Full binding state for a Track. Runtime-only; never serialized.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TrackBinding {
    /// The subflame this track's `flame_target` refers to, by id.
    /// `None` when `flame_target == Main` (or for non-flame paths).
    pub flame: Option<u64>,
    /// The list-item the track's `target` resolves to. `None` for
    /// non-list paths (View, Color, Tone, Rendering — anything not
    /// indexed into one of the six tracked lists).
    pub target: Option<TargetBinding>,
}

/// Single parameter track
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Track {
    /// Target parameter path (ConfigPath string key)
    pub target: String,

    /// Which flame the parameter lives on. `Main` is the main flame;
    /// `Subflame { index }` references a subflame by its position in
    /// the flame's `subflames` list. Defaults to `Main` so older
    /// `.anim` files (predating subflame support) deserialize against
    /// the main flame exactly as they used to.
    #[serde(default)]
    pub flame_target: EditingTarget,

    /// Source of track values (keyframes or procedural)
    pub source: TrackSource,

    /// Interpolation method (for keyframe tracks)
    #[serde(default)]
    pub interpolation: Interpolation,

    /// Runtime-only handle to whichever list-items this track
    /// targets. Populated by `Animation::bind_to_config` after load
    /// and kept current by the rebind hook on every add/delete/reorder
    /// of a tracked list. Never serialized.
    ///
    /// `target` is always the source of truth for *where to apply*; the
    /// rebind hook reads `bound` to look up where each tracked item
    /// is *now* and rewrites the index inside `target` to match.
    #[serde(skip)]
    pub bound: TrackBinding,
}

/// Source of track values
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TrackSource {
    /// Traditional keyframe animation
    Keyframes {
        keyframes: Vec<Keyframe>,
    },

    /// Signal-driven track (audio analysis, generators, MIDI, sensors, etc.)
    Signal {
        /// Name of the signal to read from SignalManager
        signal_name: String,
        /// Value when signal is at minimum (0.0)
        min_output: f64,
        /// Value when signal is at maximum (1.0)
        max_output: f64,
        /// Optional smoothing factor (0.0 = none, 1.0 = heavy)
        #[serde(default)]
        smoothing: f64,
        /// Start time in seconds (0.0 = start of animation)
        #[serde(default)]
        start_time: f64,
        /// End time in seconds (0.0 = use animation duration)
        #[serde(default)]
        end_time: f64,
        /// Fade in duration in seconds
        #[serde(default)]
        fade_in: f64,
        /// Fade in easing curve
        #[serde(default)]
        fade_in_easing: EasingFunction,
        /// Fade out duration in seconds
        #[serde(default)]
        fade_out: f64,
        /// Fade out easing curve
        #[serde(default)]
        fade_out_easing: EasingFunction,
    },
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
            generators: Vec::new(),
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
            generators: Vec::new(),
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

    /// Remove a track by index
    pub fn remove_track(&mut self, index: usize) -> Option<Track> {
        if index < self.tracks.len() {
            Some(self.tracks.remove(index))
        } else {
            None
        }
    }

    // Note: `on_transform_removed` / `on_color_effect_removed` /
    // `on_color_effect_reordered` / `on_density_effect_removed` /
    // `on_density_effect_reordered` were removed in the PR 2 stable-
    // identity overhaul. Their job — string-prefix shifting of indices
    // in `Track.target` — is now done by `rebind_targets`, which uses
    // `Track.bound` IDs to find where each tracked item moved to.
    //
    // Deletion semantics also changed: the old helpers *removed*
    // tracks pointing at the deleted item. The new model keeps the
    // track and marks it broken so it can be auto-restored if the
    // deletion is undone.

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

    /// Resolve every track's `(flame_target, target)` against the given
    /// config and populate `track.bound` with the IDs of the items each
    /// track currently points at. Tracks whose target is non-list
    /// (View, Color, etc.) get `bound = Default::default()`. Tracks
    /// whose flame_target or target index doesn't resolve get
    /// `bound = Default::default()` too — they'll be marked broken in
    /// the UI and won't be auto-rebound until the user picks a new
    /// target.
    ///
    /// Call this once after `Animation::from_json` (or after loading
    /// the embedded `base_config`) so the rebind hook has IDs to
    /// follow during the session. Safe to re-run.
    pub fn bind_to_config(&mut self, config: &FractalConfig) {
        for track in &mut self.tracks {
            track.bound = resolve_track_binding(config, track.flame_target, &track.target);
        }
    }

    /// After a structural change to one of the six tracked lists,
    /// walk every track and rewrite its `target` / `flame_target`
    /// indices to keep pointing at the same item the `bound` IDs name.
    ///
    /// - If `bound.flame` is set, find the subflame with that id and
    ///   rewrite `flame_target = Subflame{new_index}`.
    /// - If `bound.target` is set, find the item with that id in the
    ///   relevant pool of the bound flame and rewrite the index inside
    ///   `target`.
    /// - If either lookup fails, leave the path as-is. The track is
    ///   "broken" — `bound` stays set so a future undo/restore that
    ///   brings the item back will re-resolve and un-break it.
    ///
    /// Called from `ConfigManager` after every add / delete / reorder
    /// of a tracked list, and after undo/redo snapshot restores.
    pub fn rebind_targets(&mut self, config: &FractalConfig) {
        for track in &mut self.tracks {
            rebind_one(track, config);
        }
    }
}

/// Resolve a `(flame_target, target_path)` pair to the runtime
/// binding it should carry. Returns `Default` when:
/// - the path doesn't reference any of the six tracked lists
///   (so there's nothing to bind to — e.g. `Zoom`)
/// - or the path *does* reference a list but the index / flame is
///   out of range (track is broken; binding stays empty until the
///   user re-picks a target).
pub fn resolve_track_binding(
    config: &FractalConfig,
    flame_target: EditingTarget,
    target_path: &str,
) -> TrackBinding {
    use crate::config::ConfigPath;

    // Resolve the subflame ID, if any. Out-of-range subflame index →
    // bound is left empty.
    let (flame, flame_id) = match flame_target {
        EditingTarget::Main => (&config.flame, None),
        EditingTarget::Subflame { index } => match config.flame.subflames.get(index) {
            Some(sub) => (sub, Some(sub.id)),
            None => return TrackBinding::default(),
        },
    };

    let Some(path) = ConfigPath::from_string_key(target_path) else {
        return TrackBinding { flame: flame_id, target: None };
    };

    // Map ConfigPath → which list it indexes into. Paths that don't
    // index a tracked list (View / Color / Tone / Rendering / etc.)
    // return None.
    let target = match &path {
        // Normal pool
        ConfigPath::TransformWeight { index }
        | ConfigPath::TransformColor { index }
        | ConfigPath::TransformColorSpeed { index }
        | ConfigPath::TransformOpacity { index }
        | ConfigPath::TransformDirectColor { index }
        | ConfigPath::TransformAffine { index, .. }
        | ConfigPath::TransformPostAffineEnabled { index }
        | ConfigPath::TransformPostAffine { index, .. }
        | ConfigPath::TransformOriginX { index }
        | ConfigPath::TransformOriginY { index }
        | ConfigPath::TransformRotation { index }
        | ConfigPath::TransformScale { index }
        | ConfigPath::TransformPostAffineOriginX { index }
        | ConfigPath::TransformPostAffineOriginY { index }
        | ConfigPath::TransformPostAffineRotation { index }
        | ConfigPath::TransformPostAffineScale { index }
        | ConfigPath::TransformVariation { index, .. }
        | ConfigPath::TransformVariationParam { index, .. } => {
            flame.transforms.get(*index).map(|t| TargetBinding::Transform(t.id))
        }
        // Linked pool
        ConfigPath::LinkedTransformAffine { index, .. }
        | ConfigPath::LinkedTransformPostAffineEnabled { index }
        | ConfigPath::LinkedTransformPostAffine { index, .. }
        | ConfigPath::LinkedTransformOriginX { index }
        | ConfigPath::LinkedTransformOriginY { index }
        | ConfigPath::LinkedTransformRotation { index }
        | ConfigPath::LinkedTransformScale { index }
        | ConfigPath::LinkedTransformPostAffineOriginX { index }
        | ConfigPath::LinkedTransformPostAffineOriginY { index }
        | ConfigPath::LinkedTransformPostAffineRotation { index }
        | ConfigPath::LinkedTransformPostAffineScale { index }
        | ConfigPath::LinkedTransformVariation { index, .. }
        | ConfigPath::LinkedTransformVariationParam { index, .. } => {
            flame.linked_transforms.get(*index).map(|t| TargetBinding::Linked(t.id))
        }
        // Final pool
        ConfigPath::FinalTransformAffine { index, .. }
        | ConfigPath::FinalTransformPostAffineEnabled { index }
        | ConfigPath::FinalTransformPostAffine { index, .. }
        | ConfigPath::FinalTransformOriginX { index }
        | ConfigPath::FinalTransformOriginY { index }
        | ConfigPath::FinalTransformRotation { index }
        | ConfigPath::FinalTransformScale { index }
        | ConfigPath::FinalTransformPostAffineOriginX { index }
        | ConfigPath::FinalTransformPostAffineOriginY { index }
        | ConfigPath::FinalTransformPostAffineRotation { index }
        | ConfigPath::FinalTransformPostAffineScale { index }
        | ConfigPath::FinalTransformVariation { index, .. }
        | ConfigPath::FinalTransformVariationParam { index, .. } => {
            flame.final_transforms.get(*index).map(|t| TargetBinding::Final(t.id))
        }
        // Effects (FractalConfig-level, not per-flame)
        ConfigPath::ColorEffectEnabled { index }
        | ConfigPath::ColorEffectParam { index, .. } => {
            config.color_effects.get(*index).map(|e| TargetBinding::ColorEffect(e.id))
        }
        ConfigPath::DensityEffectEnabled { index }
        | ConfigPath::DensityEffectParam { index, .. } => {
            config.density_effects.get(*index).map(|e| TargetBinding::DensityEffect(e.id))
        }
        // Everything else (View, Color, Tone, Rendering, Xaos,
        // SoloTransform, …) doesn't index a tracked list.
        _ => None,
    };

    TrackBinding { flame: flame_id, target }
}

/// Rewrite one track's index references to match where its `bound`
/// IDs currently live in the config. See `Animation::rebind_targets`
/// for the full contract.
fn rebind_one(track: &mut Track, config: &FractalConfig) {
    // Step 1: rebind flame_target. If the track is bound to a
    // subflame ID, find its current position; if the ID is gone,
    // leave flame_target as-is (broken).
    if let Some(flame_id) = track.bound.flame {
        if let Some(new_index) = config
            .flame
            .subflames
            .iter()
            .position(|sub| sub.id == flame_id)
        {
            track.flame_target = EditingTarget::Subflame { index: new_index };
        }
    }

    // Step 2: rebind the target path's list index. Need to look up
    // the bound item in the right pool *of the bound flame*. If the
    // flame itself doesn't resolve, skip — track is broken.
    let Some(target_binding) = track.bound.target else { return };
    let flame = match track.flame_target {
        EditingTarget::Main => &config.flame,
        EditingTarget::Subflame { index } => match config.flame.subflames.get(index) {
            Some(sub) => sub,
            None => return, // broken — leave target as-is
        },
    };

    let new_index = match target_binding {
        TargetBinding::Transform(id) => flame.transforms.iter().position(|t| t.id == id),
        TargetBinding::Linked(id) => flame.linked_transforms.iter().position(|t| t.id == id),
        TargetBinding::Final(id) => flame.final_transforms.iter().position(|t| t.id == id),
        TargetBinding::ColorEffect(id) => config.color_effects.iter().position(|e| e.id == id),
        TargetBinding::DensityEffect(id) => config.density_effects.iter().position(|e| e.id == id),
    };

    if let Some(new_index) = new_index {
        if let Some(rewritten) = rewrite_path_index(&track.target, new_index) {
            track.target = rewritten;
        }
    }
}

/// Replace the index segment of a `{Prefix}.{index}.{rest...}` path
/// with a new index. Returns `None` if the path doesn't have that
/// shape (e.g. `Zoom`, `Pan`, `RenderMode` — none of which carry an
/// index to begin with). The function doesn't validate the prefix —
/// callers are expected to know the path is one of the indexed
/// transform / effect families.
fn rewrite_path_index(path: &str, new_index: usize) -> Option<String> {
    let dot1 = path.find('.')?;
    let after_prefix = &path[dot1 + 1..];
    let dot2_rel = after_prefix.find('.')?;
    let prefix = &path[..dot1];
    let rest = &after_prefix[dot2_rel + 1..];
    Some(format!("{}.{}.{}", prefix, new_index, rest))
}

impl Track {
    /// Create a new track with the given target and source.
    /// Defaults to `EditingTarget::Main` — use `with_flame_target` to
    /// retarget at a subflame.
    pub fn new(target: String, source: TrackSource) -> Self {
        Self {
            target,
            flame_target: EditingTarget::Main,
            source,
            interpolation: Interpolation::Linear,
            bound: TrackBinding::default(),
        }
    }

    /// Builder: set the flame this track targets. Use this for tracks
    /// that animate a subflame parameter:
    ///
    /// ```ignore
    /// let track = Track::new(path, source)
    ///     .with_flame_target(EditingTarget::Subflame { index: 0 });
    /// ```
    pub fn with_flame_target(mut self, flame_target: EditingTarget) -> Self {
        self.flame_target = flame_target;
        self
    }

    /// Create keyframe track with single keyframe (constant value)
    pub fn constant(target: String, value: serde_json::Value) -> Self {
        Self::new(
            target,
            TrackSource::Keyframes {
                keyframes: vec![Keyframe {
                    time: 0.0,
                    value,
                    easing: EasingFunction::Linear,
                }],
            },
        )
    }

    /// Create keyframe track with two keyframes (start → end)
    pub fn linear(target: String, start_value: serde_json::Value, end_value: serde_json::Value, duration: f64) -> Self {
        Self::new(
            target,
            TrackSource::Keyframes {
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
        )
    }

    /// Create signal-driven track
    ///
    /// Maps signal values (normalized 0-1) to output range [min_output, max_output]
    pub fn signal(target: String, signal_name: String, min_output: f64, max_output: f64) -> Self {
        Self::new(
            target,
            TrackSource::Signal {
                signal_name,
                min_output,
                max_output,
                smoothing: 0.0,
                start_time: 0.0,
                end_time: 0.0,
                fade_in: 0.0,
                fade_in_easing: EasingFunction::Linear,
                fade_out: 0.0,
                fade_out_easing: EasingFunction::Linear,
            },
        )
    }

    /// Create signal-driven track with smoothing
    pub fn signal_with_smoothing(
        target: String,
        signal_name: String,
        min_output: f64,
        max_output: f64,
        smoothing: f64,
    ) -> Self {
        Self::new(
            target,
            TrackSource::Signal {
                signal_name,
                min_output,
                max_output,
                smoothing,
                start_time: 0.0,
                end_time: 0.0,
                fade_in: 0.0,
                fade_in_easing: EasingFunction::Linear,
                fade_out: 0.0,
                fade_out_easing: EasingFunction::Linear,
            },
        )
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
    fn test_full_animation_json() {
        // Test a complete animation with multiple track types
        let mut anim = Animation::new("Full Test".into(), 10.0);
        anim.loop_mode = LoopMode::PingPong;

        // Keyframe track
        anim.add_track(Track::linear(
            "Zoom".into(),
            serde_json::json!(1.0),
            serde_json::json!(5.0),
            10.0,
        ));

        // Signal track
        anim.add_track(Track::signal(
            "Brightness".into(),
            "energy".into(),
            0.5,
            2.0,
        ));

        let json = anim.to_json().unwrap();
        println!("Animation JSON:\n{}", json);

        let loaded = Animation::from_json(&json).unwrap();
        assert_eq!(loaded.name, "Full Test");
        assert_eq!(loaded.duration, 10.0);
        assert_eq!(loaded.loop_mode, LoopMode::PingPong);
        assert_eq!(loaded.tracks.len(), 2);
    }
}
