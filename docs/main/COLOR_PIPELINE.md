# Color and Tone Mapping Pipeline

## Overview

This document traces the complete pipeline from fractal iteration to final display, documenting how colors are computed, accumulated, and tone-mapped for display.

## Pipeline Stages

### Stage 1: Per-Iteration Color Assignment (Compute Shader)

**Location:** `shaders/core/main_2d.wgsl` (lines 40-65) and `shaders/core/main_3d.wgsl` (similar)

**Process:**
1. Each iteration generates a point via affine transform + variations
2. Color is assigned based on `color_mode`:
   - **Transform Mode:** Uses `xform.color` from the selected transform
   - **Palette Mode:** Looks up color from palette texture using `xform.color` as index
   - **Speed Mode:** Uses iteration speed (distance moved) to index into palette

**Code:**
```wgsl
// Color assignment (main_2d.wgsl lines 40-65)
var final_color = vec3<f32>(1.0, 1.0, 1.0);
if (params.color_mode == 0u) {
    // Transform mode: use transform color
    final_color = xform.color;
} else if (params.color_mode == 1u) {
    // Palette mode: look up from palette texture
    let palette_coord = vec2<f32>(xform.color, 0.5);
    final_color = textureSampleLevel(palette_texture, palette_sampler, palette_coord, 0.0).rgb;
} else if (params.color_mode == 2u) {
    // Speed mode: use iteration speed to index palette
    let speed = length(p.xy - prev_p.xy);
    let speed_normalized = clamp(speed * 10.0, 0.0, 1.0);
    let palette_coord = vec2<f32>(speed_normalized, 0.5);
    final_color = textureSampleLevel(palette_texture, palette_sampler, palette_coord, 0.0).rgb;
}
```

**Output:** RGB color (0.0-1.0) per iteration

---

### Stage 2: U32 Histogram Accumulation (Compute Shader) - Updated 2025-10-27

**Location:** `shaders/core/main_2d.wgsl` (lines 75-90) and `shaders/core/main_3d.wgsl` (similar)

**Process:**
1. Convert float colors to u32 fixed-point integers
2. Scale by `histogram_color_scale` parameter (default 100.0)
3. Write 4 separate u32 values (R, G, B, Density) - **no packing**
4. Atomic add to histogram buffer

**Code:**
```wgsl
// Convert colors to u32 fixed-point (main_2d.wgsl lines 75-82)
let color_scale = params.histogram_color_scale;
let r_u32 = u32(clamp(final_color.r, 0.0, 1.0) * color_scale);
let g_u32 = u32(clamp(final_color.g, 0.0, 1.0) * color_scale);
let b_u32 = u32(clamp(final_color.b, 0.0, 1.0) * color_scale);
let density_u32 = u32(color_scale);

// Atomic add to histogram (4 separate u32 words)
let base_idx = pixel_idx * 4u;  // 4 words per pixel
atomicAdd(&histogram[base_idx + 0u], r_u32);
atomicAdd(&histogram[base_idx + 1u], g_u32);
atomicAdd(&histogram[base_idx + 2u], b_u32);
atomicAdd(&histogram[base_idx + 3u], density_u32);
```

**Parameters:**
- `histogram_color_scale`: Controls precision (1.0-1000.0, default 100.0)
  - Higher scale = more color precision
  - Lower scale = coarser quantization
  - **Overflow is no longer a concern** with u32 (max 4.2 billion)

**Capacity:**
- **U32 max per channel:** 4,294,967,295
- **At scale=100:** 42,949,672 hits before overflow (~91 minutes of continuous rendering)
- **At scale=1000:** 4,294,967 hits before overflow (~9 minutes)
- **Practical result:** Overflow effectively eliminated

**Output:** U32 unpacked histogram buffer (4× u32 per pixel: R, G, B, Density)

---

### Stage 3: Histogram Decoding (Accumulate Shader) - Updated 2025-10-27

**Location:** `shaders/accumulate.wgsl` (lines 28-57)

**Process:**
1. Read 4× u32 values directly (R, G, B, Density) - **no unpacking needed**
2. Convert back to float colors by dividing by `density`
3. Average the colors: `color = sum / density`

**Code:**
```wgsl
// Read histogram values (accumulate.wgsl lines 33-40)
let base_idx = pixel_idx * 4u;
let r_sum = f32(histogram[base_idx + 0u]);
let g_sum = f32(histogram[base_idx + 1u]);
let b_sum = f32(histogram[base_idx + 2u]);
let density = f32(histogram[base_idx + 3u]);

// Convert back to float color (accumulate.wgsl lines 44-56)
var new_color = vec3<f32>(0.0);
if (density > 0.0) {
    new_color = vec3<f32>(
        r_sum / density,
        g_sum / density,
        b_sum / density
    );
    new_color = clamp(new_color, vec3<f32>(0.0), vec3<f32>(1.0));
}
```

**Math Verification:**
- Encode: `r_u32 = u32(color.r × scale)`
- Multiple hits: `r_sum = Σ(color.r × scale)` and `density = Σ(scale)`
- Decode: `color.r = r_sum / density = Σ(color.r × scale) / Σ(scale) = Σ(color.r) / N`
- Result: Mathematically correct average (scale cancels out)

**Output:** RGB color (0.0-1.0) averaged over all hits in this batch

---

### Stage 4: Accumulation Blending (Accumulate Shader)

**Location:** `shaders/accumulate.wgsl` (lines 59-78)

**Process:**
1. Blend new batch colors with previous accumulation buffer
2. Use weighted average based on sample counts
3. Apply adaptive smoothing for low-density pixels

**Code:**
```wgsl
// Blend RGB with previous accumulation (accumulate.wgsl lines 63-77)
var rgb_accumulated = prev.rgb;
if (density > 0.0) {
    // Adaptive blending based on accumulated density to reduce low-density noise
    // Low-density pixels (prev.a ≈ 0) get reduced blend weight to suppress noise
    // High-density pixels (prev.a >> 0) use full blend_factor for accurate accumulation

    // Smoothing factor: 0.0 = no smoothing, 1.0 = maximum smoothing
    let density_threshold = 0.1; // Density at which full blending is reached
    let density_factor = mix(1.0, min(prev.a / density_threshold, 1.0), params.low_density_smoothing);

    let adjusted_blend = params.blend_factor * density_factor;
    rgb_accumulated = prev.rgb * (1.0 - adjusted_blend) + new_color * adjusted_blend;
}

// Alpha (density) accumulates additively
let alpha_accumulated = prev.a + (density * 0.01 * params.blend_factor);
```

**Parameters:**
- `blend_factor = samples_this_batch / total_samples_accumulated`
- `low_density_smoothing`: 0.0 = no smoothing, 1.0 = maximum smoothing (default 0.5)

**Adaptive Smoothing Logic:**
- When `prev.a` (accumulated density) is low, reduce `adjusted_blend` to suppress noise
- When `prev.a` is high (> 0.1), use full `blend_factor` for accurate accumulation
- This reduces "sparkle" artifacts from single random hits in dark areas

**Trade-offs:**
- **No smoothing (0.0):** Accurate but visible noise in sparse areas
- **Max smoothing (1.0):** Smooth but slower convergence in sparse areas

**Output:** Rgba16Float accumulation buffer (RGB colors + alpha density)

---

### Stage 5: Tone Mapping (Tonemap Shader)

**Location:** `shaders/tonemap.wgsl` (lines 43-94)

**Process:**
1. Load RGB color and density from accumulation buffer
2. Apply density scaling (log or linear based on mode)
3. Apply tone curve (optional S-curve adjustment)
4. Apply exposure and gamma correction
5. Blend with background color based on alpha

**Code:**
```wgsl
// Load from accumulation buffer (tonemap.wgsl line 43)
let acc = textureLoad(accumulation_texture, pixel, 0);
let color = acc.rgb;
let density = acc.a;

// Density scaling based on tonemap mode (lines 46-60)
var scaled_density: f32;
if (params.tonemap_mode == 0u) {
    // Linear mode
    scaled_density = density * params.density_scale;
} else {
    // Logarithmic mode (default)
    // Maps density exponentially: log(1 + density × scale) / log(1 + scale)
    scaled_density = log(1.0 + density * params.density_scale) / log(1.0 + params.density_scale);
}

// Tone curve (optional S-curve, lines 63-72)
var final_density = scaled_density;
if (params.use_curve) {
    // Look up from curve LUT texture
    let curve_value = textureSampleLevel(curve_lut, curve_sampler, vec2<f32>(scaled_density, 0.5), 0.0).r;
    final_density = curve_value;
}

// Exposure and gamma correction (lines 75-77)
var final_color = color * params.exposure;
final_color = pow(final_color, vec3<f32>(1.0 / params.gamma));

// Background blending (lines 80-88)
let background = vec3<f32>(params.background_r, params.background_g, params.background_b);
let alpha = clamp(final_density, 0.0, 1.0);
let blended = final_color * alpha + background * (1.0 - alpha);
```

**Parameters:**
- `density_scale`: Controls overall brightness (default 1.0)
- `tonemap_mode`: Linear (0) or Logarithmic (1, default)
- `use_curve`: Enable/disable S-curve adjustment (default true)
- `exposure`: Brightness multiplier (default 1.0)
- `gamma`: Gamma correction (default 2.2)
- `background_color`: RGB background (default [0, 0, 0])

**Trade-offs:**
- **Linear mode:** Direct density mapping, can wash out bright areas
- **Logarithmic mode:** Compresses high densities, shows more detail in bright areas
- **Tone curve:** S-curve enhances contrast but can clip extremes

**Output:** Final RGBA8 display pixels

---

## Data Flow Summary

```
Iteration Color (float RGB)
    ↓ (Stage 1: Color Assignment)
Per-Iteration Color (0.0-1.0)
    ↓ (Stage 2: U32 Histogram Accumulation) - Updated 2025-10-27
    ↓ - Scaled by histogram_color_scale
    ↓ - Converted to u32 (minimal quantization)
    ↓ - Atomically summed (4 separate channels)
U32 Histogram Buffer (R, G, B, Density as u32)
    ↓ (Stage 3: Histogram Decoding)
    ↓ - Divided by density (scale cancels out)
    ↓ - Averaged colors
Batch Average Color (float RGB)
    ↓ (Stage 4: Accumulation Blending)
    ↓ - Blended with previous accumulation
    ↓ - Adaptive smoothing applied
    ↓ - Density accumulated additively
Accumulation Buffer (Rgba16Float: RGB + alpha density)
    ↓ (Stage 5: Tone Mapping)
    ↓ - Density scaled (log or linear)
    ↓ - Tone curve applied (optional)
    ↓ - Exposure and gamma correction
    ↓ - Background blending
Final Display (RGBA8 pixels)
```

---

## Key Parameters and Their Effects

### histogram_color_scale (1.0-1000.0, default 100.0) - Updated 2025-10-27

**Purpose:** Controls color precision in u32 histogram accumulation

**Effect on Quality:**
- **Precision:** Higher scale = finer color gradations (1000 = 0.1% precision)
- **Quantization:** Lower scale = coarser steps (10 = 10% precision)
- **Overflow:** No longer a practical concern with u32 (4.2 billion max)

**New Behavior:**
- With u32 histogram, overflow is effectively eliminated
- Can use much higher scales for better precision
- Only limited by total accumulated value (4.2 billion)

**Recommended Settings:**
- **Default:** 100.0 (good precision, plenty of headroom)
- **High precision:** 1000.0 (0.1% color steps)
- **Fast convergence:** 50.0 (lower precision but faster)

**No longer need low values** - overflow protection not required with u32

---

### low_density_smoothing (0.0-1.0, default 0.5)

**Purpose:** Reduces noise in sparse/dark areas caused by statistical variance

**Effect on Quality:**
- **Noise Reduction:** Higher smoothing suppresses "sparkle" artifacts
- **Convergence Speed:** Lower smoothing converges faster
- **Accuracy:** Lower smoothing is more mathematically accurate

**Algorithm:**
- `density_factor = mix(1.0, min(prev.a / 0.1, 1.0), smoothing)`
- Low-density pixels (prev.a < 0.1): Reduced blend weight
- High-density pixels (prev.a >= 0.1): Full blend weight

**Recommended Settings:**
- **Accurate but noisy:** 0.0 (no smoothing)
- **Balanced:** 0.5 (default)
- **Smooth but slow:** 1.0 (maximum smoothing)

---

### density_scale (adjustable, default 1.0)

**Purpose:** Controls overall brightness in tone mapping

**Effect on Quality:**
- **Brightness:** Higher scale makes image brighter
- **Dynamic Range:** Affects how density maps to visible brightness
- **Works with:** Tonemap mode (log vs linear)

**Note:** This is applied AFTER accumulation, so it doesn't affect color accuracy

---

### tonemap_mode (Linear or Logarithmic)

**Purpose:** Controls how density maps to brightness

**Effect on Quality:**
- **Linear:** Direct mapping, can wash out bright areas
- **Logarithmic (default):** Compresses high densities, shows more detail

**Formula (Logarithmic):**
- `scaled_density = log(1 + density × scale) / log(1 + scale)`

---

## Potential Quality Regressions to Investigate - Updated 2025-10-27

### 1. Color Quantization from histogram_color_scale

**Status:** ✅ **SIGNIFICANTLY IMPROVED** with u32 histogram

**Previous Issue (u16):** Lower histogram_color_scale values caused severe banding
- Scale 10: RGB(0.537, 0.824, 0.193) → (0.5, 0.8, 0.1) = 6.9% error
- Scale 100: RGB(0.537, 0.824, 0.193) → (0.53, 0.82, 0.19) = 0.7% error

**Current Status (u32):**
- Can now use scale=100 (default) or higher without overflow concerns
- Scale 100: ~1% precision, plenty of headroom (42M hits before overflow)
- Scale 1000: ~0.1% precision, still 4.2M hits before overflow

**Remaining Consideration:**
- Quantization still exists at integer boundaries
- Higher scales reduce quantization error proportionally
- Recommend scale ≥ 100 for smooth gradients

**Code Reference:** `shaders/core/main_2d.wgsl` lines 79-81, `shaders/accumulate.wgsl` lines 49-52

---

### 2. Low-Density Smoothing Reducing Accuracy

**Issue:** Adaptive blending may over-smooth sparse areas

**Example:**
- Sparse pixel gets 1 hit: color should update immediately
- With smoothing=1.0: Update is heavily suppressed until density builds
- Result: Slower convergence, delayed color appearance

**Investigation Needed:**
- Compare convergence speed at smoothing=0.0 vs 1.0
- Measure time to reach target quality in sparse areas
- Visual comparison of sparse region detail

**Code Reference:** `shaders/accumulate.wgsl` lines 66-77

---

### 3. Histogram Overflow Causing Color Wrapping

**Status:** ✅ **ELIMINATED** with u32 histogram (2025-10-27)

**Previous Issue (u16):**
- RGB channels overflow after ~1,310 hits at scale=50
- Wrapping: 70000 → 4464 (70000 - 65536)
- Result: Bright areas suddenly turn dark (severe visual artifact)

**Current Status (u32):**
- **Overflow eliminated** - U32 max is 4.2 billion
- At scale=100: Can accumulate 42.9 million hits per pixel
- Time to overflow: ~91 minutes of continuous full-screen rendering
- **Practical result:** Overflow is no longer a concern

**Verification:**
- Tested with extreme zoom (10,000× iterations per pixel)
- Bright areas stay bright (proper HDR behavior)
- Colors accumulate correctly to their true high values

**Code Reference:** `shaders/core/main_2d.wgsl` lines 88-90 (u32 atomicAdd)

---

### 4. Batched Accumulation Affecting Color Mixing

**Issue:** With batch_size=4, fewer accumulation passes may affect quality

**Example:**
- Old: 1000 accumulations with 1 sample each
- New: 250 accumulations with 4 samples each
- Difference: Fewer blend operations, different statistical behavior

**Investigation Needed:**
- Compare batch_size=1 vs batch_size=4 for visual quality
- Measure color variance in low-density regions
- Test if blend_factor scaling is correct

**Code Reference:** `src/renderer/compute_kernel.rs` lines 176-179 (blend_factor calculation)

---

### 5. Logarithmic Tone Mapping Compressing Colors

**Issue:** Log scaling may reduce color separation in bright areas

**Example:**
- Density 1.0: log(1 + 1.0) / log(2.0) = 1.0
- Density 10.0: log(1 + 10.0) / log(11.0) = 1.0 (compressed!)
- Result: High densities all map to similar brightness

**Investigation Needed:**
- Compare Linear vs Logarithmic mode for color preservation
- Test with wide dynamic range scenes
- Measure color separation in bright vs dark areas

**Code Reference:** `shaders/tonemap.wgsl` lines 54-60

---

## Testing Recommendations

### Visual Regression Tests

1. **Gradient Palette Test:**
   - Use smooth gradient palette
   - Render at multiple histogram_color_scale values
   - Measure color banding artifacts

2. **Sparse Region Test:**
   - Render with low iterations_per_thread
   - Compare low_density_smoothing values
   - Measure noise variance in dark areas

3. **High Density Test:**
   - Render zoomed-out scenes at high iterations
   - Test for histogram overflow (sudden color shifts)
   - Compare different histogram_color_scale values

4. **Convergence Speed Test:**
   - Measure iterations to reach stable color in sparse areas
   - Compare smoothing=0.0 vs smoothing=1.0
   - Quantify convergence delay

### Quantitative Metrics

1. **Color Accuracy:**
   - RMSE between ideal color and quantized color
   - Measure at different histogram_color_scale values

2. **Noise Variance:**
   - Standard deviation of pixel values in sparse regions
   - Compare across low_density_smoothing values

3. **Overflow Frequency:**
   - Count pixels with suspected overflow (sudden color changes)
   - Measure at different iteration counts

4. **Convergence Rate:**
   - Iterations required to reach 95% of final color
   - Compare different smoothing settings

---

## Code References

### Main Pipeline Files

- **Stage 1:** `shaders/core/main_2d.wgsl` lines 40-65 (color assignment)
- **Stage 2:** `shaders/core/main_2d.wgsl` lines 75-90 (histogram accumulation)
- **Stage 3:** `shaders/accumulate.wgsl` lines 28-57 (histogram decoding)
- **Stage 4:** `shaders/accumulate.wgsl` lines 59-78 (accumulation blending)
- **Stage 5:** `shaders/tonemap.wgsl` lines 43-94 (tone mapping)

### Parameter Management

- **Config:** `src/config.rs` lines 30-35 (histogram_color_scale, low_density_smoothing)
- **GPU Buffers:** `src/gpu/buffers.rs` lines 172-184 (AccumulateParams struct)
- **Renderer:** `src/renderer/compute_kernel.rs` lines 25-26 (FlameRenderer fields)
- **UI Controls:** `src/ui/settings.rs` lines 202-235 (sliders)

### 3D Shader Variants

- **Stage 1:** `shaders/core/main_3d.wgsl` (similar to main_2d.wgsl)
- **Stage 2:** `shaders/core/main_3d.wgsl` (same histogram accumulation)
- Uses vec3 instead of vec2, includes Z coordinate handling

---

## Historical Context

### Batched Accumulation (batch_size=4)

**Commit:** experiment/batched-accumulation branch

**Motivation:** Reduce GPU overhead by processing multiple frames before accumulation

**Changes:**
- Histogram cleared every 4 frames instead of every frame
- Accumulation pass runs every 4 frames
- `blend_factor` scaled by batch size

**Performance Gain:** ~3.28× speedup (25.08 Giter/sec)

**Quality Issues Introduced:**
- Alpha blending bug (fixed with blend_factor scaling)
- Low-density noise (addressed with low_density_smoothing)
- Potential color mixing differences (under investigation)

### U16 Packed Histogram (2025-10-26, Replaced)

**Commit:** ce58657

**Motivation:** Use atomic operations for histogram accumulation

**Changes:**
- Switched from f16 to u16 fixed-point
- Pack 4× u16 into 2× u32 for atomic operations
- Scale colors by histogram_color_scale

**Performance Gain:** 13.8% improvement over textureStore

**Quality Issues Introduced:**
- Color quantization (controlled by histogram_color_scale)
- **Overflow wrapping** (bright areas wrap to dark - severe artifact)

**Why Replaced:** Overflow was unacceptable, required u32 upgrade

---

### U32 Unpacked Histogram (2025-10-27, Current)

**Commit:** a8301de

**Motivation:** Eliminate overflow while maintaining performance

**Changes:**
- Switched from u16 to u32 (no packing)
- 4× u32 per pixel: separate R, G, B, Density channels
- Increased histogram_color_scale default from 10 to 100

**Performance Cost:** 2.4% slower than u16 packed (acceptable)

**Quality Improvements:**
- ✅ Overflow eliminated (4.2 billion max vs 65k)
- ✅ Proper HDR behavior (bright stays bright)
- ✅ Clean codebase (no failed optimization attempts)
- ✅ Can use higher scales for better precision

**Memory Cost:** 33% larger histogram (3→4 words per pixel)
- 1920×1080: ~31.5 MB (was ~23.6 MB)
- 800×600: ~9.2 MB (was ~6.9 MB)

**Trade-off Verdict:** **Worth it** - Visual quality > 2.4% performance

---

## Recommendations for Quality Investigation ✅ COMPLETE

Investigation completed 2025-10-26. See [HISTOGRAM_INVESTIGATION_SUMMARY.md](../archive/histogram/HISTOGRAM_INVESTIGATION_SUMMARY.md) for results.

Key findings:
1. ✅ Baseline established (main branch, ef0cdd8)
2. ✅ Parameters tested and analyzed
3. ✅ Quality regression identified (color_scale 10000 → 10)
4. ✅ Recommendations provided (u8 packing or increase defaults)
5. ✅ All documentation updated

**Next Steps:** Implement u8 packing or update defaults before merge.

---

## Future Improvements

1. **Higher Precision Histogram:** Use u32 or f32 instead of u16 (at performance cost)

2. **Adaptive Scaling:** Dynamically adjust histogram_color_scale based on density

3. **Overflow Detection:** Detect and handle histogram overflow explicitly

4. **Improved Smoothing:** Better adaptive algorithms for low-density regions

5. **Quality Metrics:** Built-in image quality measurement tools

6. **A/B Testing:** In-app comparison mode for parameter tuning
