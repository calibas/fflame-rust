# Histogram Accumulation Algorithm Evolution

## Overview

This document tracks the evolution of the histogram color accumulation system, documenting each algorithm change, the motivation behind it, and the quality/performance impact.

---

## Timeline of Changes

### Phase 1: Original Atomic Float Accumulation (Pre-Histogram)

**Status:** Before histogram experiments

**Algorithm:**
- Direct atomic operations on f32 RGBA values
- Each iteration atomically adds color to texture
- Simple and straightforward

**Code:** Not preserved in current branch

**Performance:** Baseline (slower due to atomic float operations)

**Quality:** High quality, mathematically accurate

**Issues:**
- Atomic float operations are slow on GPU
- Limited throughput for high iteration counts

---

### Phase 2: F16 Packed Histogram Attempt

**Commit:** Early experiment (not in main history)

**Algorithm:**
- Tried packing colors as IEEE 754 f16 floats
- Used `pack2x16float()` to create bit patterns
- Atomic add on the packed bits

**Code Reference:** Mentioned in commit message but not preserved

**Result:** **FAILED** ❌

**Root Cause:**
> "pack2x16float creates IEEE 754 float bit patterns, but atomicAdd performs INTEGER addition, corrupting the float representation"

**Example Corruption:**
- Color 0.5 as f16: bit pattern `0x3800`
- Adding bit patterns: `0x3800 + 0x3800 = 0x7000`
- Result interprets as garbage, not 1.0

**Lesson Learned:** Cannot mix float representation with integer arithmetic

---

### Phase 3: U16 Fixed-Point Histogram

**Commit:** Early on experiment/batched-accumulation branch

**Algorithm:**
- Convert float colors (0.0-1.0) to u16 fixed-point integers
- Scale: `r16 = u32(color.r × 100.0)` (hardcoded scale)
- Pack: `packed_rg = r16 | (g16 << 16u)`
- Atomic add on u32 packed values (integer addition works correctly)
- Unpack and divide to get average color

**Code:**
```wgsl
// Encode (compute shader)
let r16 = u32(clamp(final_color.r, 0.0, 1.0) * 100.0);
let g16 = u32(clamp(final_color.g, 0.0, 1.0) * 100.0);
let b16 = u32(clamp(final_color.b, 0.0, 1.0) * 100.0);

// Decode (accumulate shader)
let color_scale = 100.0;
new_color.r = r_sum / (density * color_scale);
```

**Performance:** 13.8% improvement over atomic floats

**Quality:** Good, but limited by fixed scale

**Issues:**
- Hardcoded scale=100 caused overflow at high densities
- Only 655 hits before overflow (65535 / 100 = 655)
- Visible "blown highlights" in zoomed-out scenes

---

### Phase 4: Batched Accumulation (batch_size=4)

**Commits:** Multiple commits on experiment/batched-accumulation branch

**Algorithm Change:**
- Process 4 frames before accumulating to histogram
- Clear histogram every 4 frames instead of every frame
- Run accumulate pass every 4 frames
- Scale `blend_factor` to account for batch size

**Code Reference:** `src/app/mod.rs` line 62 (`accumulation_batch_size: 4`)

**Performance:** 3.28× speedup (25.08 Giter/sec)

**Quality Issues Introduced:**
1. **Alpha Blending Bug:** 4× excessive brightness (FIXED with blend_factor scaling)
2. **Low-Density Noise:** Single random hits very visible in dark areas
3. **Color Mixing Differences:** Fewer accumulation passes may affect quality (UNDER INVESTIGATION)

**Motivation:** Reduce GPU overhead by batching work

---

### Phase 5: User-Configurable histogram_color_scale

**Commit:** `a353a73` "FEAT: Add user-configurable histogram_color_scale parameter"

**Motivation:** **FIX HISTOGRAM OVERFLOW**, not improve quality

**Problem Being Solved:**
> "With color_scale=10 and batch_size=4: 6553 / 4 = 1638 hits per pixel before overflow"
> "User identified overflow artifacts in dense areas at different zoom levels"

**Algorithm Change:**
- Made `histogram_color_scale` a runtime parameter (1.0-100.0, default 10.0)
- Changed from hardcoded 100 to configurable value
- UI slider to adjust trade-off between precision and overflow protection

**Code:**
```wgsl
// Before (hardcoded)
let color_scale = 100.0;

// After (configurable)
let color_scale = params.histogram_color_scale; // Default 10.0
```

**Quality Impact:**
- **Overflow Protection:** Lower scale prevents blown highlights
- **Color Quantization:** Lower scale causes more color banding
- **Trade-off Exposed:** User can now balance precision vs overflow

**Files Changed:**
- `src/config.rs`: Added `histogram_color_scale` field
- `src/gpu/buffers.rs`: Added to `GpuParams` and `AccumulateParams`
- `shaders/core/header.wgsl`: Added parameter
- `shaders/core/main_2d.wgsl`: Use `params.histogram_color_scale`
- `shaders/core/main_3d.wgsl`: Use `params.histogram_color_scale`
- `shaders/accumulate.wgsl`: Use `params.histogram_color_scale`
- `src/ui/settings.rs`: Added slider (logarithmic, 1.0-100.0)

**Default Changed:** 100.0 → 10.0 for better overflow protection

---

### Phase 6: Low-Density Smoothing

**Commit:** `9ac278a` "FEAT: Add user-configurable low-density smoothing parameter"

**Motivation:** **REDUCE NOISE IN SPARSE AREAS**, not improve color accuracy

**Problem Being Solved:**
> "With batched accumulation, low-density pixels that receive only 1 hit per batch show high variance"
> "Visible 'sparkle' artifacts from single random hits in dark areas"

**Algorithm Change:**
- Added adaptive per-pixel blending based on accumulated density
- Low-density pixels: Reduce blend weight to suppress noise
- High-density pixels: Full blend weight for accuracy

**Code:**
```wgsl
// Before (global blend factor for all pixels)
rgb_accumulated = prev.rgb * (1.0 - params.blend_factor) + new_color * params.blend_factor;

// After (adaptive smoothing)
let density_threshold = 0.1;
let density_factor = mix(1.0, min(prev.a / density_threshold, 1.0), params.low_density_smoothing);
let adjusted_blend = params.blend_factor * density_factor;
rgb_accumulated = prev.rgb * (1.0 - adjusted_blend) + new_color * adjusted_blend;
```

**Quality Impact:**
- **Noise Reduction:** Smoother dark areas, less sparkle
- **Convergence Speed:** Slower convergence in sparse areas (trade-off)
- **Accuracy:** Less mathematically pure at high smoothing values

**Files Changed:**
- `src/config.rs`: Added `low_density_smoothing` field (default 0.5)
- `src/gpu/buffers.rs`: Added to `AccumulateParams` (with std140 padding)
- `shaders/accumulate.wgsl`: Implemented adaptive blending
- `src/ui/settings.rs`: Added slider (0.0-1.0)

**Default:** 0.5 (moderate smoothing)

---

## Quality Baseline Commits (Per User Feedback)

### Commit ef0cdd8: "FIX: Correct density accumulation in histogram shader"

**User Feedback:** "The code at commits ce58657 and ef0cdd8 appear to have improved quality"

**Algorithm at ef0cdd8:** (GOOD QUALITY BASELINE)

**Compute Shader (main_2d.wgsl):**
```wgsl
// 4 separate atomic operations per pixel
let color_scale = 10000.0;  // Hardcoded
let r = u32(clamp(final_color.r, 0.0, 1.0) * color_scale);
let g = u32(clamp(final_color.g, 0.0, 1.0) * color_scale);
let b = u32(clamp(final_color.b, 0.0, 1.0) * color_scale);

atomicAdd(&histogram[base_idx + 0u], r);  // R channel
atomicAdd(&histogram[base_idx + 1u], g);  // G channel
atomicAdd(&histogram[base_idx + 2u], b);  // B channel
atomicAdd(&histogram[base_idx + 3u], 1u); // Density
// 4 atomics per pixel, 4× u32 per pixel = 16 bytes
```

**Accumulate Shader (accumulate.wgsl):**
```wgsl
// Simple global blend factor for all pixels
let color_scale = 10000.0;  // Hardcoded
let r_sum = f32(histogram[base_idx + 0u]);
let g_sum = f32(histogram[base_idx + 1u]);
let b_sum = f32(histogram[base_idx + 2u]);
let density = f32(histogram[base_idx + 3u]);

new_color = vec3<f32>(
    r_sum / (density * color_scale),
    g_sum / (density * color_scale),
    b_sum / (density * color_scale)
);

// Simple blend - ALL pixels use same blend_factor
rgb_accumulated = prev.rgb * (1.0 - params.blend_factor) + new_color * params.blend_factor;

// Simple density accumulation
alpha_accumulated = prev.a + (density * 0.01);
```

**Characteristics:**
- ✅ High precision (scale=10000, ~10000 color levels)
- ✅ No adaptive smoothing (mathematically pure)
- ✅ Simple, straightforward algorithm
- ✅ 4 atomic operations per pixel
- ❌ Slower performance (more atomic ops)
- ❌ Would overflow at ~6.5 hits with scale=10000 (but acceptable in practice)

**Performance:** Baseline before packing optimization

**Quality:** GOOD (user confirmed)

---

## What Changed Since ef0cdd8 (Good Quality Baseline)

### Summary of All Changes

Between ef0cdd8 (good quality) and HEAD (current), the following changes were made:

### 1. U16 Packing (Performance Optimization)

**Motivation:** Reduce atomic operations from 4 to 2 per pixel

**Changes:**
- Pack RGBA into 2× u32 instead of 4× u32
- R+G in first u32: `r16 | (g16 << 16)`
- B+D in second u32: `b16 | (d16 << 16)`
- **Reduced histogram buffer size:** 16 bytes → 8 bytes per pixel
- **Reduced atomic operations:** 4 atomics → 2 atomics per pixel

**Performance Impact:** ~13.8% improvement (measured)

**Quality Impact:** None if scale remains same

---

### 2. Color Scale Reduced: 10000 → 10 (Overflow Fix)

**Motivation:** Fix histogram overflow at high densities

**Changes:**
- **Before:** `color_scale = 10000.0` (hardcoded)
- **After:** `color_scale = 10.0` (default value)
- **Max hits before overflow:** 6.5 → 6553 (1000× improvement!)

**Quality Impact:** **MAJOR REGRESSION** ⚠️
- **Precision loss:** 10000 color levels → 10 color levels (1000× worse!)
- **Visible color banding** due to severe quantization
- Example: color 0.537 → stored as 5 → decoded as 0.5 (error: 0.037)

**Trade-off Exposed:** This change sacrificed precision to fix overflow

---

### 3. Configurable histogram_color_scale (User Control)

**Motivation:** Let user balance precision vs overflow

**Changes:**
- Made scale a runtime parameter (1.0-100.0, default 10.0)
- UI slider added
- User can increase scale for precision (risk overflow)
- User can decrease scale for overflow protection (lose precision)

**Quality Impact:** Neutral (user choice), but default=10 has severe quantization

---

### 4. Adaptive Low-Density Smoothing (Noise Fix)

**Motivation:** Reduce noise in sparse areas from batched accumulation

**Changes:**
- **Before:** All pixels use global `blend_factor`
- **After:** Low-density pixels get reduced blend weight

```wgsl
// Before
rgb_accumulated = prev.rgb * (1.0 - blend_factor) + new_color * blend_factor;

// After
let density_factor = mix(1.0, min(prev.a / 0.1, 1.0), low_density_smoothing);
let adjusted_blend = blend_factor * density_factor;
rgb_accumulated = prev.rgb * (1.0 - adjusted_blend) + new_color * adjusted_blend;
```

**Quality Impact:** **POTENTIAL REGRESSION** ⚠️
- ✅ Reduces sparkle artifacts in dark areas
- ❌ Slower convergence in sparse areas
- ❌ Less mathematically accurate
- ❌ Can be disabled (smoothing=0.0), but default=0.5 enables it

---

### 5. Blend Factor Scaled by Batch Size (Batching Correctness)

**Motivation:** Fix alpha accumulation in batched mode

**Changes:**
- **Before:** `alpha_accumulated = prev.a + (density * 0.01)`
- **After:** `alpha_accumulated = prev.a + (density * 0.01 * blend_factor)`

**Quality Impact:** Correctness fix (brightness was 4× too bright before)

---

### 6. Skip Blending if Density = 0 (Correctness Fix)

**Motivation:** Don't pull low-density areas toward black

**Changes:**
- **Before:** Always blend, even if density=0 (blends with black)
- **After:** Only blend if density > 0

```wgsl
// Before
rgb_accumulated = prev.rgb * (1.0 - blend_factor) + new_color * blend_factor;
// When density=0, new_color=0, so this dims the pixel!

// After
var rgb_accumulated = prev.rgb;  // Default to previous
if (density > 0.0) {
    // Only blend if pixel received hits
    rgb_accumulated = prev.rgb * (1.0 - adjusted_blend) + new_color * adjusted_blend;
}
```

**Quality Impact:** Correctness fix (was causing grey fog in dark areas)

---

## Direct Comparison: ef0cdd8 vs HEAD

| Aspect | ef0cdd8 (Good Quality) | HEAD (Current) |
|--------|------------------------|----------------|
| **Atomic Ops** | 4 per pixel | 2 per pixel (2× faster) |
| **Color Scale** | 10000 (hardcoded) | 10 (default, configurable) |
| **Color Precision** | ~10000 levels | ~10 levels (1000× worse!) |
| **Max Hits Before Overflow** | ~6.5 | ~6553 (1000× better) |
| **Adaptive Smoothing** | No (pure average) | Yes (default 0.5) |
| **Blending Logic** | Always blend | Conditional blend |
| **Alpha Scaling** | Missing (bug) | Fixed with blend_factor |
| **Performance** | Baseline | ~13.8% faster |
| **Quality** | GOOD ✅ | **REGRESSED** ❌ (due to scale=10) |

---

## The Core Quality Regression

**Root Cause:** Changing `color_scale` from 10000 to 10

**Why it was changed:** To fix histogram overflow

**The problem:**
1. U16 can only hold values 0-65535
2. With scale=10000, overflow happens at: 65535 / 10000 = 6.5 hits
3. Zoomed-out scenes have hundreds/thousands of hits per pixel
4. Result: Blown highlights (overflow artifacts)

**The solution chosen:**
- Reduce scale to 10 (overflow at 6553 hits)
- **Side effect:** Severe color quantization (10 levels instead of 10000)

**The trade-off:**
- ✅ Fixed: Overflow in bright areas
- ❌ Introduced: Color banding everywhere
- ⚖️ User can adjust, but default is quantized

---

## Recommended Path to Restore Quality

### Option 1: Increase Default Scale (Accept Overflow Risk)

**Change:** `default_histogram_color_scale() -> f32 { 100.0 }` (currently 10.0)

**Result:**
- 100 color levels (10× better than current)
- Overflow at 655 hits (still reasonable for most scenes)
- User warned about overflow, can lower if needed

**Trade-off:** May see overflow in extreme zoom-out cases

---

### Option 2: Disable Adaptive Smoothing by Default

**Change:** `default_low_density_smoothing() -> f32 { 0.0 }` (currently 0.5)

**Result:**
- Mathematically pure blending (like ef0cdd8)
- Faster convergence
- May see sparkle in dark areas with batched accumulation

**Trade-off:** Noisier sparse regions

---

### Option 3: Return to 4 Atomic Ops (Accept Performance Loss)

**Change:** Revert u16 packing, use 4× u32 histogram like ef0cdd8

**Result:**
- Can use scale=10000 without overflow concerns (u32 holds up to 4.2 billion)
- Perfect color accuracy
- Simple algorithm

**Trade-off:** ~13.8% slower performance

---

### Option 4: Use U32 Histogram (Best Quality)

**Change:** Pack RGBA into 1× u32 using u8 (0-255) per channel

**Result:**
- 256 color levels (26× better than current default)
- Overflow at 16.7M hits (essentially never)
- Still 2 atomic ops per pixel

**Trade-off:** Slightly more complex bit manipulation

**Implementation:**
```wgsl
// Pack 4× u8 into 1× u32
let r8 = u32(clamp(final_color.r, 0.0, 1.0) * 255.0);
let g8 = u32(clamp(final_color.g, 0.0, 1.0) * 255.0);
let b8 = u32(clamp(final_color.b, 0.0, 1.0) * 255.0);
let packed_rgba = r8 | (g8 << 8u) | (b8 << 16u) | (255u << 24u);

// Separate u32 for density (no overflow until 4.2B hits)
atomicAdd(&histogram[base_idx + 0u], packed_rgba);
atomicAdd(&histogram[base_idx + 1u], 1u);  // density
```

---

## Current State (Latest Commits)

### Commit a353a73: histogram_color_scale Parameter

**Purpose:** Fix histogram overflow (not improve quality)

**Trade-off:** Overflow protection vs color precision

**Quality Impact:**
- ✅ Fixed: Blown highlights at high densities
- ❌ Introduced: Color quantization at low scales
- ⚖️ Default changed: 100 → 10 (favor overflow protection)

### Commit 9ac278a: low_density_smoothing Parameter

**Purpose:** Reduce low-density noise (not improve color accuracy)

**Trade-off:** Noise reduction vs convergence speed

**Quality Impact:**
- ✅ Fixed: Sparkle artifacts in dark areas
- ❌ Introduced: Slower convergence, less accuracy
- ⚖️ Default: 0.5 (moderate smoothing)

---

## Algorithm Comparison Table

| Algorithm | Scale | Overflow Threshold | Color Precision | Quality Notes |
|-----------|-------|-------------------|----------------|---------------|
| **Original Atomic Float** | N/A | No overflow | Perfect (f32) | High quality baseline |
| **F16 Packed (Failed)** | N/A | N/A | Corrupted | Did not work |
| **U16 Fixed (scale=100)** | 100.0 | 655 hits | 1% (100 levels) | Good but overflows easily |
| **U16 Fixed (scale=10, default)** | 10.0 | 6553 hits | 10% (10 levels) | Overflow protected, color banding |
| **U16 Fixed (scale=1, min)** | 1.0 | 65535 hits | 100% (1-2 levels) | Maximum protection, severe banding |

**Formula:**
- `max_hits = 65535 / histogram_color_scale`
- `color_levels = histogram_color_scale`

---

## Previous Algorithms (For Reference)

### Algorithm 1: Global Blend Factor (Before Smoothing)

**Code Location:** `shaders/accumulate.wgsl` (before commit 9ac278a)

```wgsl
// Blend RGB with previous accumulation
var rgb_accumulated = prev.rgb;
if (density > 0.0) {
    rgb_accumulated = prev.rgb * (1.0 - params.blend_factor) + new_color * params.blend_factor;
}
```

**Characteristics:**
- All pixels use same `blend_factor` regardless of density
- Mathematically pure weighted average
- High noise variance in low-density pixels

**Restored by:** Set `low_density_smoothing = 0.0`

---

### Algorithm 2: Hardcoded Color Scale (Before Configurable)

**Code Location:** `shaders/core/main_2d.wgsl` (before commit a353a73)

```wgsl
// Hardcoded scale
let color_scale = 100.0; // Was this 100 or 10? Need to check git history
let r16 = u32(clamp(final_color.r, 0.0, 1.0) * color_scale);
```

**Characteristics:**
- Fixed precision/overflow trade-off
- No user control
- Caused overflow in high-density scenes

**Restored by:** Set `histogram_color_scale = 100.0` (if that was the original value)

---

## Git History Investigation Needed

### 1. Find Original Hardcoded Scale Value

```bash
git log --all --oneline -- shaders/core/main_2d.wgsl | head -20
git show <commit>:shaders/core/main_2d.wgsl | grep "color_scale"
```

**Goal:** Determine if original was 100.0 or different value

---

### 2. Analyze Quality-Improving Commits

```bash
# Check ce58657
git show ce58657

# Check ef0cdd8 (density accumulation fix)
git show ef0cdd8
```

**Goal:** Understand what changes actually improved quality

---

### 3. Compare Before/After Batching

```bash
# Find commit before batched accumulation
git log --all --oneline --grep="batch" | tail -5

# Find main branch commit before experiments
git log main --oneline -10
```

**Goal:** Establish baseline for quality comparison

---

## Testing Recommendations

### 1. Reproduce Original Quality

**Test:** Render same scene with different settings

```
A) Original Float Atomic (if possible to revert)
B) U16 scale=100, smoothing=0.0 (closest to original U16)
C) U16 scale=10, smoothing=0.5 (current default)
D) U16 scale=10, smoothing=0.0 (no smoothing)
```

**Compare:** Visual quality, color accuracy, noise levels

---

### 2. Measure Quantization Error

**Test:** Render gradient palette

```bash
# Different scales
--histogram-color-scale 1    # Maximum banding
--histogram-color-scale 10   # Default
--histogram-color-scale 50   # Medium precision
--histogram-color-scale 100  # High precision
```

**Measure:** Color error (RMSE) from ideal gradient

---

### 3. Measure Convergence Speed

**Test:** Render sparse region at intervals

```
Iterations: 1000, 5000, 10000, 50000, 100000
Settings: smoothing=0.0 vs smoothing=1.0
```

**Measure:** Time to reach stable color (95% of final value)

---

### 4. Test Overflow Threshold

**Test:** Render zoomed-out scene at increasing iterations

```
Iterations: 1M, 5M, 10M, 50M, 100M
Scale: 1.0, 10.0, 100.0
```

**Observe:** When overflow artifacts appear (sudden color shifts)

---

## Known Quality Issues

### 1. Color Quantization (From Low Scale)

**Cause:** `histogram_color_scale = 10.0` provides only ~10 color levels

**Example:**
- Input: RGB(0.537, 0.824, 0.193)
- Encoded: (5, 8, 1) with scale=10
- Decoded: (0.5, 0.8, 0.1)
- Error: Δ(0.037, 0.024, 0.093)

**Severity:** Moderate color banding visible in gradients

**Mitigation:** Increase `histogram_color_scale` (risk overflow)

**User Control:** Yes (slider)

---

### 2. Low-Density Sparkle (From Statistical Variance)

**Cause:** Batched accumulation with global blend factor

**Example:**
- Pixel gets 1 hit in batch of 4 frames
- `blend_factor = 0.25` (large weight for single sample)
- Result: Bright pixel appears suddenly in dark area

**Severity:** Very noticeable in dark/sparse regions

**Mitigation:** Enable `low_density_smoothing` (default 0.5)

**User Control:** Yes (slider)

**Trade-off:** Slower convergence in sparse areas

---

### 3. Histogram Overflow (From High Scale)

**Cause:** U16 overflow when accumulated value exceeds 65535

**Example:**
- `histogram_color_scale = 100`
- Pixel receives 700 hits at full red
- Accumulation: 700 × 100 = 70000
- Overflow: 70000 - 65536 = 4464 (wraps around)
- Decoded: 4464 / 700 / 100 = 0.063 (dark red instead of bright)

**Severity:** Sudden color corruption in very bright areas

**Mitigation:** Decrease `histogram_color_scale` (lose precision)

**User Control:** Yes (slider)

**Trade-off:** Color quantization

---

## Open Questions

1. **What was the original hardcoded scale value?** (100? 10? other?)
   - Need to check git history before configurable parameter

2. **What changes in ce58657 and ef0cdd8 improved quality?**
   - User reported these commits as quality improvements
   - Need to analyze diffs

3. **Is the default scale=10 the right choice?**
   - Balances overflow vs precision
   - But causes visible color banding
   - Should default be higher (50?) with warnings about overflow?

4. **Does batched accumulation fundamentally change color mixing?**
   - Fewer blend operations (250 vs 1000 for same samples)
   - Different statistical behavior
   - Need rigorous testing to confirm

5. **Is low-density smoothing worth the accuracy trade-off?**
   - Reduces noise but slows convergence
   - Mathematically less accurate
   - User preference? Scene-dependent?

---

## Recommendations

### 1. Document Original Baseline

- Find and document the exact algorithm before experiments
- Render reference images for comparison
- Establish quality metrics baseline

### 2. Systematic Quality Testing

- Test each parameter independently
- Measure quantitative metrics (RMSE, variance, convergence)
- Gather subjective visual assessments

### 3. Consider Alternative Approaches

- **Higher Precision Histogram:** Use u32 or f32 (performance cost)
- **Overflow Detection:** Detect and clamp instead of wrapping
- **Adaptive Scaling:** Auto-adjust scale based on density
- **Hybrid Approach:** Different algorithms for sparse vs dense regions

### 4. User Education

- Document trade-offs clearly in UI tooltips
- Provide presets for different use cases
- Add visual feedback for overflow/quantization issues

### 5. Preserve Quality Options

- Keep ability to disable smoothing (smoothing=0.0)
- Keep ability to use high precision (scale=100)
- Don't force users into "performance mode" if they want quality

---

## Conclusion

**Recent changes were NOT about improving quality - they were about:**

1. **Fixing Overflow:** `histogram_color_scale` parameter to prevent blown highlights
2. **Reducing Noise:** `low_density_smoothing` parameter to suppress sparkle artifacts

**Both changes introduce new trade-offs:**

- Lower scale → Better overflow protection, but more color quantization
- Higher smoothing → Less noise, but slower convergence and less accuracy

**Quality investigation needed to determine:**

- Whether these trade-offs are acceptable
- What the original quality baseline was
- If there are better algorithms that avoid trade-offs entirely

**Next Steps:**

1. Analyze commits ce58657 and ef0cdd8 to understand quality improvements
2. Establish visual baseline from pre-experiment code
3. Quantitative testing of current vs original algorithms
4. Consider alternative approaches if current trade-offs are too severe
