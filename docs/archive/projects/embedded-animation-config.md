# Embedded Animation Config

## Status: Complete

Core implementation complete. Existing animation files continue to work (backward compatible).

## Problem

Current `.anim` files are generic - they animate parameters without knowing what fractal they're applied to. This causes issues:

1. **Unpredictable results**: A zoom animation authored for one fractal looks wrong on another
2. **Transform mismatches**: Animating "Transform.1.Affine.A" fails if the target fractal doesn't have transform 1
3. **No reproducibility**: Can't share an animation and expect it to look the same

## Solution

Embed the full `FractalConfig` in the animation file. The animation becomes self-contained and reproducible.

## New Format

```json
{
  "name": "My Animation",
  "base_config": {
    "version": 1,
    "flame": { ... },
    "zoom": 1.0,
    "pan_x": 0.0,
    ...
  },
  "duration": 10.0,
  "tracks": {
    "Zoom": {
      "source": {
        "type": "Keyframes",
        "keyframes": [
          { "time": 0.0, "value": 1.0, "easing": "Linear" },
          { "time": 10.0, "value": 4.0, "easing": "EaseInOut" }
        ]
      },
      "interpolation": "Linear"
    }
  },
  "circular_tracks": [],
  "loop_mode": "Once"
}
```

## Behavior Changes

### Loading an Animation
1. Parse animation file
2. **Load `base_config` into ConfigManager** (replaces current fractal)
3. Load animation tracks into AnimationController
4. User sees the authored fractal with animation ready to play

### Playing an Animation
- No change - animation applies values to current config as before

### Saving an Animation
1. Capture current `FractalConfig` as `base_config`
2. Save animation tracks
3. Write to `.anim` file

### CLI Export
- Uses embedded `base_config` by default
- Optional `--config` flag to override (for advanced use)

## Migration

### Existing `.anim` Files
- Files without `base_config` continue to work (apply to current fractal)
- Add migration helper: "Capture current config" button to upgrade old animations

### File Size Impact
- `FractalConfig` is ~5-20KB JSON depending on transform count
- Acceptable tradeoff for reproducibility

## Implementation Steps

1. ✅ **Update Animation struct** - Add `base_config: Option<FractalConfig>` field
2. ✅ **Update serialization** - Handle new field in JSON serde (skip_serializing_if = "Option::is_none")
3. ✅ **Update load logic** - Apply base_config when loading animation (panel_viewer.rs)
4. ✅ **Update save logic** - Capture current config when saving (panel_viewer.rs)
5. **Update UI** - Show "Load Animation" replaces current fractal (with warning?) - Skipped, matches "Load Preset" behavior
6. **Update CLI** - Use embedded config by default - Future work
7. **Migrate example files** - Update assets/animations/*.anim - Optional, old files work

## UI Considerations

### Load Animation Warning
When loading an animation with embedded config:
> "This animation includes a fractal configuration. Loading it will replace your current fractal. Continue?"
> [Load] [Cancel]

Or just always replace without warning (simpler, matches "Load Preset" behavior).

### Save Animation
- Always captures current config
- No option to save without config (keeps it simple)

## Future: Animation Track Editor

Once this is done, we can add UI for creating/editing animation tracks directly:
- Add keyframe at current time
- Visual timeline with draggable keyframes
- Track list showing animated parameters

This makes generic animations unnecessary since users author animations for their specific fractal.

## Dependencies

- None

## Priority

High - Current animation system is confusing without this.
