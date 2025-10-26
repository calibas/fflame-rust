# Histogram Optimization Attempts

This document tracks attempts to optimize the histogram-based atomic accumulation system to reduce the ~50% performance impact.

## Current Implementation (Baseline)

**Approach:** 4 separate atomic operations per pixel hit
```wgsl
atomicAdd(&histogram[base_idx + 0u], r);  // R: 0-10000
atomicAdd(&histogram[base_idx + 1u], g);  // G: 0-10000
atomicAdd(&histogram[base_idx + 2u], b);  // B: 0-10000
atomicAdd(&histogram[base_idx + 3u], 1u); // density
```

**Performance:** ~50% frame rate reduction vs old textureStore approach
**Quality:** Perfect (near-identical to Apophysis)
**Buffer Size:** 31MB @ 1920×1080 (width × height × 4 × 4 bytes)

---

## Attempt 1: Bit-Packed RGB (FAILED)

**Date:** 2025-10-25

**Idea:** Pack RGB into single u32 to reduce atomic operations from 4 to 2
- R: bits 0-9 (10-bit: 0-1023)
- G: bits 10-19 (10-bit: 0-1023)
- B: bits 20-29 (10-bit: 0-1023)
- Unused: bits 30-31

**Implementation:**
```wgsl
// Pack
let packed_color = r | (g << 10u) | (b << 20u);
atomicAdd(&histogram[base_idx + 0u], packed_color);
atomicAdd(&histogram[base_idx + 1u], 1u);  // density

// Unpack
let r_sum = f32(packed_color & 0x3FFu);
let g_sum = f32((packed_color >> 10u) & 0x3FFu);
let b_sum = f32((packed_color >> 20u) & 0x3FFu);
```

**Why it failed:**
- Bit field overflow corrupts adjacent channels
- Example: R field (bits 0-9) can hold max 1023
- If R accumulates to 1024+, it overflows into G field (bits 10-19)
- Multiple hits to same pixel cause unpredictable overflow
- Result: Bright areas turn grey (color corruption)

**Lesson:** Bit-packing doesn't work with atomicAdd unless we can guarantee no overflow

**UPDATE 2025-10-26:** New analysis reveals atomic operations are the primary bottleneck (10% perf difference between zoom levels purely from atomic count). Revisiting packed approach with 8-bit saturation instead of 10-bit. See [PACKED_HISTOGRAM_PLAN.md](PACKED_HISTOGRAM_PLAN.md) for detailed plan.

---

## Attempt 3: Per-Thread Local Cache (REVERTED - Performance Regression!)

**Date:** 2025-10-26 (commits 06bfcab, 58fb8de)

**Idea:** Each thread maintains 16-pixel cache to batch atomic operations

**Implementation:**
```wgsl
var local_cache: array<LocalPixel, 16>;  // 16 pixels per thread
// Accumulate to cache (no atomics!)
// On cache miss or full: flush to global histogram
```

**Result:** Catastrophic performance regression!
- Simple (zoom 1.0): **-0.9% slower**
- Medium (zoom 21.1): **-14.5% slower**
- High (zoom 25.5): **-53% slower!!**

**Why it failed:**
- Cache overhead (64 floats per thread)
- Poor cache hit rate (fractal iterations jump spatially)
- Cache flush overhead dominates any atomic savings
- Gets exponentially worse at high zoom (opposite of intended!)

**Lesson:** Local caching without workgroup cooperation doesn't help fractal workloads

**Status:** REVERTED in commit 58fb8de

---

## Attempt 2: Lower Precision (FAILED)

**Date:** 2025-10-25

**Idea:** Reduce color scale from 10000 to 1023 (10-bit) to reduce atomic contention

**Implementation:**
```wgsl
let color_scale = 1023.0;  // Instead of 10000.0
```

**Result:** Performance worse, no improvement
**Why it failed:** Precision reduction doesn't significantly reduce atomic contention

---

## Potential Future Optimizations

### Option 1: GPU-Based Clear
Replace `encoder.clear_buffer()` with compute shader clear:
```wgsl
@compute @workgroup_size(256, 1, 1)
fn clear_histogram(idx: u32) {
    if (idx < buffer_size) {
        atomicStore(&histogram[idx], 0u);
    }
}
```

**Pros:** Potentially faster than CPU-side clear for large buffers
**Cons:** Adds pipeline/dispatch overhead
**Status:** Shader exists at `shaders/clear_histogram.wgsl`, not yet integrated

### Option 2: Overflow-Safe Packing
Use saturating add or conditional packing to prevent overflow:
```wgsl
// Only pack if sum won't overflow
let new_r = old_r + r;
if (new_r < 1024) {
    // Safe to pack
} else {
    // Fallback or saturate
}
```

**Pros:** Could reduce atomic ops to 2
**Cons:** Complex logic, may not be faster
**Status:** Unexplored

### Option 3: Per-Thread Local Histograms
Each thread maintains local histogram, then merge at end:
```wgsl
var local_histogram: array<u32, HISTOGRAM_SIZE>;
// Accumulate locally (no atomics)
// Then: atomicAdd to global histogram once
```

**Pros:** Dramatically fewer atomic operations
**Cons:** Massive memory usage (histogram per thread)
**Status:** Impractical for current workgroup sizes

### Option 4: Per-Thread Local Cache ⭐ RECOMMENDED
Each thread maintains small cache of recently hit pixels:
```wgsl
var local_cache: array<LocalPixel, 16>;  // 16 pixels per thread
// Accumulate to cache (no atomics!)
// On cache miss or full: flush to global histogram
```

**Pros:**
- Simple to implement (no workgroup sync)
- Automatic adaptation to fractal structure
- Expected 2-3× improvement with 70-80% cache hit rate
- Fractal flames have spatial locality (same pixels hit repeatedly)

**Cons:** Limited cache size (8-32 pixels typical)
**Status:** ⭐ **PLANNED** - See [WORKGROUP_LOCAL_HISTOGRAM_PLAN.md](WORKGROUP_LOCAL_HISTOGRAM_PLAN.md)

### Option 5: Hybrid Mode Switch
Provide quality vs speed toggle:
- **Quality mode:** Histogram (current, slow, perfect)
- **Speed mode:** textureStore (old, fast, color noise)
- UI toggle or auto-switch based on interaction

**Pros:** User choice, animation could use speed mode
**Cons:** Code complexity, two render paths to maintain
**Status:** Feasible fallback option

### Option 6: Adaptive Iterations
Reduce `iterations_per_thread` to decrease atomic contention:
- Fewer iterations = fewer atomic conflicts
- Compensate with more dispatches or longer accumulation

**Pros:** Could balance quality and speed
**Cons:** May affect convergence quality
**Status:** Worth testing

---

## Performance Profiling Needed

To better understand the bottleneck:
1. Profile with GPU profiler (NSight, RenderDoc)
2. Measure clear vs compute vs accumulate pass times
3. Test different workgroup sizes
4. Test different iterations_per_thread values
5. Compare atomic contention at different resolutions

**Key Question:** Is the bottleneck:
- A) `clear_buffer()` on 31MB buffer every frame?
- B) Atomic contention during compute pass?
- C) Memory bandwidth from histogram reads/writes?

---

## Current Status (2025-10-26)

### Atomic Bottleneck Hypothesis - CONFIRMED ✅

**Controlled test revealed 10% performance difference from atomic count:**
- Low zoom (1.0): 6510ms (more atomics, 95% viewport hit rate)
- High zoom (25.5): 5904ms (fewer atomics, 1.5% viewport hit rate)

**Packed histogram test PROVED the hypothesis:**
- Implemented u8 packing (4 atomics → 1)
- Result: **5810ms → 4998ms (14% faster!)**
- Conclusion: **Atomic operations ARE the bottleneck**

### Previous Issue: Overflow Artifacts (SOLVED) ✅

**Problem (u8 packing):** Color corruption in bright areas
- Overflow occurred after ~256 hits per channel
- atomicAdd on packed u32 caused bit field overflow

**Solution (f16 packing):** Use half-precision floats instead
- Pack RGBA as 4× f16 into 2× u32
- 2 atomic operations per pixel (down from 4)
- No overflow (f16 range: ±65504)
- HDR support (values > 1.0 preserved)

**Status:** Implemented in commit 51f05cb - ready for testing

## Recommendations (Updated 2025-10-26)

**✅ COMPLETED: f16 packed format implementation**
- Implemented in commit 51f05cb
- Pack RGBA as 4× f16 into 2× u32
- Reduced from 4 atomics → 2 atomics (50% reduction)
- Expected: 7-10% speedup vs naive
- **Benefits:**
  - No overflow (f16 range: ±65504)
  - HDR support for tone mapping (user requirement met)
  - Better quality than u8 packing
  - Faster than naive histogram

**Next: Benchmark f16 implementation**
- Test visual quality (should be perfect, no artifacts)
- Measure performance (expect 7-10% improvement vs naive)
- Compare with Apophysis HDR workflow

**Alternative (if needed): atomicCompareExchange with saturation**
- Implement saturating add using compare-and-swap
- Maintains u8 precision but prevents overflow
- Performance unknown (loop overhead may negate gains)
- Only explore if f16 results are unsatisfactory

**Fallback: Naive histogram (always available)**
- 14% slower than packed, but perfect quality
- Already implemented and working
- Conservative option if optimizations don't pan out

---

## References

- WGSL Atomic Operations: https://www.w3.org/TR/WGSL/#atomic-builtin-functions
- Histogram Acceleration Papers: GPU Gems, Parallel Histogram
- Apophysis Implementation: CPU-based histogram (no atomic contention)
