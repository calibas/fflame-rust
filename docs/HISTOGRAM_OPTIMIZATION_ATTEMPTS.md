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

### Option 4: Workgroup-Local Histograms
Share histogram across workgroup (256 threads), then atomic merge:
```wgsl
var<workgroup> local_histogram: array<atomic<u32>, WIDTH*HEIGHT*4>;
// Accumulate to workgroup histogram
// workgroupBarrier()
// Single thread: merge to global
```

**Pros:** 256× fewer atomic operations
**Cons:** Workgroup memory limits, complex synchronization
**Status:** Worth exploring

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

## Recommendations

**Short term:** Accept the 50% performance cost for quality
- Histogram approach is correct and matches Apophysis
- Quality improvement is worth the cost for static rendering

**Medium term:** Investigate workgroup-local histograms (Option 4)
- Most promising for significant performance gain
- Reduces global atomic operations dramatically

**Long term:** Implement hybrid mode (Option 5)
- Quality mode for final renders
- Speed mode for animation/interaction
- Best of both worlds

---

## References

- WGSL Atomic Operations: https://www.w3.org/TR/WGSL/#atomic-builtin-functions
- Histogram Acceleration Papers: GPU Gems, Parallel Histogram
- Apophysis Implementation: CPU-based histogram (no atomic contention)
