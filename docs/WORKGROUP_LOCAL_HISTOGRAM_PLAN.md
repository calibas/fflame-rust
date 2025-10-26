# Workgroup-Local Histogram Implementation Plan

## ✅ IMPLEMENTED - Performance Results

**Implementation Date:** 2025-10-26

**Approach Used:** Per-thread local cache (16 pixels per thread)
**Alternative Considered:** Workgroup shared memory (rejected due to size constraints)

### Measured Performance (Quantified)

**Test Configuration:**
- Resolution: 1920×1080 (2.07M pixels)
- Total Iterations: 1 billion (1,000,341,504)
- Iterations/Thread: 256
- Test Case: Spherical variation

**Baseline (Before Histogram):**
- Implementation: Direct textureStore (race conditions, quality issues)
- Render Time: 159.84ms
- Throughput: 6.26 Giter/sec
- Git Hash: `ce41484` (commit a376488)

**Current (Histogram + Local Cache):**
- Implementation: Histogram with 16-pixel per-thread cache
- Render Time: 146.39ms
- Throughput: 6.83 Giter/sec
- Git Hash: `406b0d9`

**Performance Improvement:**
- **9% faster** than old textureStore approach (1.09× speedup)
- Apophysis-quality rendering maintained (proper accumulation)
- No visual artifacts or color noise

**Conclusion:** Per-thread local cache **exceeded expectations** - not only recovered the performance loss but actually improved performance beyond the baseline while maintaining correct rendering quality.

---

## Executive Summary (Original Plan)

**Goal:** Reduce atomic contention by accumulating to workgroup-local memory first, then merging to global histogram.

**Expected Result:** Recover most of the 50% performance loss while maintaining Apophysis-quality rendering.

**Key Insight:** Workgroup shared memory is on-chip (very fast), global memory is off-chip (slow). By accumulating locally first, we reduce global atomic operations by 64× (workgroup size).

---

## Current Implementation (Baseline)

### Architecture
```
Each Thread (64 threads per workgroup, 128 workgroups)
  ↓
  Iterate 256 times
  ↓
  Hit pixel → atomicAdd to GLOBAL histogram (4 ops)
  ↓
SLOW: Global memory, high contention
```

**Performance:**
- 128 workgroups × 64 threads × 256 iterations = 2,097,152 pixel hits per frame
- Each hit = 4 atomic operations to **global memory**
- Total: ~8.4 million global atomic operations per frame
- Result: ~50% slower than old textureStore approach

### Memory Layout (Global)
```rust
// Global histogram buffer: [r, g, b, density] × (width × height)
// Size: 1920 × 1080 × 4 × 4 bytes = 31 MB
histogram_buffer: Buffer
```

---

## Proposed Implementation: Workgroup-Local Histograms

### Architecture
```
Workgroup (64 threads)
  ↓
  Each thread iterates 256 times
  ↓
  Hit pixel → atomicAdd to LOCAL workgroup histogram (4 ops)
  ↓
FAST: On-chip shared memory, low contention
  ↓
  workgroupBarrier()  // Wait for all threads
  ↓
  Designated threads merge LOCAL → GLOBAL (4 ops per touched pixel)
  ↓
REDUCED: Only pixels touched by this workgroup need global atomics
```

**Performance Benefits:**
- Local atomics: MUCH faster (on-chip shared memory, ~1-2 cycles)
- Global atomics: Reduced by ~64× (only merge at end, once per pixel per workgroup)
- Less contention: Multiple threads can hit different pixels in local memory simultaneously

### Memory Layout

**Workgroup Shared Memory:**
```wgsl
// Problem: Can't store full-resolution histogram in workgroup memory
// Typical limit: 16-32 KB per workgroup
// Full histogram would be: 1920 × 1080 × 4 × 4 = 31 MB (WAY too big!)

// Solution: Only store histogram for pixels that THIS workgroup touches
// Workgroup touches ~1000-2000 unique pixels (depends on fractal structure)
// Conservative allocation: 4096 pixels × 4 u32s × 4 bytes = 64 KB

// But workgroup limit is typically 16-32 KB!
// Need either:
// 1. Smaller local histogram (1024-2048 pixels)
// 2. Hash table / sparse storage
// 3. Different approach
```

**Challenge:** Workgroup memory limits prevent storing full-resolution local histogram.

---

## Alternative Approach: Tiled Local Histograms

### Concept
Divide the screen into tiles, assign workgroups to tiles, each workgroup maintains histogram for its tile only.

```
Screen (1920 × 1080)
  ↓
Divide into tiles (e.g., 16 × 16 = 256 pixels per tile)
  ↓
Each workgroup processes ONE tile
  ↓
Local histogram: 256 pixels × 4 u32s = 4 KB (FITS!)
```

**Pros:**
- Fits in workgroup memory (4-8 KB per workgroup)
- Predictable memory access patterns
- No hash table complexity

**Cons:**
- Requires restructuring compute shader (workgroup per tile, not per thread batch)
- Threads may idle if tile has few fractal points
- Load imbalance (some tiles hot, some cold)

---

## Better Alternative: Hash-Based Sparse Local Histogram

### Concept
Use a hash table in workgroup memory to store only pixels that are actually hit.

```wgsl
// Workgroup shared memory
struct LocalHistEntry {
    pixel_idx: u32,  // Which pixel (or 0xFFFFFFFF if empty)
    r: u32,
    g: u32,
    b: u32,
    density: u32,
}

var<workgroup> local_histogram: array<LocalHistEntry, 1024>;  // 1024 slots = 20 KB
```

**Algorithm:**
1. Thread hits pixel
2. Hash pixel coordinate to find slot in local_histogram
3. If slot empty: claim it (atomic compare-exchange)
4. If slot matches pixel: accumulate (atomic add)
5. If slot occupied by different pixel: linear probe to next slot
6. After all threads done: merge non-empty slots to global histogram

**Pros:**
- Adapts to actual pixel hit distribution
- Fixed memory footprint (1024 slots = 20 KB, fits in most GPUs)
- Handles sparse fractal distributions well

**Cons:**
- Hash collisions require probing (slower)
- Complex atomic logic (compare-exchange loop)
- May not fit all hits if fractal is very dense in one region

---

## Simplest Approach: Per-Thread Local Accumulation

### Concept
Each thread maintains a **small local buffer** for recently hit pixels, flushes to global when full.

```wgsl
struct LocalPixel {
    pixel_idx: u32,
    r: u32,
    g: u32,
    b: u32,
    density: u32,
}

// Per-thread private storage (not shared!)
var local_cache: array<LocalPixel, 16>;  // 16 pixels × 20 bytes = 320 bytes per thread
var cache_size: u32 = 0;

fn accumulate_pixel(pixel: vec2<i32>, color: vec3<f32>) {
    let pixel_idx = u32(pixel.y) * params.width + u32(pixel.x);

    // Check if pixel already in cache
    for (var i = 0u; i < cache_size; i++) {
        if (local_cache[i].pixel_idx == pixel_idx) {
            // Accumulate to cache (no atomics!)
            local_cache[i].r += r;
            local_cache[i].g += g;
            local_cache[i].b += b;
            local_cache[i].density += 1u;
            return;
        }
    }

    // Not in cache - add if space
    if (cache_size < 16) {
        local_cache[cache_size] = LocalPixel(pixel_idx, r, g, b, 1u);
        cache_size++;
    } else {
        // Cache full - flush and add
        flush_cache();
        local_cache[0] = LocalPixel(pixel_idx, r, g, b, 1u);
        cache_size = 1;
    }
}

fn flush_cache() {
    // Write all cached pixels to global histogram
    for (var i = 0u; i < cache_size; i++) {
        let base_idx = local_cache[i].pixel_idx * 4u;
        atomicAdd(&histogram[base_idx + 0u], local_cache[i].r);
        atomicAdd(&histogram[base_idx + 1u], local_cache[i].g);
        atomicAdd(&histogram[base_idx + 2u], local_cache[i].b);
        atomicAdd(&histogram[base_idx + 3u], local_cache[i].density);
    }
    cache_size = 0;
}
```

**Pros:**
- MUCH simpler than hash tables or tiling
- No workgroup memory limits (private per-thread)
- Automatically reduces global atomics by cache hit rate
- No synchronization needed (each thread independent)

**Cons:**
- Cache size limited (16-32 pixels typical)
- If fractal hits many unique pixels, cache thrashes
- Still does global atomics on cache misses

**Expected Performance:**
- If cache hit rate is 75%: 4× fewer global atomics
- If cache hit rate is 90%: 10× fewer global atomics
- Fractal flames tend to have locality (same pixels hit repeatedly)

---

## Recommended Approach: Per-Thread Local Cache

**Why:**
1. **Simplest to implement** - no complex synchronization or hash tables
2. **Fits in register file** - no workgroup memory limits
3. **Automatic adaptation** - works for sparse and dense fractals
4. **Fractal flames have locality** - orbit often revisits nearby pixels
5. **Incremental improvement** - can test/tune cache size easily

**Implementation Steps:**

### Step 1: Add Local Cache to Compute Shaders
```wgsl
// shaders/core/main_2d.wgsl (and main_3d.wgsl)

struct LocalPixel {
    pixel_idx: u32,
    r: u32,
    g: u32,
    b: u32,
    density: u32,
}

const CACHE_SIZE: u32 = 16u;  // Tunable

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    // Local cache (per-thread private storage)
    var local_cache: array<LocalPixel, CACHE_SIZE>;
    var cache_count: u32 = 0u;

    // ... existing init code ...

    // Iterate
    for (var i = 0u; i < params.iterations_per_thread; i++) {
        // ... existing iteration code ...

        if (i >= params.burn_in && pixel_in_bounds) {
            // NEW: Accumulate to local cache
            let pixel_idx = u32(pixel.y) * params.width + u32(pixel.x);

            // Try to find in cache
            var found = false;
            for (var c = 0u; c < cache_count; c++) {
                if (local_cache[c].pixel_idx == pixel_idx) {
                    // Cache hit - accumulate locally (no atomics!)
                    local_cache[c].r += r;
                    local_cache[c].g += g;
                    local_cache[c].b += b;
                    local_cache[c].density += 1u;
                    found = true;
                    break;
                }
            }

            if (!found) {
                // Cache miss
                if (cache_count < CACHE_SIZE) {
                    // Add to cache
                    local_cache[cache_count] = LocalPixel(pixel_idx, r, g, b, 1u);
                    cache_count++;
                } else {
                    // Cache full - flush oldest, add new
                    flush_entry(&local_cache[0]);  // Write to global
                    // Shift cache (or use circular buffer)
                    for (var c = 0u; c < CACHE_SIZE - 1; c++) {
                        local_cache[c] = local_cache[c + 1u];
                    }
                    local_cache[CACHE_SIZE - 1] = LocalPixel(pixel_idx, r, g, b, 1u);
                }
            }
        }
    }

    // Flush remaining cache entries
    for (var c = 0u; c < cache_count; c++) {
        flush_entry(&local_cache[c]);
    }
}

fn flush_entry(entry: ptr<function, LocalPixel>) {
    let base_idx = (*entry).pixel_idx * 4u;
    atomicAdd(&histogram[base_idx + 0u], (*entry).r);
    atomicAdd(&histogram[base_idx + 1u], (*entry).g);
    atomicAdd(&histogram[base_idx + 2u], (*entry).b);
    atomicAdd(&histogram[base_idx + 3u], (*entry).density);
}
```

### Step 2: Tune Cache Size
Test different cache sizes:
- **8 pixels**: 160 bytes per thread, low memory, may thrash
- **16 pixels**: 320 bytes per thread, balanced
- **32 pixels**: 640 bytes per thread, better hit rate but more memory

### Step 3: Measure Performance
Compare frame rates:
- Baseline (current): X FPS
- With cache size 8: ? FPS
- With cache size 16: ? FPS
- With cache size 32: ? FPS

### Step 4: Analyze Cache Hit Rate (Optional)
Add debug counters to measure effectiveness:
```wgsl
var cache_hits: u32 = 0u;
var cache_misses: u32 = 0u;

// After iteration loop:
// Hit rate = cache_hits / (cache_hits + cache_misses)
```

---

## Performance Expectations

**Best Case** (90% cache hit rate):
- Global atomics reduced by 10×
- Frame rate improvement: ~5× (not full 10× due to cache management overhead)
- Approximate: 30 FPS → 150 FPS (close to original textureStore performance!)

**Realistic Case** (70-80% cache hit rate):
- Global atomics reduced by 3-5×
- Frame rate improvement: ~2-3×
- Approximate: 30 FPS → 60-90 FPS (acceptable improvement!)

**Worst Case** (50% cache hit rate):
- Global atomics reduced by 2×
- Frame rate improvement: ~1.5×
- Approximate: 30 FPS → 45 FPS (still better than current!)

---

## Risks and Mitigations

**Risk 1: Register Pressure**
- Large cache arrays may spill to local memory
- **Mitigation**: Start with small cache (8-16 pixels), tune up if needed

**Risk 2: Cache Thrashing**
- Fractal may hit many unique pixels, causing constant flushes
- **Mitigation**: Implement LRU or random eviction instead of FIFO

**Risk 3: Complexity**
- Cache management adds code complexity
- **Mitigation**: Isolate in helper functions, thorough testing

**Risk 4: Marginal Improvement**
- If cache hit rate is low, improvement may not be worth complexity
- **Mitigation**: Measure first with simple cache, abandon if < 2× improvement

---

## Alternative If Local Cache Fails: Hybrid Mode

If local cache doesn't provide sufficient improvement, implement a **quality vs speed toggle**:

```rust
enum RenderQuality {
    Quality,   // Histogram (current, slow, perfect)
    Speed,     // TextureStore (old, fast, color noise)
}
```

**User Control:**
- UI toggle in settings
- Hotkey (Q for quality, S for speed)
- Auto-switch based on interaction (speed while dragging, quality when idle)

**Benefits:**
- Users choose their own trade-off
- Animation can use speed mode
- No forced performance penalty

**Cons:**
- Two render paths to maintain
- Code complexity

---

## Recommendation

**Step 1:** Implement per-thread local cache (8-16 pixel cache)
- Simple, low risk, likely effective
- Expected: 2-3× frame rate improvement

**Step 2:** If insufficient, increase cache size to 32 pixels
- Test impact on register pressure
- Measure hit rate

**Step 3:** If still insufficient, implement hybrid mode
- Quality mode: Histogram (current)
- Speed mode: TextureStore (old approach from git history)
- Let users choose

---

## Success Criteria

**Minimum Acceptable:**
- 2× frame rate improvement (30 FPS → 60 FPS)
- No visible quality degradation
- No increase in memory usage

**Target:**
- 3× frame rate improvement (30 FPS → 90 FPS)
- Same Apophysis-quality rendering
- Minimal code complexity

**Stretch Goal:**
- 5× frame rate improvement (30 FPS → 150 FPS)
- Near-original textureStore performance
- Maintain histogram correctness

---

## Next Steps

1. **Document current performance** - Measure exact FPS with current histogram approach
2. **Implement local cache** - Start with 16-pixel cache in main_2d.wgsl
3. **Test and measure** - Compare FPS, verify visual quality
4. **Tune cache size** - Test 8, 16, 32 pixel caches
5. **Apply to 3D shader** - Replicate changes in main_3d.wgsl
6. **Update documentation** - Document final cache size and performance gains
