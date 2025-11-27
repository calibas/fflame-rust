# Animation Track Editor

## Status: Complete

UI for editing animation tracks directly in the Animation panel is now implemented.

## Features

Users can:
- Add new tracks (Keyframe, Oscillator, or Circular)
- Edit existing track properties
- Delete tracks
- Select target parameters from a categorized dropdown list

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
- PaletteRotation, SpeedFactor

**Transform-level (per transform index):**
- TransformWeight, TransformColor, TransformColorSpeed, TransformOpacity
- TransformAffine (with param: A/B/C/D/E/F/G)
- TransformVariation (with variation name)

**Rendering:**
- HistogramColorScale, BlendFactor, PerspectiveStrength

## Implementation

### Files Changed

1. **`src/ui/track_editor.rs`** (NEW) - Main track editor module:
   - `TrackEditorState` struct for UI state
   - `render_track_editor()` function
   - `render_keyframe_editor()` function
   - `render_add_track_dialog()` function
   - `animatable_parameters()` helper function

2. **`src/animation/mod.rs`** - Added methods:
   - `remove_track()` method to Animation
   - `remove_circular_track()` method

3. **`src/ui/mod.rs`** - Module export

4. **`src/ui/panel_viewer.rs`** - Integration:
   - Added `track_editor_state` to PanelContext
   - Replaced `render_track_summary` with `render_track_editor`

5. **`src/ui/animation_panel.rs`** - Added:
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
