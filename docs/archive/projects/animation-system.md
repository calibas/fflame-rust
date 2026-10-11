# Animation System - Design Document

**Date:** 2025-11-13 (Updated: 2025-11-25)
**Status:** Ready for Implementation
**Priority:** Active Development

---

## Overview

Add keyframe-based animation system that takes control of FractalConfig during playback, allowing automated parameter changes over time. Supports multiple interpolation types including linear, eased, sinusoidal, and circular motion.

## Three Render States

### 1. Normal Mode (Current Default)
- **Accumulation:** Progressive multi-frame refinement
- **Config Control:** UI → ConfigManager → GPU
- **Overwrite:** `false`
- **Undo:** Creates undo points
- **User Actions:** Full UI control, all edits allowed

### 2. Preview Mode (Current Live Updates)
- **Accumulation:** Single-frame overwrite
- **Config Control:** UI → ConfigManager → GPU
- **Overwrite:** `true`
- **Undo:** Creates undo points (coalesced)
- **User Actions:** Active drag/interaction on specific controls
- **Scope:** Per-control opt-in (via `lazy=response.dragged()`)
- **Duration:** Temporary during interaction

### 3. Animation Mode (NEW)
- **Accumulation:** Single-frame overwrite
- **Config Control:** AnimationController → ConfigManager (silent) → GPU
- **Overwrite:** `true`
- **Undo:** NO undo points created
- **User Actions:** Large UI sections disabled during playback
- **Frame Rate:** Match current app frame rate (60 FPS default)

**Key Difference:** Animation mode is separate from preview mode. Both use overwrite accumulation, but:
- Preview mode: User is actively editing, undo points created
- Animation mode: Automated playback, no undo points, UI locked

## Design Decisions

### ConfigManager Integration

**Decision:** Add `update_param_silent()` for animation playback

**Implementation:**
```rust
impl ConfigManager {
    /// Update parameter without creating undo point
    /// Used by animation system during playback
    pub fn update_param_silent(&mut self, path: ConfigPath, value: ConfigValue) -> Result<UpdateType, ConfigError> {
        // Skip undo stack entirely
        let old_value = self.get_value(&path)?;
        if old_value.approx_eq(&value) {
            return Ok(UpdateType::None);
        }

        self.set_value(&path, value)?;
        let update_type = path.update_type();
        self.record_action(update_type);
        Ok(update_type)
    }
}
```

**Rationale:**
- Reuses existing `set_value()` and `get_value()` infrastructure
- Returns `UpdateType` for proper GPU synchronization
- Simple addition (~10 lines) to existing ConfigManager
- All ConfigPath variants already exist for transforms and parameters

### Animation Mode Flag

**Decision:** Add explicit `animation_playing` flag to App, separate from preview mode

```rust
pub struct App {
    // ... existing fields ...

    /// Animation controller (owns playback state)
    animation_controller: AnimationController,
}

// In render loop:
let animation_playing = self.animation_controller.state == PlaybackState::Playing;

// Overwrite mode triggered by EITHER preview OR animation
let use_overwrite = preview_active || animation_playing;
```

### Track Types

**Decision:** Support both keyframe-based and procedural tracks

```rust
/// Source of track values
pub enum TrackSource {
    /// Traditional keyframe animation
    Keyframes(Vec<Keyframe>),

    /// Sinusoidal oscillation (no keyframes needed)
    Oscillator {
        oscillator_type: OscillatorType,
        center: f64,      // Center value
        amplitude: f64,   // Peak deviation from center
        frequency: f64,   // Cycles per second
        phase: f64,       // Starting phase (0-1)
    },

    /// Circular motion (outputs to TWO parameters)
    Circular {
        center_x: f64,
        center_y: f64,
        radius: f64,
        speed: f64,       // Revolutions per second
        phase: f64,       // Starting angle (radians)
    },
}

pub enum OscillatorType {
    Sine,       // Smooth sine wave
    Triangle,   // Linear up/down
    Sawtooth,   // Linear ramp, instant reset
    Square,     // Instant flip between values
}
```

**Circular Track Special Case:**
Circular tracks output two values (X and Y). They're defined once but linked to two ConfigPaths:
```json
{
  "type": "Circular",
  "target_x": "PanX",
  "target_y": "PanY",
  "center_x": 0.0,
  "center_y": 0.0,
  "radius": 0.5,
  "speed": 0.1,
  "phase": 0.0
}
```

### Animation Data Structure

```rust
pub struct Animation {
    pub name: String,
    pub duration: f64,                              // Total length (seconds)
    pub tracks: HashMap<String, Track>,             // Parameter tracks (String key for JSON)
    pub circular_tracks: Vec<CircularTrack>,        // Special 2D circular motion
    pub loop_mode: LoopMode,
}

pub struct Track {
    pub source: TrackSource,
    pub interpolation: Interpolation,               // For keyframe tracks
}

pub struct Keyframe {
    pub time: f64,                                  // Seconds from start
    pub value: serde_json::Value,                   // Parameter value
    pub easing: EasingFunction,                     // Easing to NEXT keyframe
}

pub struct CircularTrack {
    pub target_x: String,                           // ConfigPath for X (e.g., "Pan" or "TransformAffine{0,E}")
    pub target_y: String,                           // ConfigPath for Y
    pub center_x: f64,
    pub center_y: f64,
    pub radius: f64,
    pub speed: f64,                                 // Revolutions per second
    pub phase: f64,                                 // Starting angle (radians)
}

pub enum LoopMode {
    Once,           // Stop at end
    Loop,           // Restart from beginning
    PingPong,       // Reverse direction at ends
}

pub enum Interpolation {
    Step,           // Jump to value (no interpolation)
    Linear,         // Linear interpolation
    Smooth,         // Cubic/Catmull-Rom spline
    Sinusoidal,     // Sine-based interpolation (smooth oscillation between keyframes)
}

pub enum EasingFunction {
    Linear,
    EaseIn,         // Quadratic ease in
    EaseOut,        // Quadratic ease out
    EaseInOut,      // Quadratic ease in/out
    EaseInCubic,
    EaseOutCubic,
    EaseInOutCubic,
    // Sine-based (smoother than quadratic)
    EaseInSine,
    EaseOutSine,
    EaseInOutSine,
}
```

### Animation Controller

```rust
pub struct AnimationController {
    pub animation: Option<Animation>,
    pub state: PlaybackState,
    pub current_time: f64,
    pub speed: f64,                     // Playback speed multiplier
    direction: f32,                     // 1.0 or -1.0 (for ping-pong)
}

pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}

impl AnimationController {
    /// Evaluate all tracks at current time
    /// Returns values to apply (including both X and Y from circular tracks)
    pub fn evaluate_frame(&self) -> Vec<(String, serde_json::Value)> {
        let mut values = Vec::new();

        if let Some(ref anim) = self.animation {
            // Regular tracks
            for (path, track) in &anim.tracks {
                if let Some(value) = self.evaluate_track(track, self.current_time) {
                    values.push((path.clone(), value));
                }
            }

            // Circular tracks (output X and Y)
            for circular in &anim.circular_tracks {
                let angle = self.current_time * circular.speed * 2.0 * std::f64::consts::PI + circular.phase;
                let x = circular.center_x + circular.radius * angle.cos();
                let y = circular.center_y + circular.radius * angle.sin();
                values.push((circular.target_x.clone(), serde_json::json!(x)));
                values.push((circular.target_y.clone(), serde_json::json!(y)));
            }
        }

        values
    }

    fn evaluate_track(&self, track: &Track, time: f64) -> Option<serde_json::Value> {
        match &track.source {
            TrackSource::Keyframes(keyframes) => {
                self.interpolate_keyframes(keyframes, time, track.interpolation)
            }
            TrackSource::Oscillator { oscillator_type, center, amplitude, frequency, phase } => {
                let t = time * frequency + phase;
                let wave = match oscillator_type {
                    OscillatorType::Sine => (t * 2.0 * std::f64::consts::PI).sin(),
                    OscillatorType::Triangle => 1.0 - 4.0 * (t - (t + 0.5).floor()).abs(),
                    OscillatorType::Sawtooth => 2.0 * (t - t.floor()) - 1.0,
                    OscillatorType::Square => if t.fract() < 0.5 { 1.0 } else { -1.0 },
                };
                Some(serde_json::json!(center + amplitude * wave))
            }
            TrackSource::Circular { .. } => {
                // Handled separately in evaluate_frame() since it outputs two values
                None
            }
        }
    }
}
```

### Frame Order Integration

```rust
// In App::render()

// Phase 1: UI
let ui_response = self.egui_layer.render_ui(...);

// Phase 2: Process Updates
let animation_playing = self.animation_controller.state == PlaybackState::Playing;

if animation_playing {
    // Animation takes control - update time and evaluate
    self.animation_controller.update(delta_time);
    let frame_values = self.animation_controller.evaluate_frame();

    for (path_str, value) in frame_values {
        // Convert string path to ConfigPath (need helper function)
        if let Ok(path) = ConfigPath::from_str(&path_str) {
            if let Ok(config_value) = json_to_config_value(&value, &path) {
                // Silent update - no undo point
                let _ = self.config_manager.update_param_silent(path, config_value);
            }
        }
    }
} else {
    // Normal UI updates (existing code)
    // ... handle ui_response ...
}

// Determine overwrite mode
let use_overwrite = preview_active || animation_playing;

// Phase 3: Render
// ... compute/accumulate/tonemap with use_overwrite flag ...
```

### UI Behavior During Animation

**Disabled During Playback:**
- Settings panel (most controls)
- Transforms panel (add/delete/edit)
- Triangle editor
- Palette editor
- View controls (if those parameters are animated)
- Any control for an animated parameter

**Enabled During Playback:**
- Timeline scrubbing (pauses automatically)
- Play/Pause/Stop buttons
- Playback speed control
- Non-animated parameters (if we allow partial editing)

**Implementation:**
```rust
let animation_playing = animation_controller.state == PlaybackState::Playing;

// Option 1: Lock everything
ui.add_enabled_ui(!animation_playing, |ui| {
    // All parameter controls
});

// Option 2: Lock only animated parameters (more complex)
let animated_paths: HashSet<String> = animation_controller
    .animation.as_ref()
    .map(|a| a.tracks.keys().cloned().collect())
    .unwrap_or_default();

// Then check each control against animated_paths
```

## File Format

### Animation File (.ffanim)

```json
{
  "name": "Spiral Zoom with Oscillation",
  "duration": 10.0,
  "loop_mode": "Loop",
  "tracks": {
    "Zoom": {
      "source": {
        "type": "Keyframes",
        "keyframes": [
          {"time": 0.0, "value": 1.0, "easing": "EaseInOut"},
          {"time": 5.0, "value": 10.0, "easing": "EaseInOut"},
          {"time": 10.0, "value": 1.0, "easing": "Linear"}
        ]
      },
      "interpolation": "Linear"
    },
    "Rotation": {
      "source": {
        "type": "Keyframes",
        "keyframes": [
          {"time": 0.0, "value": 0.0, "easing": "Linear"},
          {"time": 10.0, "value": 360.0, "easing": "Linear"}
        ]
      },
      "interpolation": "Linear"
    },
    "Exposure": {
      "source": {
        "type": "Oscillator",
        "oscillator_type": "Sine",
        "center": 1.0,
        "amplitude": 0.3,
        "frequency": 0.5,
        "phase": 0.0
      },
      "interpolation": "Linear"
    }
  },
  "circular_tracks": [
    {
      "target_x": "PanX",
      "target_y": "PanY",
      "center_x": 0.0,
      "center_y": 0.0,
      "radius": 0.2,
      "speed": 0.25,
      "phase": 0.0
    }
  ]
}
```

## Implementation Phases

### Phase 1: Core Infrastructure ✅ (Scaffolding exists)
- [x] Animation data structures (`src/animation/mod.rs`)
- [x] EasingFunction with apply() method
- [x] Basic AnimationController
- [x] Keyframe interpolation
- [ ] Oscillator track evaluation
- [ ] Circular track evaluation

### Phase 2: ConfigManager Integration
- [ ] Add `update_param_silent()` to ConfigManager
- [ ] Add ConfigPath string serialization/deserialization
- [ ] Add JSON → ConfigValue conversion helper

### Phase 3: App Integration
- [ ] Add animation_playing flag check in render loop
- [ ] Integrate AnimationController.evaluate_frame() with ConfigManager
- [ ] Set overwrite mode when animation_playing
- [ ] Basic playback controls (play/pause/stop)

### Phase 4: UI
- [ ] Simple timeline scrubber (horizontal slider)
- [ ] Play/Pause/Stop buttons
- [ ] Speed control
- [ ] Animation load/save dialogs
- [ ] Lock animated parameters during playback

### Phase 5: Animation Editor (Future)
- [ ] Visual keyframe timeline
- [ ] Drag to move keyframes
- [ ] Add/delete keyframes
- [ ] Easing curve preview
- [ ] Track management (add/remove tracks)

### Phase 6: Video Export (Future)
- [ ] CLI: `fractal_flame_wgpu animate -i config.fflame -a animation.ffanim -o output.mp4`
- [ ] FFmpeg integration
- [ ] PNG sequence fallback
- [ ] Progress indicator

## Technical Considerations

### ConfigPath Serialization

Need bidirectional conversion between ConfigPath enum and string for JSON:

```rust
impl ConfigPath {
    pub fn to_string_key(&self) -> String {
        match self {
            ConfigPath::Zoom => "Zoom".to_string(),
            ConfigPath::Pan => "Pan".to_string(),
            ConfigPath::TransformAffine { index, param } => {
                format!("TransformAffine.{}.{:?}", index, param)
            }
            ConfigPath::TransformVariation { index, variation } => {
                format!("TransformVariation.{}.{}", index, variation)
            }
            // ... etc
        }
    }

    pub fn from_string_key(s: &str) -> Result<Self, ParseError> {
        // Parse string back to ConfigPath
    }
}
```

### JSON ↔ ConfigValue Conversion

```rust
fn json_to_config_value(json: &serde_json::Value, path: &ConfigPath) -> Result<ConfigValue, Error> {
    // Use path to determine expected type
    match path.expected_type() {
        ValueType::Float => Ok(ConfigValue::Float(json.as_f64()? as f32)),
        ValueType::Bool => Ok(ConfigValue::Bool(json.as_bool()?)),
        ValueType::Vec2 => {
            let arr = json.as_array()?;
            Ok(ConfigValue::Vec2(arr[0].as_f64()? as f32, arr[1].as_f64()? as f32))
        }
        // ... etc
    }
}
```

### Performance

- Oscillators: O(1) per evaluation (just math)
- Keyframe interpolation: O(log n) with binary search, O(n) with linear scan
- Circular tracks: O(1) per evaluation
- Total per frame: Negligible compared to GPU rendering

### Edge Cases

- **Before first keyframe:** Hold first value
- **After last keyframe:** Hold last value (or loop)
- **Empty track:** Skip (use current config value)
- **Invalid ConfigPath string:** Log warning, skip track
- **Animation duration = 0:** Treat as single frame

## Open Questions (Resolved)

1. ~~Timeline UI Library~~ → Start with simple egui slider, expand later
2. ~~Keyframe Editing~~ → JSON manual edit for MVP, GUI editor in Phase 5
3. ~~Parameter Locking~~ → Lock ALL parameters during playback for simplicity (can relax later)
4. ~~Export Frame Rate~~ → Fixed FPS (30 or 60), configurable in export dialog

---

## Related Documents
- [ConfigPath enum](../../src/config/delta.rs) - All animatable parameters
- [ConfigManager](../../src/config/manager.rs) - Parameter update infrastructure
- [Existing scaffolding](../../src/animation/) - Animation module with basic types

## References
- Existing easing functions: `src/animation/interpolation.rs`
- Existing controller: `src/animation/controller.rs`
