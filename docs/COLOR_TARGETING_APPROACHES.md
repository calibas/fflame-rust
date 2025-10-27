# Color Targeting in Light/Dark Areas - Implementation Approaches

**Date:** 2025-10-27
**Status:** Research and Planning
**Goal:** Provide artistic control over how colors appear in sparse (dark) vs dense (bright) areas

---

## ⚠️ Critical Architectural Note

**Density information is readily available in the tonemap shader (post-accumulation):**
- ✅ Accumulation buffer stores density in alpha channel
- ✅ Tonemap shader already reads this (`let density = accum.a`)
- ✅ Zero architectural changes needed
- ✅ Zero performance cost

**Pre-accumulation approaches are architecturally problematic:**
- ❌ Compute shader doesn't have access to density during rendering
- ❌ Would require reading previous frame (temporal lag)
- ❌ Race conditions (density changes during accumulation)
- ❌ Performance cost (extra texture read in hot path)

**Verdict:** All post-accumulation approaches (1-4) are feasible. Pre-accumulation (5) is not recommended.

---

## Overview

Fractal flames naturally create areas with vastly different density values - from sparse (few iterations hit) to dense (many iterations hit). Artists often want different color behavior in these regions:

- **Dark/Sparse areas:** Boost vibrance, increase contrast
- **Bright/Dense areas:** Compress highlights, reduce saturation
- **Mid-tones:** Standard behavior

This document explores all approaches for achieving density-aware color control.

---

## Current State Analysis

### Existing Variables (Already in Code)

#### 1. **Tone Curve (1D LUT)**
**Status:** ✅ **Fully Implemented** (lines 80-85 in tonemap.wgsl)

**Location:** `shaders/tonemap.wgsl`
```wgsl
if (tonemap_params.use_curve != 0u && density > 0.001) {
    let r = textureSample(curve_lut_texture, curve_lut_sampler, color.r).r;
    let g = textureSample(curve_lut_texture, curve_lut_sampler, color.g).r;
    let b = textureSample(curve_lut_texture, curve_lut_sampler, color.b).r;
    fractal_color = vec3<f32>(r, g, b);
}
```

**What it does:**
- Maps input color value → output color value per channel
- Applies S-curve or other adjustments
- Applied AFTER tone mapping (log/linear + gamma)

**UI Control:** ✅ "Use Curve" toggle in Settings window

**Limitation:** Not density-aware - applies same curve regardless of sparse vs dense

---

#### 2. **Density Scale**
**Status:** ✅ **Fully Implemented** (line 53 in tonemap.wgsl)

**Location:** `shaders/tonemap.wgsl`
```wgsl
let normalized_density = sqrt(density * tonemap_params.density_scale);
```

**What it does:**
- Controls overall brightness contribution from density
- Uses sqrt() for compression (prevents unbounded growth)
- Applied to both color intensity and alpha

**UI Control:** ✅ "Density Scale" slider in Settings window (default 1.0)

**Limitation:** Global multiplier - same behavior everywhere

---

#### 3. **Tonemap Mode**
**Status:** ✅ **Fully Implemented** (lines 63-69 in tonemap.wgsl)

**Location:** `shaders/tonemap.wgsl`
```wgsl
if (tonemap_params.tonemap_mode == 0u) {
    // Linear: simple clamping
    color = clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));
} else {
    // Logarithmic: compress bright areas
    color = log(color + 1.0) / log(10.0);
}
```

**What it does:**
- **Linear mode:** Direct mapping, can wash out bright areas
- **Logarithmic mode:** Compresses high values, shows more detail in bright areas

**UI Control:** ✅ Tonemap Mode dropdown (Linear/Logarithmic) in Settings window

**Limitation:** Uniform compression - doesn't distinguish by density directly

---

#### 4. **Exposure**
**Status:** ✅ **Fully Implemented** (line 60 in tonemap.wgsl)

**Location:** `shaders/tonemap.wgsl`
```wgsl
color *= tonemap_params.exposure;
```

**What it does:** Global brightness multiplier before tone mapping

**UI Control:** ✅ "Exposure" slider in Settings window (default 1.0)

**Limitation:** Global - same everywhere

---

#### 5. **Gamma**
**Status:** ✅ **Fully Implemented** (line 72 in tonemap.wgsl)

**Location:** `shaders/tonemap.wgsl`
```wgsl
color = pow(color, vec3<f32>(1.0 / tonemap_params.gamma));
```

**What it does:** Standard gamma correction for display

**UI Control:** ✅ "Gamma" slider in Settings window (default 2.2)

**Limitation:** Global - same everywhere

---

#### 6. **Histogram Color Scale**
**Status:** ✅ **Fully Implemented** (pre-accumulation, in compute shader)

**Location:** `shaders/core/main_2d.wgsl` lines 75-90

**What it does:** Controls precision of color accumulation in histogram (u32 fixed-point scale)

**UI Control:** ✅ "Histogram Color Scale" slider in Settings window (default 100.0)

**Usage:** Pre-accumulation, not for artistic color control

---

### Summary: Existing Variables

| Variable | Location | Stage | UI Control | Density-Aware? |
|----------|----------|-------|------------|----------------|
| Tone Curve (1D LUT) | Tonemap Shader | Post-Accumulation | ✅ Toggle | ❌ No |
| Density Scale | Tonemap Shader | Post-Accumulation | ✅ Slider | ⚠️ Global multiplier |
| Tonemap Mode | Tonemap Shader | Post-Accumulation | ✅ Dropdown | ⚠️ Uniform compression |
| Exposure | Tonemap Shader | Post-Accumulation | ✅ Slider | ❌ No |
| Gamma | Tonemap Shader | Post-Accumulation | ✅ Slider | ❌ No |
| Histogram Color Scale | Compute Shader | Pre-Accumulation | ✅ Slider | ❌ No (precision) |

**Key Finding:** All existing controls are either global or not density-aware. None provide selective behavior for dark vs bright areas.

---

## Proposed Approaches

### Approach 1: Density-Conditional Curves (Simple)

**Concept:** Apply different tone curves based on density ranges

**Implementation:**
```wgsl
// In tonemap.wgsl, replace single curve with density-based selection
if (tonemap_params.use_curve != 0u && density > 0.001) {
    var fractal_color: vec3<f32>;

    if (density < tonemap_params.dark_threshold) {
        // Dark areas: Use aggressive contrast curve
        fractal_color = apply_dark_curve(color);
    } else if (density > tonemap_params.bright_threshold) {
        // Bright areas: Use gentle highlight compression curve
        fractal_color = apply_bright_curve(color);
    } else {
        // Mid-tones: Standard curve
        fractal_color = apply_standard_curve(color);
    }
}
```

**New GPU Parameters:**
```rust
struct TonemapParams {
    // ... existing fields ...
    dark_threshold: f32,      // Density below which "dark curve" applies (default: 0.3)
    bright_threshold: f32,    // Density above which "bright curve" applies (default: 0.7)
}
```

**New Textures:**
- `curve_dark_lut_texture` (1D, 256 samples)
- `curve_bright_lut_texture` (1D, 256 samples)
- Keep existing `curve_lut_texture` for mid-tones

**UI Controls:**
- ✅ "Dark Area Threshold" slider (0.0 - 1.0)
- ✅ "Bright Area Threshold" slider (0.0 - 1.0)
- ✅ "Dark Area Curve" editor (S-curve UI)
- ✅ "Bright Area Curve" editor (S-curve UI)

**Pros:**
- Simple conditional logic
- Reuses existing curve infrastructure
- Easy to understand and control

**Cons:**
- Hard transitions at thresholds (unless blended)
- Limited to 3 zones (dark, mid, bright)

---

### Approach 2: 2D LUT (Density × Color) (Flexible)

**Concept:** Use density as a second dimension in color lookup

**Implementation:**
```wgsl
// In tonemap.wgsl, replace 1D curve with 2D lookup
if (tonemap_params.use_curve != 0u && density > 0.001) {
    // Normalize density to [0, 1] for texture lookup
    let density_normalized = clamp(density * tonemap_params.density_scale, 0.0, 1.0);

    // 2D texture: (color_value, density) → adjusted_color
    let r = textureSample(curve_2d_lut, sampler, vec2(color.r, density_normalized)).r;
    let g = textureSample(curve_2d_lut, sampler, vec2(color.g, density_normalized)).g;
    let b = textureSample(curve_2d_lut, sampler, vec2(color.b, density_normalized)).b;

    fractal_color = vec3<f32>(r, g, b);
}
```

**New Textures:**
- `curve_2d_lut` (2D, 256×256 samples)
  - X-axis: input color value (0.0 - 1.0)
  - Y-axis: density (0.0 - 1.0)
  - Value: output color value (0.0 - 1.0)

**UI Controls:**
- 2D gradient editor (like Photoshop's Curves with multiple points)
- Vertical bands for density ranges
- Horizontal curves for color adjustment per density range

**Pros:**
- Maximum flexibility
- Smooth transitions between density ranges
- Industry-standard approach (used in color grading)

**Cons:**
- More complex UI for editing 2D LUT
- Larger memory footprint (256×256 vs 256 samples)
- Need to implement 2D curve editor UI

---

### Approach 3: Parametric Adjustments (HSV-Based)

**Concept:** Adjust saturation, hue, and value based on density

**Implementation:**
```wgsl
// In tonemap.wgsl, after tone curve application
if (tonemap_params.use_density_adjustments != 0u && density > 0.001) {
    // Convert to HSV
    let hsv = rgb_to_hsv(fractal_color);

    // Parametric adjustments based on density
    let t = clamp(density * tonemap_params.density_scale, 0.0, 1.0);

    // Saturation: boost in dark areas, reduce in bright areas
    let sat_curve = mix(
        tonemap_params.dark_saturation_mult,    // Dark areas
        tonemap_params.bright_saturation_mult,  // Bright areas
        t
    );
    hsv.y *= sat_curve;

    // Hue shift: cool in dark, warm in bright (optional)
    let hue_shift = mix(
        tonemap_params.dark_hue_shift,    // Dark areas (e.g., -0.05 = cooler)
        tonemap_params.bright_hue_shift,  // Bright areas (e.g., +0.05 = warmer)
        t
    );
    hsv.x = fract(hsv.x + hue_shift);  // Wrap around [0, 1]

    // Value: optional brightness adjustment
    let value_curve = mix(
        tonemap_params.dark_value_mult,
        tonemap_params.bright_value_mult,
        t
    );
    hsv.z *= value_curve;

    // Convert back to RGB
    fractal_color = hsv_to_rgb(hsv);
}
```

**New GPU Parameters:**
```rust
struct TonemapParams {
    // ... existing fields ...
    use_density_adjustments: u32,  // Enable/disable

    // Saturation control
    dark_saturation_mult: f32,     // Saturation multiplier in dark areas (default: 1.5)
    bright_saturation_mult: f32,   // Saturation multiplier in bright areas (default: 0.8)

    // Hue shift control
    dark_hue_shift: f32,           // Hue shift in dark areas (default: 0.0)
    bright_hue_shift: f32,         // Hue shift in bright areas (default: 0.0)

    // Value control
    dark_value_mult: f32,          // Value multiplier in dark areas (default: 1.0)
    bright_value_mult: f32,        // Value multiplier in bright areas (default: 1.0)
}
```

**Helper Functions Needed:**
```wgsl
fn rgb_to_hsv(rgb: vec3<f32>) -> vec3<f32> {
    // Standard RGB → HSV conversion
}

fn hsv_to_rgb(hsv: vec3<f32>) -> vec3<f32> {
    // Standard HSV → RGB conversion
}
```

**UI Controls:**
- ✅ "Use Density Adjustments" toggle
- ✅ "Dark Saturation" slider (0.0 - 3.0, default 1.5)
- ✅ "Bright Saturation" slider (0.0 - 3.0, default 0.8)
- ✅ "Dark Hue Shift" slider (-180° to +180°, default 0°)
- ✅ "Bright Hue Shift" slider (-180° to +180°, default 0°)
- ✅ "Dark Value" slider (0.0 - 2.0, default 1.0)
- ✅ "Bright Value" slider (0.0 - 2.0, default 1.0)

**Pros:**
- Intuitive controls (saturation, hue, value)
- No additional textures needed
- Linear interpolation provides smooth transitions
- Artists familiar with HSV adjustments

**Cons:**
- Limited to linear interpolation (no custom curves)
- RGB ↔ HSV conversion has slight performance cost
- May not suit all artistic goals

---

### Approach 4: Density Masks (Layer-Based)

**Concept:** Create density-based masks and blend different color treatments

**Implementation:**
```wgsl
// In tonemap.wgsl, after tone curve application
if (tonemap_params.use_density_masks != 0u && density > 0.001) {
    // Normalize density
    let d = clamp(density * tonemap_params.density_scale, 0.0, 1.0);

    // Generate masks for different density ranges
    let dark_mask = smoothstep(
        tonemap_params.dark_mask_start,
        tonemap_params.dark_mask_end,
        1.0 - d  // Invert so high values = dark areas
    );

    let bright_mask = smoothstep(
        tonemap_params.bright_mask_start,
        tonemap_params.bright_mask_end,
        d
    );

    // Apply different color treatments per layer
    var dark_layer = fractal_color;
    dark_layer = boost_saturation(dark_layer, tonemap_params.dark_boost);
    dark_layer *= tonemap_params.dark_tint;  // Tint color

    var bright_layer = fractal_color;
    bright_layer = compress_highlights(bright_layer, tonemap_params.bright_compress);
    bright_layer *= tonemap_params.bright_tint;  // Tint color

    // Blend layers using masks
    fractal_color = mix(fractal_color, dark_layer, dark_mask);
    fractal_color = mix(fractal_color, bright_layer, bright_mask);
}
```

**New GPU Parameters:**
```rust
struct TonemapParams {
    // ... existing fields ...
    use_density_masks: u32,

    // Dark mask
    dark_mask_start: f32,    // Density where dark mask starts (default: 0.0)
    dark_mask_end: f32,      // Density where dark mask peaks (default: 0.3)
    dark_boost: f32,         // Saturation boost (default: 1.5)
    dark_tint: vec3<f32>,    // RGB tint color (default: [1.0, 1.0, 1.0])

    // Bright mask
    bright_mask_start: f32,  // Density where bright mask starts (default: 0.7)
    bright_mask_end: f32,    // Density where bright mask peaks (default: 1.0)
    bright_compress: f32,    // Highlight compression (default: 0.8)
    bright_tint: vec3<f32>,  // RGB tint color (default: [1.0, 1.0, 1.0])
}
```

**UI Controls:**
- ✅ "Use Density Masks" toggle
- ✅ "Dark Mask Range" (min/max sliders)
- ✅ "Dark Saturation Boost" slider
- ✅ "Dark Tint Color" picker
- ✅ "Bright Mask Range" (min/max sliders)
- ✅ "Bright Highlight Compression" slider
- ✅ "Bright Tint Color" picker

**Pros:**
- Photoshop-like layer-based workflow
- Color tinting for artistic effects
- Smooth mask blending with smoothstep
- Separate control over dark and bright treatments

**Cons:**
- More complex parameter set
- Requires understanding of masking concepts
- Can get results similar to Approach 3 but with more complexity

---

## Pre-Accumulation Approaches

### Approach 5: Pre-Accumulation Color Adjustment (Compute Shader)

**Concept:** Modify colors BEFORE they're written to histogram, based on previous frame density

**Implementation:**
```wgsl
// In main_2d.wgsl / main_3d.wgsl, before histogram write
// Read density from previous frame's accumulation buffer
let prev_density = textureSample(prev_accumulation, sampler, screen_pos).a;

if (prev_density < params.pre_accum_dark_threshold) {
    // Sparse areas: boost saturation
    final_color = boost_saturation(final_color, params.pre_accum_dark_boost);
} else if (prev_density > params.pre_accum_bright_threshold) {
    // Dense areas: reduce saturation
    final_color = reduce_saturation(final_color, params.pre_accum_bright_reduce);
}

// Then write adjusted color to histogram
atomicAdd(&histogram[base_idx + 0u], u32(final_color.r * color_scale));
// ... etc
```

**Challenges:**
1. **Requires binding previous accumulation texture to compute shader**
   - Current architecture: compute writes to histogram, accumulate reads histogram
   - Need: compute also reads prev accumulation for density estimate
2. **Temporal lag:** Using previous frame's density creates 1-frame delay
3. **Complexity:** Adds read dependency in hot path (compute shader)

**New GPU Parameters:**
```rust
struct GpuParams {
    // ... existing fields ...
    pre_accum_dark_threshold: f32,
    pre_accum_dark_boost: f32,
    pre_accum_bright_threshold: f32,
    pre_accum_bright_reduce: f32,
}
```

**Pros:**
- Affects color accumulation directly (more fundamental)
- Could create different visual results than post-processing

**Cons:**
- More invasive to architecture
- Performance impact (additional texture read in hot path)
- 1-frame temporal lag
- Less intuitive than post-processing

---

## Comparison Matrix

| Approach | Complexity | Flexibility | Performance | UI Complexity | New Textures | Existing Infrastructure |
|----------|-----------|-------------|-------------|---------------|--------------|------------------------|
| **1. Density-Conditional Curves** | Low | Medium | Fast | Medium | 2× 1D LUT | ✅ Reuses curves |
| **2. 2D LUT** | Medium | High | Fast | High | 1× 2D LUT | ⚠️ New editor |
| **3. Parametric HSV** | Low | Medium | Medium | Low | None | ❌ Need HSV functions |
| **4. Density Masks** | Medium | High | Medium | High | None | ⚠️ Complex UI |
| **5. Pre-Accumulation** | High | Medium | Slow | Medium | None | ❌ Architecture change |

---

## Recommendations

### Phase 1: Quick Win (Easiest to Implement)
**Approach 3: Parametric HSV Adjustments**

**Why:**
- No new textures needed
- Simple linear interpolation
- Intuitive controls (6 sliders)
- Artists familiar with HSV
- Low risk, easy to test

**Implementation Effort:** ~2-3 hours
- Add 6 parameters to TonemapParams
- Implement rgb_to_hsv() and hsv_to_rgb() in WGSL
- Add parametric adjustment code in tonemap.wgsl
- Add UI sliders in settings window

**Immediate Value:** Artists can boost saturation in dark areas, desaturate bright areas

---

### Phase 2: Power User Feature
**Approach 1: Density-Conditional Curves**

**Why:**
- Reuses existing curve infrastructure
- More artistic control than parametric
- Simple UI (just add thresholds + 2 curve editors)

**Implementation Effort:** ~4-6 hours
- Add 2 new 1D LUT textures
- Add dark_threshold and bright_threshold parameters
- Modify curve application logic
- Extend UI with threshold sliders
- Clone existing curve editor UI for dark/bright curves

**Value:** Advanced users can design custom curves per density range

---

### Phase 3: Industry Standard
**Approach 2: 2D LUT**

**Why:**
- Maximum flexibility
- Standard in color grading
- Smooth transitions

**Implementation Effort:** ~8-12 hours
- Create 2D texture (256×256)
- Implement 2D curve editor UI (complex)
- Replace 1D lookup with 2D lookup
- Add density normalization controls

**Value:** Professional-grade color grading capability

---

### Not Recommended (Yet)
**Approach 5: Pre-Accumulation**

**Why:**
- High complexity, invasive changes
- Unclear if results differ significantly from post-processing
- Performance impact in hot path
- Temporal lag issues

**When to revisit:** If post-accumulation approaches prove insufficient

---

## Implementation Checklist

### For Approach 3 (Parametric HSV) - Recommended First

**Shader Changes:**
- [ ] Add `rgb_to_hsv()` function to `shaders/tonemap.wgsl`
- [ ] Add `hsv_to_rgb()` function to `shaders/tonemap.wgsl`
- [ ] Add parametric adjustment code after tone curve (lines 85-90)
- [ ] Add new fields to `TonemapParams` struct

**Rust Changes:**
- [ ] Update `TonemapParams` struct in `src/gpu/buffers.rs`
- [ ] Add default values in buffer creation
- [ ] Add fields to `FractalConfig` for serialization (optional)

**UI Changes:**
- [ ] Add "Density Adjustments" collapsible section in Settings window
- [ ] Add "Enable" toggle
- [ ] Add 6 sliders:
  - Dark Saturation (0.0 - 3.0, default 1.5)
  - Bright Saturation (0.0 - 3.0, default 0.8)
  - Dark Hue Shift (-180° to +180°, default 0°)
  - Bright Hue Shift (-180° to +180°, default 0°)
  - Dark Value (0.0 - 2.0, default 1.0)
  - Bright Value (0.0 - 2.0, default 1.0)

**Testing:**
- [ ] Test with sparse fractals (low density everywhere)
- [ ] Test with dense fractals (high density everywhere)
- [ ] Test with mixed density (zoomed views)
- [ ] Verify HSV conversion math is correct
- [ ] Check performance impact (should be minimal)

**Documentation:**
- [ ] Update ARCHITECTURE.md with new tone mapping stage
- [ ] Update COLOR_PIPELINE.md with density adjustments
- [ ] Add example use cases to CLAUDE.md

---

## Future Enhancements

### Beyond Basic Implementation
1. **Preset Adjustments** - Save/load density adjustment presets
2. **Per-Transform Adjustments** - Different adjustments per transform layer
3. **Animation** - Keyframe density adjustment parameters
4. **Feedback Loop** - Use current frame density to influence next frame (advanced)

---

## References

### Existing Code
- `shaders/tonemap.wgsl` - Current tone mapping implementation
- `src/gpu/buffers.rs` - TonemapParams struct (lines 172-184)
- `src/ui/mod.rs` - Settings window UI

### Related Concepts
- **Color grading:** Industry-standard post-processing for film/games
- **Tone mapping:** HDR → LDR conversion with artistic control
- **Density-based rendering:** Using sample count as artistic parameter

---

**Last Updated:** 2025-10-27
**Status:** Ready for implementation (Phase 1 recommended)
**Estimated Effort:** 2-3 hours for Parametric HSV approach
