# Path Caching for Flame Iteration - Experimental Proposal

**Status:** Experimental idea - not yet implemented
**Date:** 2025-10-28
**Complexity:** High
**Potential Impact:** Massive performance improvement (10-100×?)

---

## Core Insight

The fractal flame algorithm repeats the same mathematical calculations millions of times across threads. With a fixed set of transforms, the possible **iteration paths** are deterministic and can be pre-computed.

### The Redundancy Problem

**Current approach** (per thread, every frame):
```
Thread 1: Start → Transform A → Transform B → Transform A → ... (256 iterations)
Thread 2: Start → Transform B → Transform A → Transform B → ... (256 iterations)
...
Thread 8192: (256 iterations)

Total per frame: 8,192 threads × 256 iterations = 2,097,152 iterations
```

**Each iteration recalculates:**
- Affine transformation: 6 multiplies + 4 adds
- Variation blending: N variations × (variation function + weight multiply + accumulate)
- Color blending: 3-4 operations

**Key observation:** With 2 transforms and 256 iterations, many threads will follow **identical paths** through transform space!

---

## Path Explosion Analysis

### Number of Unique Paths

With **T transforms** and **N iterations**, the number of unique paths is:

```
Iterations | 2 Transforms | 3 Transforms | 4 Transforms | 10 Transforms
-----------|--------------|--------------|--------------|---------------
1          | 2            | 3            | 4            | 10
2          | 4            | 9            | 16           | 100
3          | 8            | 27           | 64           | 1,000
4          | 16           | 81           | 256          | 10,000
5          | 32           | 243          | 1,024        | 100,000
6          | 64           | 729          | 4,096        | 1,000,000
7          | 128          | 2,187        | 16,384       | 10,000,000
8          | 256          | 6,561        | 65,536       | 100,000,000
10         | 1,024        | 59,049       | 1,048,576    | (too large)
```

**Formula:** `paths = T^N`

### Feasible Caching Depth

**Memory constraints:**

Each cached point needs:
- Position: vec2 or vec3 (8-12 bytes)
- Color: vec3 (12 bytes) or color_index (4 bytes)
- Total: ~16-24 bytes per cached point

**Feasibility:**

```
Transforms | Iterations | Paths      | Memory @ 20 bytes | Feasible?
-----------|------------|------------|-------------------|----------
2          | 10         | 1,024      | 20 KB            | ✅ Trivial
2          | 15         | 32,768     | 640 KB           | ✅ Easy
2          | 20         | 1,048,576  | 20 MB            | ✅ Acceptable
3          | 8          | 6,561      | 128 KB           | ✅ Easy
3          | 10         | 59,049     | 1.15 MB          | ✅ Acceptable
4          | 6          | 4,096      | 80 KB            | ✅ Easy
4          | 8          | 65,536     | 1.28 MB          | ✅ Acceptable
4          | 10         | 1,048,576  | 20 MB            | ⚠️ Heavy
10         | 5          | 100,000    | 2 MB             | ✅ Acceptable
```

**Practical caching depth:** 8-15 iterations depending on transform count

---

## Proposed Architecture

### Phase 1: Path Pre-computation (CPU, once per flame change)

```rust
struct PathCache {
    // Indexed by path_id (computed from transform sequence)
    cached_points: Vec<CachedPoint>,
    cache_depth: u32,  // How many iterations are cached
}

struct CachedPoint {
    position: [f32; 2],      // Final position after N iterations
    color_index: f32,        // Color state after N iterations
    // Could also cache: color RGB, speed, etc.
}

fn precompute_paths(flame: &Flame, depth: u32) -> PathCache {
    let num_transforms = flame.transforms.len();
    let num_paths = num_transforms.pow(depth);
    let mut cache = PathCache {
        cached_points: Vec::with_capacity(num_paths),
        cache_depth: depth,
    };

    // Enumerate all possible path sequences
    for path_id in 0..num_paths {
        // Decode path_id into transform sequence
        let transform_sequence = decode_path(path_id, num_transforms, depth);

        // Simulate the path from origin
        let mut p = [0.0, 0.0];
        let mut color_index = 0.0;

        for &xform_idx in &transform_sequence {
            let xform = &flame.transforms[xform_idx];
            p = apply_transform(p, xform, &mut color_index);
        }

        cache.cached_points.push(CachedPoint {
            position: p,
            color_index,
        });
    }

    cache
}

fn decode_path(path_id: usize, num_transforms: usize, depth: u32) -> Vec<usize> {
    // Convert path_id to base-N number (where N = num_transforms)
    let mut sequence = Vec::with_capacity(depth as usize);
    let mut id = path_id;
    for _ in 0..depth {
        sequence.push(id % num_transforms);
        id /= num_transforms;
    }
    sequence
}
```

### Phase 2: GPU Iteration with Cache (WGSL)

```wgsl
struct CachedPoint {
    position: vec2<f32>,
    color_index: f32,
}

@group(0) @binding(6)
var<storage, read> path_cache: array<CachedPoint>;

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    var rng = rng_init(thread_id, params.seed);

    // Build path_id from first CACHE_DEPTH iterations
    var path_id = 0u;
    var multiplier = 1u;
    for (var i = 0u; i < CACHE_DEPTH; i++) {
        let xform_idx = select_transform(rng_nextf(&rng));
        path_id += xform_idx * multiplier;
        multiplier *= params.num_transforms;
    }

    // Load cached result instead of computing CACHE_DEPTH iterations
    let cached = path_cache[path_id];
    var current = cached.position;
    var color_index = cached.color_index;

    // Continue iterating from cached point for remaining iterations
    for (var i = CACHE_DEPTH; i < params.iterations_per_thread; i++) {
        let xform_idx = select_transform(rng_nextf(&rng));
        current = apply_transform(current, xform_idx, &color_index);

        // Plot point (after burn-in)
        if (i >= params.burn_in) {
            plot_to_histogram(current, color_index);
        }
    }
}
```

---

## Performance Analysis

### Current Cost

**Per thread, per frame:**
- 256 iterations × (affine + variations + color blend)
- Affine: ~10 ops
- Variations: ~50-200 ops (depends on active variations)
- Total: ~60-210 ops × 256 = 15,360 to 53,760 ops per thread

**Total per frame:**
- 8,192 threads × 15,360 ops = **125 million operations**
- 8,192 threads × 53,760 ops = **440 million operations**

### With Path Caching (depth = 10)

**Pre-computation cost (CPU, once per flame change):**
- 2 transforms: 1,024 paths × 10 iterations = 10,240 iterations
- 4 transforms: 1,048,576 paths × 10 iterations = 10.5M iterations
- **Done once, amortized over thousands of frames**

**Per thread, per frame:**
- 1 cache lookup (negligible)
- 246 iterations × (affine + variations + color blend)
- Total: ~60-210 ops × 246 = 14,760 to 51,660 ops per thread

**Savings:**
- ~600 ops per thread (256 - 246 = 10 iterations saved)
- 8,192 threads × 600 ops = **4.9 million operations saved per frame**
- At 60 FPS: **294 million operations saved per second**

### With Deeper Caching (depth = 20)

**Pre-computation cost:**
- 2 transforms: 1,048,576 paths × 20 iterations = 21M iterations (~10 seconds CPU)
- **Memory:** 20 MB for 2 transforms

**Per thread, per frame:**
- 1 cache lookup
- 236 iterations × operations
- Total: ~60-210 ops × 236 = 14,160 to 49,560 ops per thread

**Savings:**
- ~1,200 ops per thread (20 iterations saved)
- 8,192 threads × 1,200 ops = **9.8 million operations saved per frame**
- At 60 FPS: **588 million operations saved per second**

**Speedup estimate:** 10-20% faster per frame (conservative)

---

## Implementation Challenges

### 1. **Starting Point Dependency**

**Problem:** Paths are computed from origin `[0, 0]`, but threads start from random points.

**Solutions:**

**Option A: Origin-based cache (simplest)**
- All threads use cache computed from origin
- After cache depth, threads diverge naturally
- Works because attractor is invariant to starting point (after burn-in)

**Option B: Multiple starting points**
- Pre-compute paths from N different starting points
- Thread selects nearest starting point
- Higher memory cost (N× cache size)

**Option C: Burn-in before cache**
- Threads compute burn-in iterations normally
- Cache starts after burn-in completes
- Ensures threads are on attractor before caching

**Recommendation:** Start with Option A (simplest), test quality impact

### 2. **Weighted Transform Selection**

**Problem:** Transform selection is weighted random, not uniform.

**Current approach:**
```
weights = [0.5, 0.3, 0.2]  // Transform probabilities
select_transform(rand) → weighted selection
```

**Solutions:**

**Option A: Weighted path enumeration**
- Pre-compute all paths, but weight them by probability
- Cache includes: `(point, color, probability_weight)`
- Thread selects path based on weighted random
- Memory: +4 bytes per cached point

**Option B: Probability-stratified cache**
- Group paths by probability bracket
- Cache only high-probability paths (top 90%)
- Fall back to normal iteration for rare paths
- Reduced memory, covers most cases

**Option C: Ignore weights (uniform caching)**
- Cache assumes uniform transform selection
- Actual weighted selection happens after cache
- Quality impact: Unknown (needs testing)

**Recommendation:** Option A for correctness, Option C for simplicity

### 3. **Memory Management**

**Problem:** Large flames (10+ transforms) have exponential path growth.

**Solutions:**

**Adaptive cache depth:**
```rust
fn optimal_cache_depth(num_transforms: usize, memory_budget_mb: f32) -> u32 {
    let bytes_per_point = 20;
    let max_points = (memory_budget_mb * 1_000_000.0) / bytes_per_point as f32;

    // Solve: num_transforms^depth = max_points
    let depth = (max_points.log(num_transforms as f32)) as u32;
    depth.min(20).max(5)  // Clamp between 5-20
}
```

**Example:**
- 2 transforms, 20 MB budget → depth = 20 (1M paths)
- 10 transforms, 20 MB budget → depth = 6 (1M paths)

### 4. **Cache Invalidation**

**When to recompute cache:**
- Flame transforms changed (affine, variations, parameters)
- Transform weights changed
- Number of transforms changed
- Variation parameters changed

**When NOT to recompute:**
- View changed (zoom, pan, rotation)
- Color palette changed (color is cache-independent if using Transform mode)
- Tone mapping changed
- Render settings changed (iterations_per_thread, etc.)

**Implementation:**
```rust
struct PathCacheKey {
    flame_checksum: u64,  // Hash of all transforms
}

impl FlameRenderer {
    fn update_path_cache(&mut self, flame: &Flame) {
        let key = compute_flame_checksum(flame);
        if self.cache_key != key {
            self.path_cache = precompute_paths(flame, self.cache_depth);
            self.cache_key = key;
        }
    }
}
```

### 5. **GPU Storage Buffer Size Limits**

**Problem:** WGPU storage buffers have size limits (~128 MB typically).

**Solution:**
- Adaptive cache depth based on buffer limits
- Fall back to shallower cache for large flames
- Could use texture storage if buffer too small

---

## Variations and Extensions

### 1. **Partial Path Caching**

Instead of caching entire point trajectories, cache **variation application results**:

```rust
struct VariationCache {
    // For each variation, cache results for common input ranges
    cache: HashMap<(VariationId, QuantizedInput), Vec2>,
}
```

**Benefits:**
- Much smaller memory footprint
- Works for any number of transforms
- Variation functions are the expensive part

**Challenges:**
- Input space is continuous, needs quantization
- Cache hit rate depends on input distribution

### 2. **Incremental Cache Building**

Build cache **during rendering** instead of pre-computing:

```rust
// On first frame: render normally, record paths
// On subsequent frames: use recorded paths

struct AdaptiveCache {
    observed_paths: HashMap<PathId, CachedPoint>,
    cache_hits: u64,
    cache_misses: u64,
}
```

**Benefits:**
- No pre-computation delay
- Naturally focuses on high-probability paths
- Memory grows gradually

**Challenges:**
- Thread-safe cache updates (atomic or locks)
- Cache grows indefinitely without eviction policy

### 3. **Path Compression**

Store paths as deltas instead of absolute positions:

```rust
struct CompressedPath {
    base_point: Vec2,
    deltas: Vec<Vec2>,  // Differences between iterations
}
```

**Benefits:**
- Deltas compress better (smaller values)
- Could use 16-bit or 8-bit delta storage
- 2-4× memory reduction

### 4. **Probabilistic Caching**

Cache only **high-probability paths** (top 90% by weight):

```rust
fn precompute_weighted_paths(flame: &Flame, depth: u32, coverage: f32) -> PathCache {
    let all_paths = enumerate_paths(flame, depth);
    let sorted_by_probability = sort_by_weight(all_paths);

    let mut cumulative_prob = 0.0;
    let cached_paths: Vec<_> = sorted_by_probability
        .into_iter()
        .take_while(|path| {
            cumulative_prob += path.probability;
            cumulative_prob < coverage
        })
        .collect();

    PathCache::from(cached_paths)
}
```

**Benefits:**
- Covers most cases with much less memory
- 90% coverage might use 10% of full cache size

---

## Testing Strategy

### Phase 1: Proof of Concept

1. **Implement CPU path caching** (no GPU)
2. **Test on simple flames** (2-3 transforms)
3. **Verify visual output is identical** to non-cached version
4. **Measure CPU performance improvement**

### Phase 2: GPU Implementation

1. **Add storage buffer for cached paths**
2. **Modify compute shader** to use cache
3. **Test on GPU** with small cache (depth = 5-10)
4. **Verify no visual artifacts**

### Phase 3: Optimization

1. **Experiment with cache depth**
2. **Test weighted vs uniform caching**
3. **Measure memory vs performance tradeoff**
4. **Profile GPU performance improvement**

### Phase 4: Production

1. **Add UI controls** for cache depth
2. **Implement adaptive depth** based on transform count
3. **Add cache statistics** to debug UI
4. **Document performance characteristics**

---

## Open Questions

1. **Quality impact:** Does origin-based caching affect fractal quality?
   - Need side-by-side comparison at high zoom
   - May need burn-in before cache for quality

2. **Attractor convergence:** How many iterations needed to reach attractor?
   - Current burn-in is 20 iterations
   - Cache might need to start after attractor is reached

3. **Color accuracy:** How does color caching affect gradient smoothness?
   - Color evolves along path, may have discontinuities
   - May need to cache color separately or interpolate

4. **Weighted selection:** Is uniform caching "good enough"?
   - Weighted paths are complex to implement
   - Need to test if uniform caching produces acceptable results

5. **Animation:** How does cache behave during parameter morphing?
   - Cache invalidation frequency during animation
   - Amortization over frames might be poor

6. **3D mode:** Does caching work with Z coordinate?
   - 3D points need 3 coordinates (12 bytes vs 8 bytes)
   - Memory cost increases 50%

---

## Success Criteria

### Minimum Viable Product (MVP)

- ✅ 10-20% performance improvement for simple flames (2-4 transforms)
- ✅ Visual output identical to non-cached version
- ✅ Memory usage under 50 MB for cache
- ✅ Cache precomputation under 1 second

### Stretch Goals

- 🎯 50-100% performance improvement (2× faster)
- 🎯 Adaptive cache depth based on memory budget
- 🎯 Works with weighted transform selection
- 🎯 Scales to 10+ transform flames
- 🎯 Graceful fallback when cache too large

### Failure Conditions

- ❌ Visual artifacts or quality degradation
- ❌ Memory usage exceeds 200 MB
- ❌ Cache precomputation takes too long (>5 seconds)
- ❌ Performance improvement less than 5%

---

## Related Work

### Similar Techniques in Other Domains

**Path tracing:** Uses importance sampling and path reuse
**Neural networks:** Computation graphs and memoization
**Fractals:** Buddhabrot uses pre-computed escape paths
**Video games:** Precomputed radiance transfer, lightmaps

### Alternative Approaches

**GPU occupancy optimization:** Increase threads per workgroup
**Better variation functions:** Optimize hot path math
**SIMD:** Use wider vector operations
**Async compute:** Overlap compute with other work

---

## Recommendation

**Worthiness:** ⭐⭐⭐⭐ (4/5 stars)

**Pros:**
- Massive redundancy in current approach
- Memory requirements are manageable for common cases
- Implementation complexity is moderate
- Potential for 2-10× speedup

**Cons:**
- Quality impact unknown (needs testing)
- Cache invalidation complexity
- May not scale to very large flames (10+ transforms)
- Weighted selection adds complexity

**Next Steps:**
1. Implement CPU proof-of-concept (1-2 days)
2. Test visual quality and performance (1 day)
3. If promising, implement GPU version (3-5 days)
4. If successful, add adaptive depth and optimization (3-5 days)

**Total effort estimate:** 1-2 weeks for full implementation and testing

---

**Status:** Proposal - awaiting proof-of-concept implementation
**Champion:** TBD
**Last Updated:** 2025-10-28
