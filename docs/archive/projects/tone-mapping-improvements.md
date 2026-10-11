# Tone Mapping Improvements Project

## Overview

Improve the tone mapping system with Photoshop-style controls for better artistic control over the final image. The goal is to provide more intuitive controls than Apophysis's gamma/brightness/vibrancy system.

## Current System Analysis

### Color Accumulation Pipeline

1. **Compute Shader**: Colors clamped to 0-1 BEFORE writing to histogram
   ```wgsl
   let r_u32 = u32(clamp(final_color.r, 0.0, 1.0) * color_scale);
   ```

2. **Accumulate Shader**: Colors averaged (sum/count), density accumulates additively
   ```wgsl
   let new_color = vec3(r_sum/density, g_sum/density, b_sum/density);
   ```

3. **Tonemap Shader**: Applies logarithmic curve based on density

### Key Insight: Density is the HDR Channel

- **RGB values**: Always 0-1 (averaged per-pixel color)
- **Density**: Unbounded (can be millions of hits per pixel)
- **White blowout**: White pixels stay white regardless of hit count - no HDR color accumulation
- All dynamic range information is in **density only**

### Current Parameters

| Parameter | Purpose | Apophysis Equivalent |
|-----------|---------|---------------------|
| `brightness` | Log curve scaling | brightness |
| `gamma` | Power curve exponent | gamma |
| `gamma_threshold` | Linear toe for shadows | gamma_threshold |
| `vibrancy` | Palette vs grayscale blend | vibrancy |
| `white_level` | Log curve knee point | white_level (200.0) |
| `sample_density` | Iterations/pixel normalization | quality |
| `exposure` | Final multiplier | - |

### Problems with Current System

1. **Unintuitive controls** - gamma_threshold, white_level, bright_adjust are obscure
2. **No visual feedback** - Can't see density distribution
3. **Fixed curve shape** - Logarithmic only, no direct manipulation
4. **White blowout** - Dense white areas lose all detail

## Proposed Solution: Histogram + Levels Controls

### Phase 1: Density Histogram Display

Add a real-time histogram visualization showing density distribution:

```
Density Histogram
┌────────────────────────────────────┐
│    ▄                               │
│   ██▄                              │
│  ████▄     ▄                       │
│ ██████▄▄▄██▄                    ▄  │
│████████████████▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄██▄│
└────────────────────────────────────┘
 0                                 max
      ▲           ▲            ▲
     min       midpoint       max
```

**Implementation:**
- Compute histogram on CPU from accumulation buffer readback
- Display in new "Histogram" panel or integrated into Colors panel
- Update every N frames (not every frame - too expensive)
- Logarithmic X-axis option for better visualization of sparse data

### Phase 2: Levels Controls (Photoshop-style)

Three-point control over density-to-brightness mapping:

| Control | Purpose | Default |
|---------|---------|---------|
| **Input Black** | Densities below this → black | 0 |
| **Input White** | Densities above this → white | auto |
| **Midpoint (Gamma)** | Controls curve shape | 1.0 |

Formula:
```
normalized = (density - input_black) / (input_white - input_black)
output = pow(clamp(normalized, 0, 1), 1/midpoint)
```

**UI:**
- Draggable triangles below histogram (like Photoshop)
- Or three sliders: Black Point, White Point, Midpoint
- "Auto" button to set white point from histogram (e.g., 99th percentile)

### Phase 3: Shadow/Highlight/Contrast Controls

Additional artistic controls:

| Control | Purpose | Range |
|---------|---------|-------|
| **Shadows** | Lift/lower dark areas | -100 to +100 |
| **Highlights** | Lift/lower bright areas | -100 to +100 |
| **Contrast** | S-curve intensity | -100 to +100 |

These would be applied AFTER the levels adjustment, similar to Lightroom/Camera Raw.

### Note: Existing RGB Tone Curve

The codebase already has a curve editor in `src/ui/tone_mapping.rs` that operates on
**final RGB values** (0-1), applied AFTER tone mapping (tonemap.wgsl lines 502-508).

A separate density curve editor is **not needed** - Levels controls provide sufficient
flexibility for the density-to-brightness mapping. The real value is the histogram
visualization that lets you see where density values actually fall.

**Pipeline position:**
```
Density (raw hits)
    → [NEW: Histogram + Levels]
    → brightness_scale() (modified to use levels)
    → Color brightness
    → [EXISTING: RGB Curve - unchanged]
    → Final RGB
```

## Technical Implementation

### Histogram Computation

**Option A: GPU Compute Shader**
- Dispatch compute shader to bin density values
- Use atomic operations for histogram bins
- Very fast, but requires additional GPU resources

**Option B: CPU Readback**
- Read accumulation buffer to CPU periodically
- Compute histogram in parallel with rayon
- Simpler, works with existing architecture

Recommendation: Start with CPU (Option B) for simplicity, optimize later if needed.

### Shader Changes

New uniform buffer for levels/curves:
```wgsl
struct LevelsParams {
    input_black: f32,      // Density value mapped to black
    input_white: f32,      // Density value mapped to white
    midpoint: f32,         // Gamma-like curve control
    shadows: f32,          // Shadow lift/lower
    highlights: f32,       // Highlight lift/lower
    contrast: f32,         // S-curve intensity
}
```

Apply in tonemap.wgsl AFTER current brightness_scale() but BEFORE gamma:
```wgsl
// Levels adjustment
let normalized = (bucket_count - levels.input_black) /
                 (levels.input_white - levels.input_black);
let leveled = pow(clamp(normalized, 0.0, 1.0), 1.0 / levels.midpoint);

// Shadow/Highlight adjustment
let shadow_adjusted = apply_shadows(leveled, levels.shadows);
let highlight_adjusted = apply_highlights(shadow_adjusted, levels.highlights);

// Contrast (S-curve)
let contrasted = apply_contrast(highlight_adjusted, levels.contrast);
```

### UI Components

1. **HistogramWidget** - Renders density histogram with overlay controls
2. **LevelsSliders** - Black/White/Midpoint controls
3. **ShadowHighlightSliders** - Shadow/Highlight/Contrast controls

Location options:
- New "Histogram" panel (recommended - keeps it separate)
- Integrated into existing "Colors" panel (might be crowded)

## Migration Path

### Compatibility with Existing Controls

Keep existing Apophysis-compatible controls (gamma, brightness, vibrancy) as "Legacy" or "Advanced" options. New controls would be the default for new users.

Option to convert between systems:
- Import Apophysis flame → convert to new system
- Export → convert back to Apophysis parameters (approximate)

### Config Storage

Add new fields to FractalConfig:
```rust
pub struct ToneMappingConfig {
    // New system
    pub use_levels: bool,
    pub input_black: f32,
    pub input_white: f32,
    pub midpoint: f32,
    pub shadows: f32,
    pub highlights: f32,
    pub contrast: f32,

    // Legacy (Apophysis-compatible)
    pub gamma: f32,
    pub brightness: f32,
    pub vibrancy: f32,
    pub gamma_threshold: f32,
}
```

## File Changes

### New Files
- `src/ui/histogram.rs` - Histogram widget and computation
- `src/ui/levels.rs` - Levels/curves UI components (or integrate into tone_mapping.rs)

### Modified Files
- `shaders/tonemap.wgsl` - Add levels/curves processing
- `src/gpu/buffers.rs` - Add LevelsParams uniform
- `src/config/fractal_config.rs` - Add tone mapping config fields
- `src/config/delta.rs` - Add ConfigPath variants for new params
- `src/ui/tone_mapping.rs` - Add histogram and levels UI

## Open Questions

1. **Histogram update frequency** - Every frame? Every 100ms? On demand?
2. **Histogram bins** - 256? 1024? Logarithmic spacing?
3. **Auto white point** - 99th percentile? 99.9th? User-configurable?
4. **Curve editor** - Worth the complexity? Or levels sufficient?
5. **Legacy mode** - Keep Apophysis controls visible? Hidden behind "Advanced"?

## Success Criteria

- [ ] Real-time histogram display showing density distribution
- [ ] Levels controls (black/white/midpoint) with visual feedback
- [ ] Shadow/Highlight/Contrast sliders
- [ ] Better default appearance than current system
- [ ] No regression in render performance
- [ ] Config save/load preserves new settings

## References

- Photoshop Levels: Input/Output Black/White with gamma midpoint
- Lightroom Basic panel: Exposure, Contrast, Highlights, Shadows, Whites, Blacks
- Apophysis source: ImageMaker.pas tone mapping implementation
- Current implementation: shaders/tonemap.wgsl
