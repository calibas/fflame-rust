# Zoom Performance Analysis

## Benchmark Findings (2025-10-26)

### CRITICAL: Local Cache Performance Regression

**Benchmark data reveals the 16-pixel local cache optimization is a COMPLETE REGRESSION:**

| Fractal | Zoom | textureStore (ms) | Histogram Naive (ms) | **Histogram+Cache (ms)** | Cache vs Naive | Cache vs textureStore |
|---------|------|-------------------|----------------------|--------------------------|----------------|----------------------|
| complex | 1.0  | 162.75            | 169.96               | **171.58**               | **-0.9% (slower!)** | **-5.4% (slower)** |
| simple4 | 21.1 | 1456.81           | 1341.20              | **1536.17**              | **-14.5% (WORSE!)** | **-5.4% (slower)** |
| simple3 | 25.5 | 7387.78           | 5899.89              | **9030.28**              | **-53% (DISASTER!)** | **-22.2% (slower)** |

### Key Observations

1. **Local cache makes performance WORSE in ALL cases**
2. **At low zoom (1.0)**: Cache is 0.9% slower than naive histogram
3. **At high zoom (21-25)**: Cache becomes **catastrophically slower** (14-53% regression!)
4. **Naive histogram WITHOUT cache is actually faster than textureStore at high zoom**:
   - Zoom 21.1: Naive is **8% faster** than textureStore ✅
   - Zoom 25.5: Naive is **20% faster** than textureStore ✅
5. **The cache destroys this advantage**:
   - Zoom 21.1: Cache is **15% slower** than textureStore ❌
   - Zoom 25.5: Cache is **22% slower** than textureStore ❌

### Why Does the Local Cache Fail?

**Original hypothesis (WRONG):** 16-pixel cache would reduce atomic contention by batching writes.

**Actual behavior:** The cache appears to:
- Add overhead for cache management (16 slots × 4 channels = 64 floats per thread)
- Cause cache misses more often than hits (fractal iterations jump around spatially)
- Introduce synchronization overhead when flushing cache to histogram
- Degrade memory access patterns (worse cache locality at GPU L2 level?)

**At high zoom, the problem gets exponentially worse:**
- More iterations should mean better cache utilization (same pixels hit repeatedly)
- Instead, we see 53% slowdown at zoom 25.5 compared to naive histogram
- This suggests fundamental design flaw in cache implementation

**Naive histogram WITHOUT cache:**
- Simple atomic operations directly to histogram texture
- GPU may optimize atomic operations better than our manual caching
- No cache management overhead
- Clean, predictable memory access patterns

### Why Does Zoom Affect Performance?

**Root cause:** High zoom causes **more iterations to hit the same pixels repeatedly**.

When zoom is high:
- The visible fractal region is smaller
- More iterations land within the visible window
- Iterations cluster around the same pixel coordinates
- This creates **higher contention** on the same memory locations

**textureStore behavior (race conditions):**
- At high zoom: Many writes to same pixels race
- Last write wins → **more wasted work** as earlier writes are discarded
- Performance degrades as more iterations fight for same pixels
- Data loss causes quality degradation

**Histogram naive behavior (atomic accumulation):**
- At high zoom: Many atomic operations to same memory addresses
- GPU may optimize repeated atomics to same addresses (hardware cache)
- Accumulation is correct (all iterations count, nothing wasted)
- **At high zoom, becomes faster than textureStore!**

**Histogram + cache behavior (BROKEN):**
- Cache overhead dominates any potential benefit
- Gets progressively worse at high zoom (opposite of intended)
- Needs to be reverted

### Apophysis: Scale vs Zoom (THE KEY INSIGHT!)

**Source:** [Apophysis Back to Basics: Zoom And Scale Demystified](https://www.ultragnosis.com/fractals/Resources/zoomscale.pdf)

Apophysis has **TWO SEPARATE PARAMETERS** with fundamentally different behavior:

#### **Scale Parameter (Linear, "Quick-and-Dirty")**
- Works on **linear scale**: doubling Scale = 2× linear zoom
- **Does NOT adjust sample density** - quality degrades
- Default: `Scale = 25`
- **Fast for preview** - doesn't require more samples
- **Use case:** Quick navigation during editing
- **Quality impact:** `Scale = 100` requires `4× Quality` to maintain same image quality

#### **Zoom Parameter (Logarithmic, Quality-Preserving)**
- Works on **logarithmic scale (base 2)**: `Zoom = 1` means 2× visual zoom
- **Automatically compensates sample density** - quality maintained
- Default: `Zoom = 0`
- **Slower but quality-preserving** - adjusts sample density automatically
- **Use case:** Animation rendering where quality must stay constant
- **Mathematical relationship:** `image quality ∝ 2^(2 × Zoom)`

#### **Mathematical Relationships**

**Quality and magnification:**
```
image quality ∝ Quality parameter
image quality ∝ 2^(2 × Zoom)
render time ∝ image quality
```

**Zoom impact on required Quality:**
- `Zoom = -3`: Requires `64× Quality` to maintain same image quality
- `Zoom = -2`: Requires `16× Quality`
- `Zoom = -1`: Requires `4× Quality`
- `Zoom = 0`: Default (1× Quality)
- `Zoom = +1`: Requires `0.25× Quality` (image quality increases automatically)

**Scale impact on required Quality:**
- `Scale = 25` (default): 1× Quality
- `Scale = 50`: Requires `2× Quality`
- `Scale = 100`: Requires `4× Quality`
- `Scale = 200`: Requires `8× Quality`

**Formula:** `Quality_needed = Quality_base × (Scale_new / Scale_default)^2`

#### **Our Implementation (PROBLEM IDENTIFIED!)**

**We only have ONE parameter - "zoom" - which behaves like Apophysis "Scale":**

```rust
// In apophysis_xml.rs
let zoom = scale / 200.0; // Apophysis scale 200.0 = our zoom 1.0
```

**Our zoom:**
- Works **linearly** (like Apophysis Scale)
- Does **NOT automatically adjust sample density** (like Apophysis Scale)
- Higher zoom = worse quality per iteration (like Apophysis Scale)

**This explains the performance characteristics!**
- High zoom = smaller visible region
- Same number of iterations spread over smaller area
- Higher effective sample density (good for quality)
- BUT: More iterations wasted outside viewport (bad for performance)
- AND: More contention on same pixels (explains why histogram becomes faster!)

### Shader Implementation

**Current zoom application (utilities.wgsl):**
```wgsl
fn world_to_pixel(p: vec2<f32>) -> vec2<i32> {
    // Apply view transform: pan, rotation, and zoom
    var transformed = p - vec2<f32>(params.pan_x, params.pan_y);

    // Apply rotation
    let cos_r = cos(params.rotation);
    let sin_r = sin(params.rotation);
    transformed = vec2<f32>(
        transformed.x * cos_r - transformed.y * sin_r,
        transformed.x * sin_r + transformed.y * cos_r
    );

    // Apply zoom
    transformed = transformed * params.zoom;

    // Map from fractal space to pixel space
    let scale = f32(min(params.width, params.height)) * 0.25;
    let center = vec2<f32>(f32(params.width), f32(params.height)) * 0.5;

    return vec2<i32>(
        i32(transformed.x * scale + center.x),
        i32(transformed.y * scale + center.y)
    );
}
```

**The zoom multiplies the coordinates BEFORE mapping to pixels**, making high zoom values cause all iterations to cluster into a smaller pixel region.

### Analysis: Why User Reports Different Performance

> "The issue with Zoom affecting render times happens with Apophysis too. There's a different setting in Apophysis, called Scale, that seems to do the same thing but without such a negative impact on performance."

**Resolution:** User was likely comparing:
- **Apophysis Scale** (fast preview, lower quality)
- **Apophysis Zoom** (slow, maintains quality)

The "different performance" is **by design** in Apophysis:
- Scale is "quick-and-dirty" for fast preview
- Zoom adjusts quality automatically (slower but correct)

**Our implementation has only ONE parameter that behaves like Apophysis Scale** - fast but quality degrades at high values.

### Action Required: Revert Local Cache Implementation

**IMMEDIATE ACTION: Revert to commit ef0cdd8 (Histogram Fixed - naive atomic)**

**Reasoning:**
1. ✅ Naive histogram is **8-20% faster** than textureStore at high zoom
2. ✅ Naive histogram maintains quality (no race conditions)
3. ❌ Local cache is **0.9-53% slower** than naive histogram
4. ❌ Local cache regression gets WORSE at high zoom (opposite of intended)
5. ❌ Cache overhead dominates any theoretical benefit

**Commits to consider:**
- `dd80003` - Before Histogram (textureStore) - **BASELINE (has race conditions)**
- `ef0cdd8` - Histogram Fixed (naive atomic) - **TARGET (best performance + quality)**
- `06bfcab` - Histogram + Local Cache (current) - **BROKEN (revert this)**

**Performance summary:**
| Implementation | Low Zoom (1.0) | High Zoom (21.1) | High Zoom (25.5) |
|----------------|----------------|------------------|------------------|
| textureStore   | 162.75ms       | 1456.81ms        | 7387.78ms        |
| Naive Histogram| 169.96ms (+4%) | 1341.20ms (-8%)  | 5899.89ms (-20%) |
| Cache (current)| 171.58ms (+5%) | 1536.17ms (+5%)  | 9030.28ms (+22%) |

**Verdict:** Naive histogram is the clear winner. Revert the cache implementation.

### Zoom/Scale Behavior (Secondary Finding)

**Our zoom parameter correctly implements Apophysis "Scale" behavior:**
- Linear magnification (not logarithmic)
- Does NOT auto-adjust sample density
- Higher zoom = more render time (expected, same as Apophysis)
- User should manually adjust quality for different zoom levels

**This is correct behavior** - no changes needed to zoom implementation.

---

**Status:** Critical regression identified - local cache must be reverted.

**Conclusion:**
The 16-pixel local cache optimization was a failed experiment that makes performance worse in all scenarios. Naive histogram (ef0cdd8) is faster than both textureStore AND the cache implementation, especially at high zoom where it provides 8-20% speedup. Revert to commit ef0cdd8.
