# Histogram Investigation Summary

**Date:** 2025-10-26
**Branch:** `experiment/batched-accumulation`
**Status:** Investigation Complete - Recommendations Provided

---

## Executive Summary

This document consolidates findings from a multi-week investigation into histogram-based color accumulation quality and performance. The investigation began with a successful u16 packed histogram optimization on `main` branch, followed by experimental batched accumulation work that introduced quality regressions.

**Key Finding:** Recent changes were **NOT intended to improve quality** - they were **fixes for specific problems** (overflow and noise) that introduced new trade-offs.

---

## Branch History and Current State

### Main Branch (Latest: e7bbf73)
**Algorithm:** u16 Packed Histogram (scale=100, fixed)
- ✅ 4 atomic operations per pixel
- ✅ color_scale = 100 (hardcoded)
- ✅ 100 color levels (1% precision)
- ✅ No adaptive smoothing (pure blending)
- ✅ Single frame accumulation (batch_size=1)
- ✅ 13.8% faster than naive 4-atomic approach
- ✅ **Quality: Good** (validated)

**Merged PRs:**
- #4: f16 packed histogram (failed, then fixed with u16)
- #5: u16 packed histogram optimization (success)

### Experiment Branch (Current: 690c52f)
**Algorithm:** u16 Packed Histogram + Batched Accumulation + User Controls
- ✅ 2 atomic operations per pixel
- ⚠️ color_scale = 10 (default, user-configurable 1-100)
- ⚠️ 10 color levels (10% precision)
- ⚠️ Adaptive smoothing enabled (default 0.5)
- ✅ Batched accumulation (batch_size=4)
- ✅ 3.28× faster throughput (25.08 Giter/sec)
- ❌ **Quality: Regressed** (due to low scale + smoothing)

**New Features:**
1. User-configurable `histogram_color_scale` (commits a353a73, 082d524)
2. User-configurable `low_density_smoothing` (commit 9ac278a)
3. Batched accumulation system (commits abce580, f6e6bdb)

---

## Quality Investigation Results

### Commits Analyzed

#### ef0cdd8: "FIX: Correct density accumulation in histogram shader"
**User Observation:** "Good quality baseline"

**What Changed:**
```wgsl
// Before (ce58657 - WRONG)
let color_scale = 100.0;
let alpha_accumulated = prev.a + (density / color_scale * 0.01);
// Alpha divided by 100 (bug) - causes colors to appear more saturated

// After (ef0cdd8 - FIXED)
let color_scale = 10000.0;  // Also increased scale 100× here!
let alpha_accumulated = prev.a + (density * 0.01);
// Alpha correct, scale much higher (10000 levels)
```

**Two changes in one commit:**
1. **Fixed alpha accumulation bug** (removed incorrect division)
2. **Increased color_scale from 100 to 10000** (100× better precision!)

**Impact:**
- ce58657 had slight saturation artifact from alpha bug
- ef0cdd8 fixed alpha AND provided 10000 color levels
- Result: Near-perfect quality with high precision

#### Current HEAD (690c52f): Multiple Changes Since ef0cdd8

**Changes Made:**
1. ✅ U16 packing (4 atomics → 2 atomics) - **13.8% faster**
2. ⚠️ Color scale reduced (10000 → 10 default) - **1000× precision loss**
3. ✅ Configurable histogram_color_scale - **User control**
4. ⚠️ Adaptive smoothing enabled (0.0 → 0.5 default) - **Slower convergence**
5. ✅ Batched accumulation (batch_size=4) - **3.28× faster**
6. ✅ Blend factor scaling - **Correctness fix**
7. ✅ Conditional blending (density > 0) - **Correctness fix**

### Why HEAD Appears Darker and Lower Quality

**Primary Cause: Color Scale Reduced 1000×**
```
ef0cdd8:  scale=10000 → 10000 color levels, overflow at 6.5 hits
HEAD:     scale=10    → 10 color levels, overflow at 6553 hits
```

**Example Color Quantization:**
```
Input:    RGB(0.537, 0.824, 0.193)
Scale=10: Encoded as (5, 8, 1) → Decoded as (0.5, 0.8, 0.1)
Error:    Δ(0.037, 0.024, 0.093) - VISIBLE BANDING
```

**Secondary Cause: Adaptive Smoothing**
- Default `low_density_smoothing = 0.5`
- Low-density pixels get reduced blend weight
- Makes sparse areas darker and slower to converge
- ef0cdd8 had no smoothing (pure mathematical blending)

**Tertiary Cause: Batched Accumulation**
- With batch_size=4, alpha scaled by blend_factor
- Mathematically correct, but different statistical behavior
- Fewer accumulation passes (250 vs 1000 for same samples)

### Tone Mapping Investigation

**Finding:** Tone mapping is **IDENTICAL** across all versions.

```bash
git diff ef0cdd8..HEAD -- shaders/tonemap.wgsl
# No output = no changes
```

**Conclusion:** The "improved contrast" observed in ef0cdd8 is NOT from tone mapping. It's from:
1. Higher color precision (10000 levels vs 10)
2. No adaptive smoothing (faster convergence)

---

## Problem-Solution-Trade-off Analysis

### Problem 1: Histogram Overflow at High Densities
**Symptom:** Bright areas suddenly turn dark (u16 overflow wraps to 0)

**Solution Implemented:** Reduce default `histogram_color_scale` from 100 to 10
- Before: 65535 / 100 = 655 hits max
- After: 65535 / 10 = 6553 hits max (10× better overflow protection)

**Trade-off Introduced:** Color quantization
- Before: 100 color levels (1% precision)
- After: 10 color levels (10% precision) - **VISIBLE BANDING**

**User Feedback:**
> "I can confirm it keeps the histogram from overflowing."

### Problem 2: Low-Density Sparkle Artifacts
**Symptom:** Random single hits very visible in dark areas (statistical variance)

**Solution Implemented:** Adaptive `low_density_smoothing` (default 0.5)
- Low-density pixels: Reduced blend weight
- High-density pixels: Full blend weight

**Trade-off Introduced:** Slower convergence and less accuracy
- Noise suppressed in sparse areas
- But also delays color appearance
- Mathematically less pure

**User Feedback:**
> "However, something about these changes messed up the lower density areas. They move toward grey when they should be black, and there's lots of visible noise now."

---

## Recommendations

### For Restoring Quality (Match ef0cdd8)

#### Quick Fix (UI Adjustments)
User can manually adjust settings:
1. **Histogram Color Scale:** 10 → 100 (max slider value)
2. **Low-Density Smoothing:** 0.5 → 0.0 (disable)
3. **Exposure/Density Scale:** Adjust if still too dark

**Result:** Much closer to ef0cdd8, but still 100× less precision (100 vs 10000)

#### Code Fix Option 1: Increase Default Scale
```rust
// src/config.rs
fn default_histogram_color_scale() -> f32 {
    100.0  // Was 10.0
}
```

**Pros:**
- 10× better color precision (100 levels vs 10)
- Closer to main branch quality (scale=100)

**Cons:**
- Overflow at 655 hits (may see artifacts in zoomed-out scenes)
- User must lower scale if overflow occurs

#### Code Fix Option 2: Disable Smoothing by Default
```rust
// src/config.rs
fn default_low_density_smoothing() -> f32 {
    0.0  // Was 0.5
}
```

**Pros:**
- Mathematically pure blending (like ef0cdd8)
- Faster convergence
- Brighter sparse areas

**Cons:**
- Visible sparkle artifacts in dark areas
- User must enable smoothing if noise bothers them

#### Code Fix Option 3: Revert to 4 Atomic Ops
Revert u16 packing, use 4× u32 histogram like ef0cdd8

**Pros:**
- Can use scale=10000 without overflow
- Perfect color precision
- Simple algorithm

**Cons:**
- ~13.8% slower performance
- 2× memory usage (31 MB vs 16 MB @ 1080p)

#### Code Fix Option 4: Use u8 Packing (RECOMMENDED)
Pack RGBA into 1× u32 using 4× u8 (0-255)

**Pros:**
- 256 color levels (26× better than current default)
- Overflow at 16.7M hits (essentially impossible)
- Still 2 atomic ops per pixel (same speed as current)
- Simple bit manipulation

**Cons:**
- Slightly more complex packing logic
- 8-bit quantization (but much better than 10 levels!)

**Implementation:**
```wgsl
// Compute shader
let r8 = u32(clamp(final_color.r, 0.0, 1.0) * 255.0);
let g8 = u32(clamp(final_color.g, 0.0, 1.0) * 255.0);
let b8 = u32(clamp(final_color.b, 0.0, 1.0) * 255.0);
let packed_rgba = r8 | (g8 << 8u) | (b8 << 16u) | (255u << 24u);

atomicAdd(&histogram[base_idx + 0u], packed_rgba);  // RGBA color
atomicAdd(&histogram[base_idx + 1u], 1u);           // Density (separate u32, no overflow)

// Accumulate shader
let r_sum = f32(packed_rgba & 0xFFu);
let g_sum = f32((packed_rgba >> 8u) & 0xFFu);
let b_sum = f32((packed_rgba >> 16u) & 0xFFu);
let density = f32(histogram[base_idx + 1u]);

new_color.r = r_sum / (density * 255.0);
// ...
```

---

## Performance Summary

### Main Branch (e7bbf73)
- **Algorithm:** 4× u32 naive atomic
- **Throughput:** 6.43 Giter/sec
- **Quality:** Perfect (scale=100 hardcoded)
- **Memory:** 31 MB @ 1080p

### Main + u16 Packing (ce58657)
- **Algorithm:** 2× u32 packed (u16 fixed-point)
- **Throughput:** 7.46 Giter/sec (+16%)
- **Quality:** Perfect (scale=100 hardcoded)
- **Memory:** 16 MB @ 1080p (-48%)

### Experiment Branch Current (690c52f)
- **Algorithm:** 2× u32 packed + batched accumulation
- **Throughput:** 25.08 Giter/sec (+290% vs main!)
- **Quality:** Regressed (scale=10 default, smoothing=0.5)
- **Memory:** 16 MB @ 1080p

**Bottleneck Analysis:**
- Zoom level affects atomic count, not computation
- Low zoom (1.0): 95% hit rate → more atomics → slower
- High zoom (25.5): 1.5% hit rate → fewer atomics → faster
- ~10% performance variance proves atomic bottleneck

---

## Documentation Created

This investigation produced comprehensive documentation:

1. **[COLOR_PIPELINE.md](COLOR_PIPELINE.md)** - Complete pipeline (5 stages, all parameters)
2. **[HISTOGRAM_EVOLUTION.md](HISTOGRAM_EVOLUTION.md)** - Algorithm history and changes
3. **[QUALITY_INVESTIGATION.md](QUALITY_INVESTIGATION.md)** - Specific analysis of ce58657/ef0cdd8 vs HEAD
4. **[HISTOGRAM_OPTIMIZATION_SUMMARY.md](HISTOGRAM_OPTIMIZATION_SUMMARY.md)** - Final u16 packing results (main branch)
5. **[HISTOGRAM_OPTIMIZATION_ATTEMPTS.md](HISTOGRAM_OPTIMIZATION_ATTEMPTS.md)** - Failed attempts and lessons
6. **[HISTOGRAM_IMPLEMENTATION_PLAN.md](HISTOGRAM_IMPLEMENTATION_PLAN.md)** - Original implementation plan
7. **[HISTOGRAM_COLOR_SCALE.md](HISTOGRAM_COLOR_SCALE.md)** - Overflow/precision trade-offs

---

## Key Lessons Learned

### 1. Recent Changes Were FIXES, Not Improvements
**Important clarification from user:**
> "The changes recently were not made to improve quality, they were made to correct the histogram overflow."

The batched accumulation experiments introduced problems (overflow, noise) and created solutions (configurable scale, adaptive smoothing), but these solutions have quality trade-offs.

### 2. ef0cdd8 Quality Came from High Precision
The "good quality" of ef0cdd8 was primarily due to:
- `color_scale = 10000` (10000 color levels)
- No adaptive smoothing (pure blending)
- Simple algorithm (fewer moving parts)

### 3. Default Values Matter
Current defaults (scale=10, smoothing=0.5) prioritize **robustness** (no overflow, no sparkle) over **quality** (precision, convergence speed).

Previous defaults (scale=10000 → 100, no smoothing) prioritized **quality** over robustness.

### 4. Trade-offs Are Fundamental
With u16 packing, you MUST choose:
- **High precision** (scale=100) → overflow at 655 hits
- **Overflow protection** (scale=10) → severe color quantization

There's no way to have both with u16. Alternative: u8 packing (256 levels, never overflows).

### 5. User Control Is Valuable
Making parameters user-configurable was the right decision. Different scenes and use cases need different trade-offs:
- Zoomed-out animation: Low scale, high smoothing
- Zoomed-in still image: High scale, no smoothing

---

## Next Steps

### Option A: Merge Experiment Branch AS-IS
**Pros:**
- Users have full control (sliders)
- Batched accumulation is much faster
- Overflow and noise are solved

**Cons:**
- Default quality is regressed
- Users must adjust settings manually
- May confuse users expecting good defaults

**Recommendation:** Update defaults (scale=100, smoothing=0.0) before merge

### Option B: Implement u8 Packing First
**Pros:**
- 256 color levels (26× better than current default)
- No overflow possible (density separate)
- Can use higher default scale (255)
- Better balance of quality and robustness

**Cons:**
- More work (need to implement and test)
- Delays merge of batched accumulation

**Recommendation:** Implement u8 packing in new branch, then merge both

### Option C: Keep Branches Separate
**Pros:**
- Main branch stays stable (scale=100, good quality)
- Experiment branch for performance testing
- Users can choose which to use

**Cons:**
- Code duplication
- Maintenance burden

**Recommendation:** Not ideal long-term

---

## Conclusion

The quality "regression" from ef0cdd8 to HEAD is **well-understood and documented**:

1. **Primary cause:** color_scale reduced from 10000 to 10 (1000× precision loss)
2. **Secondary cause:** adaptive smoothing enabled (default 0.5)
3. **Not a cause:** Tone mapping (unchanged)

The changes were **intentional fixes** for overflow and noise, but introduced new trade-offs.

**Recommended action:** Implement u8 packing (Option B) for best balance of quality, performance, and robustness. This provides:
- 256 color levels (good quality)
- No overflow (robustness)
- 2 atomic ops (performance)
- Simple defaults (user-friendly)

**Immediate action:** Update defaults to scale=100, smoothing=0.0 if merging current experiment branch.

---

## Files to Update

If proceeding with current approach (update defaults):

1. **src/config.rs**
   ```rust
   fn default_histogram_color_scale() -> f32 { 100.0 }  // Was 10.0
   fn default_low_density_smoothing() -> f32 { 0.0 }    // Was 0.5
   ```

2. **Documentation**
   - Update [CLAUDE.md](../CLAUDE.md) with current status
   - Update [STATUS.md](STATUS.md) with experiment branch info
   - Mark quality investigation as complete

3. **Testing**
   - Benchmark with new defaults
   - Visual comparison with ef0cdd8
   - Validate overflow protection still works at typical zoom levels

---

## References

- Main branch: `e7bbf73` (u16 packed, scale=100)
- Good quality baseline: `ef0cdd8` (4 atomic, scale=10000)
- Current experiment: `690c52f` (2 atomic + batched, scale=10)
- All documentation: `docs/` directory
- Benchmark data: `benchmark_results/benchmark_history.csv`
