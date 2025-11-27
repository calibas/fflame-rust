# Animation Track Editor

## Status: Complete

UI for editing animation tracks directly in the Animation panel is now implemented.

## Features

Users can:
- Add new tracks (Keyframe, Oscillator, or Circular)
- Edit existing track properties
- Delete tracks
- Select target parameters from a categorized dropdown list
- Animate transform Origin X/Y (translate), Rotation, and Scale directly

## Track Types

1. **Keyframe tracks** (`Track` with `TrackSource::Keyframes`)
   - Target: Single ConfigPath (stored as String)
   - Properties: List of keyframes (time, value, easing), interpolation method

2. **Oscillator tracks** (`Track` with `TrackSource::Oscillator`)
   - Target: Single ConfigPath (stored as String)
   - Properties: oscillator_type (Sine/Triangle/Sawtooth/Square), center, amplitude, frequency, phase

3. **Circular tracks** (`CircularTrack`)
   - Targets: TWO ConfigPaths (target_x, target_y)
   - Properties: center_x, center_y, radius, speed, phase

## Animatable Parameters

**View (smooth, no reset):**
- Zoom, Rotation, CameraRotationX, CameraRotationY, CameraZ

**Tone Mapping:**
- Exposure, Gamma, GammaThreshold, Brightness, Vibrancy, Saturation, HueShift, ValueScale
- AlphaBlendLow, AlphaBlendHigh, DensityScale

**Color:**
- PaletteRotation, SpeedFactor, HistogramColorScale

**Rendering:**
- BlendFactor, PerspectiveStrength, LowDensitySmoothing

**Transform-level (per transform index):**
- Weight, Color, ColorSpeed, Opacity
- **Origin X (Translate)** - High-level translate X operation
- **Origin Y (Translate)** - High-level translate Y operation
- **Rotation** - High-level rotation operation (radians)
- **Scale** - High-level uniform scale operation
- Affine A, B, C, D, E, F, G - Raw affine coefficients (advanced)
- Variations (by name)

## High-Level Transform Operations

The track editor supports animating transforms using intuitive high-level operations rather than raw affine coefficients:

- **Origin X / Origin Y**: Translate the transform's origin point. Uses Apophysis conventions where origin_x = e and origin_y = -f.
- **Rotation**: Rotate the transform about its origin. Value in radians, computed from atan2(b, a).
- **Scale**: Uniformly scale the transform. Computed from sqrt(a² + b²).

These high-level operations automatically update the underlying affine coefficients (a, b, c, d, e, f) when animated.

For advanced users who need full control, the raw affine parameters (A-G) are also available as animation targets.

## Implementation

### Files Changed

1. **`src/ui/track_editor.rs`** - Main track editor module:
   - `TrackEditorState` struct for UI state
   - `render_track_editor()` function
   - `render_keyframe_editor()` function
   - `render_add_track_dialog()` function
   - `animatable_parameters()` helper function (includes Origin/Rotation/Scale targets)

2. **`src/config/delta.rs`** - Added ConfigPath variants:
   - `TransformOriginX { index }` - High-level origin X (translate X)
   - `TransformOriginY { index }` - High-level origin Y (translate Y)
   - `TransformRotation { index }` - High-level rotation
   - `TransformScale { index }` - High-level uniform scale

3. **`src/config/manager.rs`** - Added get/apply handlers:
   - `get_value()` support for new transform operation paths
   - `apply_value()` support using Transform's set_* methods

4. **`src/scene/transforms.rs`** - Transform operation methods:
   - `origin_x()` / `set_origin_x()` - Translate X operations
   - `origin_y()` / `set_origin_y()` - Translate Y operations
   - `rotation()` / `set_rotation()` - Rotation operations
   - `scale()` / `set_scale()` - Scale operations

5. **`src/animation/mod.rs`** - Track management:
   - `remove_track()` / `remove_circular_track()` methods

6. **`src/ui/mod.rs`** - Module export

7. **`src/ui/panel_viewer.rs`** - Integration:
   - Added `track_editor_state` to PanelContext
   - Replaced `render_track_summary` with `render_track_editor`

8. **`src/ui/animation_panel.rs`** - Added:
   - `new_animation` field to AnimationPanelResponse
   - "+ New" button to file controls

## UI Design

### Track List Section

```
┌─ Tracks (3) ──────────────────────────────┐
│ [+ Add Track ▼]                           │
│                                           │
│ ┌─────────────────────────────────────────┤
│ │ Zoom (Keyframes)                    [🗑] │
│ │ ├─ Keyframes: 2                         │
│ │ ├─ Interpolation: [Linear ▼]            │
│ │ └─ [Edit Keyframes]                     │
│ └─────────────────────────────────────────┤
│ ┌─────────────────────────────────────────┤
│ │ Exposure (Oscillator)               [🗑] │
│ │ ├─ Type: [Sine ▼]                       │
│ │ ├─ Center: [1.0    ]                    │
│ │ ├─ Amplitude: [0.5 ]                    │
│ │ ├─ Frequency: [0.2 ]                    │
│ │ └─ Phase: [0.0     ]                    │
│ └─────────────────────────────────────────┤
│ ┌─────────────────────────────────────────┤
│ │ PanX, PanY (Circular)               [🗑] │
│ │ ├─ Center: [0.0, 0.0]                   │
│ │ ├─ Radius: [0.5    ]                    │
│ │ ├─ Speed: [0.1     ] rev/s              │
│ │ └─ Phase: [0.0     ]                    │
│ └─────────────────────────────────────────┤
└───────────────────────────────────────────┘
```

### Keyframe Editor

```
┌─ Keyframes for "Zoom" ─────────────────────┐
│ Time     Value     Easing                  │
│ [0.00 ] [1.0   ] [Linear ▼]           [🗑] │
│ [5.00 ] [3.0   ] [EaseInOut ▼]        [🗑] │
│ [10.0 ] [1.0   ] [Linear ▼]           [🗑] │
│                                            │
│ [+ Add Keyframe]                           │
│                                            │
│ [Done]                                     │
└────────────────────────────────────────────┘
```

## Edge Cases Handled

1. **No animation loaded**: Show "New Animation" button only
2. **Empty animation**: Show "Add Track" button, no track list
3. **Duplicate targets**: Allow same parameter in multiple tracks (user's choice)
4. **Invalid keyframe order**: "Sort by Time" button available in keyframe editor
