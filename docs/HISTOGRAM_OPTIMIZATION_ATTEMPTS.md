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

## Final Status (2025-10-26)

### ✅ COMPLETED: u16 Fixed-Point Packing (commit 286fadb)

**Problem Solved:** Atomic operations were the bottleneck (confirmed via testing)

**Final Solution:** Pack RGBA+density as 4× u16 fixed-point integers into 2× u32
- Scale: 100 (allows 655 hits before overflow)
- Precision: 1% quantization (imperceptible)
- Atomics: 2 per pixel (down from 4)

### Benchmark Results (2025-10-26 12:18:18)

| Approach | Time | vs Naive | Quality | Notes |
|----------|------|----------|---------|-------|
| **u16 packed (scale=100)** | **5335ms** | **-13.8%** ✅ | Perfect ✅ | **FINAL** |
| Naive atomic (4 atomics) | 6191ms | baseline | Perfect ✅ | Original |
| u8 packed (scale=255) | 4998ms | -19.3% | Broken ❌ | Overflow |
| f16 packed (pack2x16float) | 5248ms | -15.2% | Broken ❌ | Bit corruption |

**Winner:** u16 fixed-point packing with scale=100

### Why This Works

1. **Fixed-point integers** work correctly with `atomicAdd` (unlike floats)
2. **Scale=100** prevents overflow (655 hits at full brightness)
3. **1% precision** is sufficient for visual quality
4. **2× atomic reduction** provides 13.8% speedup
5. **50% memory reduction** (16 MB vs 31 MB @ 1080p)

### Why Other Approaches Failed

- **u8 packing:** Overflow at 256 hits → grey artifacts
- **f16 packing:** Integer addition on float bits → psychedelic noise
- **Local cache:** Cache overhead > atomic cost → 50%+ regression

### Investigation Summary

**Key Discovery:** Zoom affects atomic count, not computation
- Low zoom (1.0): 95% hit rate → more atomics → slower
- High zoom (25.5): 1.5% hit rate → fewer atomics → faster
- ~10% performance difference proves atomic bottleneck

**Solution Validation:** Packed atomics confirmed the hypothesis
- u8 packed: 14% faster (but broken)
- u16 packed: 13.8% faster (working!)

## Recommendations (Final)

**✅ READY TO MERGE TO MAIN**

The u16 packed histogram (scale=100) achieves all goals:
- ✅ **13.8% performance improvement** (855ms faster)
- ✅ **Perfect visual quality** (no artifacts, no noise)
- ✅ **Memory efficient** (50% buffer reduction)
- ✅ **Stable timings** (0.25% CV)
- ✅ **Sufficient overflow headroom** (655 hits at full brightness)

**See:** `docs/HISTOGRAM_OPTIMIZATION_SUMMARY.md` for complete analysis.

---

## References

- WGSL Atomic Operations: https://www.w3.org/TR/WGSL/#atomic-builtin-functions
- Histogram Acceleration Papers: GPU Gems, Parallel Histogram
- Apophysis Implementation: CPU-based histogram (no atomic contention)
