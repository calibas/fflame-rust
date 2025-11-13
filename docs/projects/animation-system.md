# Animation System - Design Document

**Date:** 2025-11-13
**Status:** Planning
**Priority:** Future Feature (after frame sync fix merged)

---

## Overview

Add keyframe-based animation system that takes control of FractalConfig during playback, allowing automated parameter changes over time.

## Three Render States

### 1. Normal Mode (Current Default)
- **Accumulation:** Progressive multi-frame refinement
- **Config Control:** UI → ConfigManager → GPU
- **Overwrite:** `false`
- **User Actions:** Full UI control, all edits allowed

### 2. Preview Mode (Current Live Updates)
- **Accumulation:** Single-frame overwrite
- **Config Control:** UI → ConfigManager → GPU
- **Overwrite:** `true`
- **User Actions:** Active drag/interaction on specific controls
- **Scope:** Per-control opt-in (via `lazy=response.dragged()`)
- **Duration:** Temporary during interaction

### 3. Animation Mode (NEW)
- **Accumulation:** Single-frame overwrite
- **Config Control:** Animation → (ConfigManager?) → GPU
- **Overwrite:** `true`
- **User Actions:** Large UI sections disabled during playback
- **Frame Rate:** Match current app frame rate (60 FPS default)

## Design Decisions

### ConfigManager Usage
**Decision:** Use ConfigManager for parameter updates during animation

**Rationale:**
- Track-based system already uses `ConfigPath` enum (perfect match)
- Leverages existing parameter update infrastructure
- **Skip undo points** during animation playback (configurable flag)
- Can still use ConfigManager API without polluting undo history

**Implementation:**
```rust
// ConfigManager addition
impl ConfigManager {
    /// Update parameter without creating undo point
    /// Used by animation system during playback
    pub fn update_param_silent(&mut self, path: ConfigPath, value: ConfigValue) -> Result<UpdateType> {
        // Apply change without touching undo stack
        // Still returns UpdateType for GPU sync
    }
}
```

### Animation Data Structure
**Decision:** Parameter Tracks (not full config snapshots)

**Architecture:**
```rust
pub struct Animation {
    pub name: String,
    pub duration: f64,                          // Total animation length (seconds)
    pub tracks: HashMap<ConfigPath, Track>,     // Per-parameter tracks
    pub loop_mode: LoopMode,
}

pub struct Track {
    pub keyframes: Vec<Keyframe>,               // Sorted by time
    pub interpolation: Interpolation,
}

pub struct Keyframe {
    pub time: f64,                              // Seconds from start
    pub value: ConfigValue,                     // Parameter value at this time
    pub easing: EasingFunction,                 // To next keyframe
}

pub enum LoopMode {
    Once,           // Stop at end
    Loop,           // Restart from beginning
    PingPong,       // Reverse direction at ends
}

pub enum Interpolation {
    Step,           // Jump to value (no interpolation)
    Linear,         // Linear interpolation
    Smooth,         // Cubic/spline interpolation
}

pub enum EasingFunction {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    // More easing functions as needed
}
```

### Animation Controller
**Responsibilities:**
- Load/save animation files
- Track playback state (playing, paused, scrubbing)
- Calculate interpolated values for current time
- Update ConfigManager each frame (silent updates)

```rust
pub struct AnimationController {
    pub animation: Option<Animation>,
    pub state: PlaybackState,
    pub current_time: f64,      // Seconds from start
    pub speed: f64,             // Playback speed multiplier (1.0 = normal)
}

pub enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}

impl AnimationController {
    /// Calculate all parameter values for current time
    pub fn evaluate_frame(&self) -> HashMap<ConfigPath, ConfigValue> {
        // Interpolate all tracks at current_time
    }

    /// Advance time by delta (called each frame)
    pub fn update(&mut self, delta_time: f64) {
        if self.state == PlaybackState::Playing {
            self.current_time += delta_time * self.speed;
            // Handle loop_mode logic
        }
    }
}
```

### Frame Order Integration
Animation fits perfectly into our new frame order:

```rust
// Phase 1: UI (already done)
let ui_response = self.egui_layer.render_ui(...);

// Phase 2: Process Updates
if let Some(ref mut anim) = self.animation_controller {
    if anim.state == PlaybackState::Playing {
        // Animation takes control
        anim.update(delta_time);
        let frame_values = anim.evaluate_frame();

        for (path, value) in frame_values {
            // Silent update (no undo point)
            self.config_manager.update_param_silent(path, value)?;
        }
    }
} else {
    // Normal UI updates
    // ... existing ui_response handling ...
}

// Phase 3: Render with final config (already done)
let final_config = self.config_manager.active_config();
// ... compute/accumulate/tonemap ...
```

### UI Behavior During Animation

**Disabled Sections:**
- Settings panel
- Transforms panel (add/delete/edit)
- Palette editor
- All controls that would modify animated parameters

**Enabled Sections:**
- Animation timeline scrubbing
- Play/Pause/Stop buttons
- Playback speed control
- View controls (if not animated)

**Implementation:**
```rust
// In UI rendering
let animation_active = app.animation_controller
    .as_ref()
    .map_or(false, |a| a.state == PlaybackState::Playing);

ui.add_enabled(!animation_active, |ui| {
    // Controls that should be disabled during animation
});
```

## File Format

### Animation File (.ffanim)
JSON format for human readability and easy editing:

```json
{
  "name": "Spiral Zoom",
  "duration": 10.0,
  "loop_mode": "Loop",
  "tracks": {
    "Zoom": {
      "interpolation": "Linear",
      "keyframes": [
        {"time": 0.0, "value": 1.0, "easing": "EaseInOut"},
        {"time": 5.0, "value": 10.0, "easing": "EaseInOut"},
        {"time": 10.0, "value": 1.0, "easing": "Linear"}
      ]
    },
    "Rotation": {
      "interpolation": "Linear",
      "keyframes": [
        {"time": 0.0, "value": 0.0, "easing": "Linear"},
        {"time": 10.0, "value": 360.0, "easing": "Linear"}
      ]
    }
  }
}
```

## Implementation Phases

### Phase 1: Core Architecture (Planning - This Document)
- ✅ Design three render states
- ✅ Plan animation data structures
- ✅ Define ConfigManager integration
- ✅ Specify file format

### Phase 2: Preview Mode Decoupling (Separate Project)
**Goal:** Decouple preview mode from ConfigManager undo system

**Changes:**
- Add explicit `ui_state.active_preview` flag
- Track which control is in preview mode
- Preview mode = UI rendering state, not ConfigManager state
- ConfigManager still handles the actual updates

**File:** `src/app/mod.rs`
- Add `active_preview: Option<PreviewContext>` to App
- `PreviewContext { control_id: egui::Id, path: ConfigPath }`

**Why Separate:** This is orthogonal to animation - preview mode improvements benefit both normal UI usage and future animation work.

### Phase 3: Animation System Implementation (Separate Project)
**Goal:** Add full animation playback system

**Subtasks:**
1. Create animation data structures (`src/animation/mod.rs`)
2. Implement AnimationController with interpolation
3. Add `update_param_silent()` to ConfigManager
4. Integrate with frame order (Phase 2)
5. Add timeline UI panel
6. Implement file I/O (.ffanim format)
7. Add playback controls

**New Files:**
- `src/animation/mod.rs` - Core types
- `src/animation/interpolation.rs` - Interpolation/easing
- `src/animation/controller.rs` - Playback logic
- `src/ui/timeline.rs` - Timeline editor UI

### Phase 4: Animation Editor (Future)
**Goal:** GUI for creating/editing animations

**Features:**
- Visual timeline with keyframes
- Drag keyframes to reposition
- Add/delete keyframes
- Select easing functions
- Preview scrubbing
- Copy/paste keyframes

### Phase 5: Video Export (Future)
**Goal:** Render animations to video files

**Features:**
- Headless: `fractal_flame_wgpu animate -i config.fflame -a animation.ffanim -o output.mp4`
- GUI: Export dialog with format options (mp4, webm, gif)
- FFmpeg integration for encoding
- PNG sequence fallback

## Technical Considerations

### Frame Rate Matching
- Animation updates at app frame rate (60 FPS default)
- `delta_time` = time since last frame
- `current_time += delta_time` ensures smooth playback regardless of actual FPS

### Interpolation Edge Cases
- **Before first keyframe:** Use first keyframe value (hold)
- **After last keyframe:** Use last keyframe value (hold) or loop
- **Single keyframe:** Constant value (no interpolation)
- **Missing track:** Parameter not animated (use UI value)

### GPU Update Efficiency
- Animation may update many parameters per frame
- `update_param_silent()` returns `UpdateType` for batching
- Collect all `UpdateType`s, apply once at end of Phase 2
- Same pattern as current UI updates

### Memory Considerations
- Animation stored once in memory
- No per-frame config copies
- Keyframes are small (time + value)
- Typical animation: ~100 keyframes × 20 bytes = 2 KB

## Open Questions

1. **Timeline UI Library:**
   - Build custom timeline in egui?
   - Use existing Rust timeline widget?
   - Start with simple scrubber bar, expand later?

2. **Keyframe Editing:**
   - Edit keyframes in JSON manually (MVP)?
   - Or GUI editor required for Phase 3?

3. **Parameter Locking:**
   - Should non-animated parameters be editable during playback?
   - Or lock entire config during animation?

4. **Export Frame Rate:**
   - Video export at fixed FPS (30/60)?
   - Or match animation's natural timing?

## Next Steps

1. **Immediate:** Merge frame synchronization fix to main
2. **Next:** Decide which project to tackle first:
   - Preview Mode Decoupling (smaller, focused)
   - Animation System (larger, more features)
3. **Create:** Separate planning document for chosen project

---

## Related Documents
- [Frame Synchronization Issues](frame-synchronization-issues.md) - Prerequisites (✅ Fixed)
- [Undo/Redo Issues](undo-redo-issues.md) - Related to preview mode
- [UI Improvements](ui-improvements-docking.md) - UI structure

## References
- ConfigManager: `src/config/manager.rs`
- ConfigPath enum: `src/config/delta.rs`
- Current frame order: `src/app/mod.rs` lines 308-1115
