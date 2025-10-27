# Quality Investigation: ce58657, ef0cdd8, and HEAD

## User Observations

> "ce58657 and ef0cdd8 are nearly the same in definition/quality when I benchmark on a fractal"
>
> "ce58657 has slightly more saturated colors than ef0cdd8"
>
> "The most recent commit looks appears worse quality and it's much darker in the benchmarks."
>
> "It may just be the settings. I can adjust things so the quality for the current version appears nearly the same as ce58657 and ef0cdd8. The 'quality' looks like it may just be improved contrast versus the current version."

---

## Key Findings

### 1. ce58657 vs ef0cdd8: Only One Difference

**Single Change:** Alpha accumulation bug fix

**ce58657 (slightly wrong):**
```wgsl
// BUG: Dividing density by color_scale (incorrect)
let color_scale = 100.0;
let alpha_accumulated = prev.a + (density / color_scale * 0.01);
// With scale=100, this adds density/100 * 0.01 = density * 0.0001 per hit
```

**ef0cdd8 (correct):**
```wgsl
// FIXED: Density is not scaled
let color_scale = 10000.0;  // Changed from 100 to 10000 here
let alpha_accumulated = prev.a + (density * 0.01);
// This adds density * 0.01 per hit (correct)
```

**Two changes happened together in ef0cdd8:**
1. Fixed alpha calculation (removed division by color_scale)
2. **Increased color_scale from 100 to 10000** (100× better precision!)

**Why ce58657 had "slightly more saturated colors":**
- Alpha was too low (divided by 100)
- Lower alpha = more transparent = less blending with background
- This can make colors appear more vibrant/saturated

**Algorithm Otherwise Identical:**
- Both use 4 separate atomic operations (NOT packed)
- Both use same color accumulation math
- Both use same blend factor
- **Only differences:** alpha calculation and color_scale value

---

### 2. Tone Mapping: NO CHANGES

**Finding:** Tone mapping shader is **IDENTICAL** across ce58657, ef0cdd8, and HEAD

```bash
git diff ef0cdd8..HEAD -- shaders/tonemap.wgsl
# No output = no changes
```

**Conclusion:** The "improved contrast" you observe in ce58657/ef0cdd8 is NOT from better tone mapping. It's from:
1. Higher color_scale (10000 vs current default 10) = less quantization
2. Incorrect alpha causing different opacity/blending behavior

---

### 3. Why HEAD Appears Darker

**Root Cause:** Default `histogram_color_scale = 10.0` vs ef0cdd8's `10000.0`

**But wait...** color_scale shouldn't affect brightness, only precision!

Let me trace through the math to see why HEAD might be darker:

#### Color Accumulation Math (Should Be Brightness-Neutral)

**Encoding (compute shader):**
```wgsl
// ef0cdd8: scale=10000
let r16 = u32(color.r * 10000.0);  // color 0.5 → 5000

// HEAD: scale=10 (default)
let r16 = u32(color.r * 10.0);     // color 0.5 → 5
```

**Decoding (accumulate shader):**
```wgsl
// ef0cdd8: scale=10000
new_color.r = r_sum / (density * 10000.0);  // 5000 / (1 * 10000) = 0.5 ✓

// HEAD: scale=10
new_color.r = r_sum / (density * 10.0);     // 5 / (1 * 10) = 0.5 ✓
```

**Math checks out - scale should cancel!**

#### Where HEAD Differs (Darkness Sources)

**Possible causes for darkness in HEAD:**

1. **Adaptive Smoothing (default 0.5)**
   - Low-density pixels get reduced blend weight
   - Can cause darker appearance in sparse areas
   - ef0cdd8 had no smoothing (pure blend)

2. **Conditional Blending (density > 0 check)**
   - HEAD only blends if density > 0
   - ef0cdd8 always blended (could blend with black when density=0)
   - This should make HEAD LIGHTER, not darker though

3. **Blend Factor Scaling (batched accumulation fix)**
   - HEAD: `alpha_accumulated = prev.a + (density * 0.01 * blend_factor)`
   - ef0cdd8: `alpha_accumulated = prev.a + (density * 0.01)`
   - With batch_size=4, HEAD accumulates alpha 4× slower
   - Lower alpha = more transparent = darker when blended with black background!

**This is the smoking gun! ☠️**

---

## The Darkness Mystery Solved

### Alpha Accumulation Rate

**ef0cdd8 (4 atomic ops, no batching):**
```wgsl
// Each accumulate pass (every frame):
alpha_accumulated = prev.a + (density * 0.01);
// 1 hit → alpha += 0.01
// 100 hits → alpha += 1.0
```

**HEAD (2 atomic ops, batch_size=4):**
```wgsl
// Each accumulate pass (every 4 frames):
let blend_factor = samples_this_batch / total_samples;
alpha_accumulated = prev.a + (density * 0.01 * blend_factor);

// First batch (4 frames):
//   density = 4 hits
//   blend_factor = 4/4 = 1.0
//   alpha += 4 * 0.01 * 1.0 = 0.04 ✓ (same rate)

// HOWEVER, with lower density:
//   density = 1 hit (sparse area)
//   blend_factor = 4/4 = 1.0
//   alpha += 1 * 0.01 * 1.0 = 0.01 ✓ (still correct)
```

Wait, that's mathematically correct too...

Let me check if it's the smoothing:

### Adaptive Smoothing Effect

**HEAD (with smoothing=0.5, low density):**
```wgsl
// Low-density pixel (prev.a = 0.05):
let density_factor = mix(1.0, min(0.05 / 0.1, 1.0), 0.5);
// density_factor = mix(1.0, 0.5, 0.5) = 0.75

let adjusted_blend = blend_factor * density_factor;
// adjusted_blend is REDUCED for low-density pixels

rgb_accumulated = prev.rgb * (1.0 - adjusted_blend) + new_color * adjusted_blend;
// Less weight on new color = slower to brighten
```

**This is it!** Adaptive smoothing causes low-density areas to brighten more slowly, making the image appear darker overall.

---

## Complete Algorithm Comparison

| Aspect | ce58657 | ef0cdd8 | HEAD (Default) |
|--------|---------|---------|----------------|
| **Atomic Ops** | 4 (unpacked) | 4 (unpacked) | 2 (u16 packed) |
| **Color Scale** | 100 | 10000 | 10 |
| **Color Precision** | 100 levels | 10000 levels | 10 levels |
| **Alpha Calc** | density/100 * 0.01 (BUG) | density * 0.01 (FIXED) | density * 0.01 * blend_factor |
| **Adaptive Smoothing** | No | No | Yes (0.5 default) |
| **Conditional Blend** | No (always blend) | No (always blend) | Yes (if density>0) |
| **Batch Size** | 1 (every frame) | 1 (every frame) | 4 (every 4 frames) |
| **Brightness** | Slightly dark (alpha bug) | Correct | Dark (smoothing) |
| **Saturation** | High (alpha bug artifact) | Correct | Correct |
| **Quality** | Good (high scale) | Best (highest scale) | Poor (low scale) |

---

## Why "You Can Adjust Settings to Match Quality"

User said: *"I can adjust things so the quality for the current version appears nearly the same"*

**What to adjust in HEAD to match ef0cdd8:**

1. **Increase histogram_color_scale to 100** (closer to ef0cdd8's 10000)
   - Reduces color quantization
   - Slider: 10 → 100 (max on slider)
   - Still only 1% of ef0cdd8's precision, but 10× better than default

2. **Disable adaptive smoothing (set to 0.0)**
   - Restores mathematically pure blending
   - Removes darkness in low-density areas
   - Slider: 0.5 → 0.0

3. **Increase density_scale or exposure**
   - Compensates for any remaining darkness
   - These are tone mapping controls (post-accumulation)

**With these settings, HEAD should look very close to ef0cdd8!**

---

## Why ef0cdd8/ce58657 Look Better: Summary

### 1. Higher Precision (Primary Factor)

**ef0cdd8:** scale=10000 → 10000 color levels, no visible banding
**HEAD default:** scale=10 → 10 color levels, severe banding

**Impact:** This is the main quality difference. Colors are quantized 1000× more in HEAD.

### 2. No Adaptive Smoothing

**ef0cdd8:** Pure mathematical blending
**HEAD default:** Low-density pixels suppressed (smoothing=0.5)

**Impact:** ef0cdd8 brightens faster, especially in sparse areas.

### 3. Alpha Calculation (ce58657 vs ef0cdd8)

**ce58657:** Alpha too low (div by 100 bug) → more saturation artifact
**ef0cdd8:** Alpha correct → proper blending

**Impact:** Minor - slight saturation difference between ce58657 and ef0cdd8.

---

## Recommended Settings to Match ef0cdd8 Quality

### Quick Fix (Use UI Sliders)

In Settings → Performance:
1. **Histogram Color Scale:** 10 → 100 (max available)
2. **Low-Density Smoothing:** 0.5 → 0.0 (disable)

In Settings → Display:
3. **Density Scale:** Increase if still too dark
4. **Exposure:** Increase if still too dark

### Proper Fix (Code Changes)

**Option 1: Increase Default Scale**
```rust
// src/config.rs
fn default_histogram_color_scale() -> f32 {
    100.0  // Was 10.0
}
```

**Option 2: Disable Default Smoothing**
```rust
// src/config.rs
fn default_low_density_smoothing() -> f32 {
    0.0  // Was 0.5
}
```

**Option 3: Revert to 4 Atomic Ops (Best Quality)**
- Revert u16 packing
- Use 4× u32 histogram
- Can use scale=10000 without overflow
- Accept 13.8% performance loss

**Option 4: Implement u8 Packing (Recommended)**
- Pack RGBA as 4× u8 in 1× u32
- Separate u32 for density
- 256 color levels (26× better than current default)
- Overflow at 16.7M hits (impossible)
- Same 2 atomic ops as current

---

## Testing Plan

### Verify Findings

1. **Render same fractal with three settings:**
   - A) ef0cdd8 baseline
   - B) HEAD with default (scale=10, smoothing=0.5)
   - C) HEAD adjusted (scale=100, smoothing=0.0)

2. **Compare visual quality:**
   - Color banding/quantization
   - Brightness/darkness
   - Saturation
   - Sparse region noise

3. **Expected results:**
   - A (ef0cdd8): Best quality, no banding, correct brightness
   - B (HEAD default): Visible banding, darker, smoother
   - C (HEAD adjusted): Much better, closer to A, but still some banding (100 vs 10000)

### Quantitative Measurements

1. **Color precision:**
   - Render gradient palette
   - Measure color step size
   - ef0cdd8: ~0.0001 steps
   - HEAD default: ~0.1 steps (1000× coarser!)
   - HEAD adjusted: ~0.01 steps (100× coarser)

2. **Brightness:**
   - Measure average pixel value in sparse regions
   - Compare ef0cdd8 vs HEAD default vs HEAD adjusted

3. **Convergence speed:**
   - Measure iterations to stable color
   - ef0cdd8: Baseline
   - HEAD default: Slower (smoothing)
   - HEAD adjusted: Same as ef0cdd8 (no smoothing)

---

## Conclusion

**The quality difference is primarily due to two factors:**

1. **Color scale reduced 1000×** (10000 → 10)
   - This causes severe color quantization/banding
   - Necessary to fix overflow, but default is too aggressive
   - **Recommendation:** Increase default to 100 (balance quality vs overflow)

2. **Adaptive smoothing enabled by default** (0.0 → 0.5)
   - This suppresses brightness in low-density areas
   - Makes image appear darker overall
   - **Recommendation:** Disable by default (smoothing=0.0)

**Tone mapping is identical - not a factor.**

**Alpha calculation is correct in both ef0cdd8 and HEAD** (was broken in ce58657, hence slight saturation difference).

**User observation is correct:** With settings adjusted (scale=100, smoothing=0.0), HEAD can look very close to ef0cdd8 quality, just with 100× coarser color steps instead of 10000×.

**Best path forward:** Implement u8 packing (Option 4) to get 256 color levels with no overflow risk and same performance as current.

---

## See Also

For complete investigation summary and recommendations, see:
- **[HISTOGRAM_INVESTIGATION_SUMMARY.md](HISTOGRAM_INVESTIGATION_SUMMARY.md)** - Executive summary and recommendations
