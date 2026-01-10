# Refactor Background Compositing to Tonemap Shader

## Summary

This PR moves background color compositing from the accumulate shader to the tonemap shader, fixing visual artifacts and enabling transparent PNG exports.

**Key changes:**
- Background blending now happens in tonemap pass (after accumulation completes)
- Added adjustable alpha blend sliders to control edge quality vs density detail
- Added transparent PNG export with proper alpha channel
- Fixed overwrite mode smearing during parameter changes

## Problem

Previously, background color was blended into the accumulation buffer itself. This caused several issues:
1. **Dark halos at fractal edges** when zoomed out (alpha curve rose too slowly at low densities)
2. **Smearing artifacts** during live parameter changes (background contaminated the running average)
3. **No way to export transparent PNGs** (background was already baked in)

## Solution

### Architecture Change

**Before:** `Accumulate shader` → blend RGB with background → write to buffer
**After:** `Accumulate shader` → store raw RGB + density → `Tonemap shader` → blend with background

The accumulate shader now stores clean fractal colors and density. Background compositing happens once at the end in the tonemap pass, giving us:
- Clean separation between accumulation and display
- Ability to switch backgrounds without re-rendering
- Proper alpha channel for transparent export

### Alpha Blending Curve

To fix dark halos while preserving density detail, we blend between two alpha calculations:
- **Gamma-corrected alpha** (rises fast at edges, no halos)
- **Linear alpha** (preserves mid-range density variation)

New sliders in Tone Mapping panel:
- **Alpha Blend Low** (default 0.3): Start transition from gamma to linear
- **Alpha Blend High** (default 0.8): Full linear alpha above this value

### Transparent Export

Added `transparent_mode` to the tonemap shader:
- When enabled, outputs fractal color + alpha directly (no background blend)
- UI: File menu → "Export Transparent PNG..."
- UI: Settings → Export → "Export Transparent" button
- Works with both viewport and custom export sizes

### Overwrite Mode Fix

Fixed smearing during live parameter changes:
- Accumulate shader now detects overwrite mode (`blend_factor >= 0.99`)
- Clears to zero instead of keeping stale data when no new samples arrive
- Prevents ghost images during real-time editing

## Files Changed

- **shaders/tonemap.wgsl** - Added background compositing, alpha blending, transparent mode
- **shaders/accumulate.wgsl** - Removed background blending, added overwrite mode handling
- **src/gpu/buffers.rs** - Added alpha_blend_low/high and transparent_mode to TonemapParams
- **src/renderer/compute_kernel.rs** - Added set_transparent_mode(), updated buffer handling
- **src/config/** - Added alpha_blend_low, alpha_blend_high fields to FractalConfig
- **src/app/mod.rs** - Transparent export handling for viewport export
- **src/app/config.rs** - Transparent export handling for custom size export
- **src/app/export.rs** - Added transparent parameter to headless export
- **src/ui/** - Added transparent export menu item and button, alpha blend sliders

## Testing

- [x] Background color changes work without re-accumulation
- [x] No dark halos at fractal edges when zoomed out
- [x] No smearing during live parameter changes
- [x] Transparent PNG export works (viewport size)
- [x] Transparent PNG export works (custom size)
- [x] Alpha blend sliders adjust edge quality
- [x] Desktop and WASM builds compile

## Screenshots

*(Add before/after screenshots showing halo fix and transparent export)*
