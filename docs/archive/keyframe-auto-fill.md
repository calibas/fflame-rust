# Keyframe Auto-Fill for Add Track Dialog

## Overview

When adding a new track, automatically populate keyframe values based on current fractal state. This makes track creation faster and more intuitive.

## User Experience

1. User clicks "Add Track"
2. User selects a target parameter
3. **Automatically**: Start/end keyframes are populated with current value
4. For specific parameters (rotation), end value gets special treatment (e.g., +2π)
5. User can edit keyframes before clicking "Add Track"
6. Track is created with the configured keyframes

## Implementation Steps

### Phase 1: Read Current Values from Config
- [ ] Add `get_value(path: &ConfigPath) -> Option<f64>` to ConfigManager or create helper
- [ ] Handle all ConfigPath variants that return numeric values
- [ ] For transform params, need to extract from Flame struct

### Phase 2: Preview Keyframes in State
- [ ] Add `preview_keyframes: Vec<Keyframe>` to `TrackEditorState`
- [ ] Initialize when target is selected (in target selector callback)
- [ ] Default: `[Keyframe(0.0, current), Keyframe(duration, current)]`

### Phase 3: Show Keyframe Editor in Add Mode
- [ ] Modify `render_keyframe_subpanel` to work with preview_keyframes when not editing
- [ ] Allow editing time, value, easing for each preview keyframe
- [ ] Add/remove keyframes in preview list

### Phase 4: Use Preview Keyframes on Create
- [ ] Modify `update_or_create_track` to use `state.preview_keyframes` instead of defaults
- [ ] Clear preview_keyframes when dialog closes

### Phase 5: Parameter-Specific Auto-Fill
Implement special end-value logic for specific parameters:

| Parameter | End Value Logic |
|-----------|-----------------|
| `TransformRotation` | current + 2π (full rotation) |
| `ColorIndex` | current + 1.0 (full palette cycle) |
| `CameraPitch` | current + 2π |
| `CameraYaw` | current + 2π |
| All others | current (same as start) |

### Phase 6: Oscillator/Circular Defaults
- [ ] When creating oscillator track, set `center = current_value`
- [ ] When creating circular track, set `center_x/y = current_values`

## Technical Notes

### Getting Current Value
Need to map ConfigPath → actual value from FractalConfig:

```rust
fn get_current_value(config: &FractalConfig, path: &ConfigPath) -> Option<f64> {
    match path {
        ConfigPath::TransformRotation { index } => {
            config.flame.transforms.get(*index).map(|t| t.rotation())
        }
        ConfigPath::Zoom => Some(config.view.zoom),
        ConfigPath::ColorIndex { index } => {
            config.flame.transforms.get(*index).map(|t| t.color_index)
        }
        // ... etc
    }
}
```

### Transform Rotation
Transform rotation is stored implicitly in affine coefficients (a, b, d, e).
Need to extract: `rotation = atan2(b, a)` (assuming no skew).

## Files to Modify

- `src/ui/track_editor.rs` - Main changes (state, UI, creation logic)
- `src/config/manager.rs` or new helper - Value extraction
- `src/scene/transforms.rs` - May need `Transform::rotation()` method

## Status

- [x] Phase 1: Read Current Values
- [x] Phase 2: Preview Keyframes State
- [x] Phase 3: Keyframe Editor UI
- [x] Phase 4: Use on Create
- [x] Phase 5: Parameter-Specific Logic
- [x] Phase 6: Oscillator/Circular Defaults

## Completed 2026-01-24

All phases implemented:
- `get_current_value()` extracts values for ~40 ConfigPath variants
- `get_auto_fill_end_value()` returns special end values for rotation (+2π), color (+1.0)
- `initialize_preview_keyframes()` auto-fills when target selected
- `initialize_oscillator_center()` and `initialize_circular_centers()` for other track types
- Preview keyframe editor in Add mode with full interpolation/easing support
- Track creation uses preview keyframes instead of defaults
