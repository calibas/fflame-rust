# Spatial Path Correlation - Experimental Proposal

**Status:** Experimental idea - not yet implemented
**Date:** 2025-10-28
**Complexity:** Very High
**Potential Impact:** Revolutionary (enables extreme zooms, adaptive sampling, path steering)

---

## Core Insight

**Hypothesis:** Specific transform sequences (paths) correlate to specific spatial regions of the fractal. By tracking which paths contribute to which pixels, we can:

1. **Detect exhausted paths** - Paths that converge to single pixels or go offscreen
2. **Focus sampling** - Spawn threads on productive paths only
3. **Enable extreme zooms** - Limited path sets for zoomed regions
4. **Start deep** - Begin iteration chains N steps in, skipping converged regions
5. **Adaptive switching** - Change paths when current path is exhausted

---

## Theoretical Foundation

### Spatial Path Correlation

**Observation 1: IFS Attractors Have Structure**

Fractal flames are attractors of Iterated Function Systems (IFS). Each transform maps the entire space to a subset:

```
Transform A: Maps space → Region A (subset of space)
Transform B: Maps space → Region B (subset of space)

Path "AB": Maps space → Region A → Subregion AB
Path "BA": Maps space → Region B → Subregion BA
```

**Result:** Different paths lead to different spatial regions!

**Observation 2: Self-Similarity**

Fractals are self-similar - zooming reveals similar structure:

```
Zoom level 1: Paths [A, B, AA, AB, BA, BB] → Full fractal
Zoom level 10: Paths [AAAA..., AAAB..., ...] → Zoomed region

At extreme zoom, only a SUBSET of paths contribute to visible region.
```

**Observation 3: Path Convergence**

Some paths converge to **fixed points** or **cycles**:

```
Path AAAAAAA... → Converges to fixed point of transform A
Path ABABAB... → Cycles between two regions (period-2 orbit)

These paths contribute to SINGLE PIXELS (or small clusters).
```

---

## Proposed Architecture

### Phase 1: Path Tracking Per Pixel

**Data Structure:**

```rust
struct PathTracker {
    // Per-pixel path accumulation
    pixel_paths: Vec<CompressedPathSet>,  // One per pixel

    // Global path statistics
    path_contributions: HashMap<PathId, PathStats>,
}

struct CompressedPathSet {
    // Bitfield for 2 transforms (1 bit per iteration)
    // For N transforms, use log2(N) bits per iteration
    paths: BitVec,  // Compressed representation

    // Statistics
    hit_count: u32,       // How many times this pixel was hit
    unique_paths: u32,    // How many different paths hit this pixel
}

struct PathStats {
    total_hits: u64,           // Total times this path was sampled
    pixels_hit: u32,           // Number of unique pixels hit
    goes_offscreen: bool,      // Path escapes viewport
    converged_to_point: bool,  // Path converges to single pixel
}
```

### Phase 2: Path Encoding

**2 Transforms Example:**

```
Path: A → B → A → A → B → B → A
Bits: 0   1   0   0   1   1   0

Stored as: 0b0100110 (7 bits for 7 iterations)
```

**N Transforms Example:**

```
4 transforms (A, B, C, D) → 2 bits per iteration
Path: A → B → D → C → A → B
Bits: 00  01  11  10  00  01

Stored as: 0b00011110001 (12 bits for 6 iterations)
```

**Storage Requirements:**

```
Iterations | 2 Xforms | 4 Xforms | 8 Xforms | 16 Xforms
-----------|----------|----------|----------|----------
10         | 10 bits  | 20 bits  | 30 bits  | 40 bits
20         | 20 bits  | 40 bits  | 60 bits  | 80 bits
30         | 30 bits  | 60 bits  | 90 bits  | 120 bits

Per pixel @ 30 iterations:
- 2 xforms: 4 bytes
- 4 xforms: 8 bytes
- 8 xforms: 12 bytes

Resolution | 2 Xforms  | 4 Xforms  | 8 Xforms
-----------|-----------|-----------|----------
800×600    | 1.9 MB    | 3.8 MB    | 5.7 MB
1920×1080  | 8.3 MB    | 16.6 MB   | 24.9 MB
```

**Feasible!** Memory requirements are reasonable.

### Phase 3: GPU Path Recording

**Modified Compute Shader:**

```wgsl
struct PixelPathData {
    path_bits: u32,      // Compressed path (up to 32 iterations for 2 xforms)
    hit_count: u32,      // Number of hits
    unique_paths: u32,   // Number of different paths seen
}

@group(0) @binding(7)
var<storage, read_write> pixel_paths: array<PixelPathData>;

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    var rng = rng_init(thread_id, params.seed);
    var current = random_start_point(&rng);

    // Build path bitfield
    var path_bits = 0u;
    var path_length = 0u;

    for (var i = 0u; i < params.iterations_per_thread; i++) {
        let xform_idx = select_transform(rng_nextf(&rng));

        // Record path (for 2 transforms, 1 bit per iteration)
        if (params.num_transforms == 2u) {
            path_bits |= (xform_idx << path_length);
            path_length += 1u;
        }
        // For N transforms, use log2(N) bits
        else {
            let bits_per_step = u32(ceil(log2(f32(params.num_transforms))));
            path_bits |= (xform_idx << (path_length * bits_per_step));
            path_length += 1u;
        }

        // Apply transform
        current = apply_transform(current, xform_idx, ...);

        // Plot and record path
        if (i >= params.burn_in) {
            let pixel = world_to_pixel(current);
            if (in_bounds(pixel)) {
                // Accumulate to histogram (normal)
                plot_to_histogram(pixel, color);

                // Record path for this pixel
                let pixel_idx = pixel.y * params.width + pixel.x;

                // Atomic increment hit count
                atomicAdd(&pixel_paths[pixel_idx].hit_count, 1u);

                // Store path (simplified - real version needs collision handling)
                pixel_paths[pixel_idx].path_bits = path_bits;
            }
        }
    }
}
```

### Phase 4: Path Analysis (CPU)

**After each frame (or every N frames):**

```rust
fn analyze_paths(pixel_paths: &[PixelPathData], width: u32, height: u32) -> PathAnalysis {
    let mut path_stats: HashMap<PathId, PathStats> = HashMap::new();

    for (pixel_idx, pixel_data) in pixel_paths.iter().enumerate() {
        let path_id = PathId(pixel_data.path_bits);

        let stats = path_stats.entry(path_id).or_insert(PathStats::default());
        stats.total_hits += pixel_data.hit_count as u64;
        stats.pixels_hit += 1;

        // Detect convergence
        if pixel_data.hit_count > 1000 && stats.pixels_hit == 1 {
            stats.converged_to_point = true;
        }
    }

    // Detect offscreen paths
    // (Paths that were sampled but never hit any pixel)

    PathAnalysis { path_stats }
}
```

---

## Use Cases and Optimizations

### 1. Exhausted Path Detection

**Problem:** Some paths converge to single pixels or go offscreen. Threads waste time on these paths.

**Solution:**

```rust
// Blacklist converged/offscreen paths
let blacklisted_paths: HashSet<PathId> = path_stats
    .iter()
    .filter(|(_, stats)| stats.converged_to_point || stats.goes_offscreen)
    .map(|(id, _)| *id)
    .collect();

// GPU: Skip blacklisted paths
@compute
fn main_optimized(...) {
    loop {
        // Select transform sequence
        let path_id = generate_path(&rng, DEPTH);

        // Check if blacklisted
        if (is_blacklisted(path_id)) {
            continue;  // Try different path
        }

        // Use this path
        iterate_path(path_id);
        break;
    }
}
```

**Impact:** Avoid wasted computation on unproductive paths.

### 2. Adaptive Path Switching

**Problem:** Threads might hit exhausted paths during iteration.

**Solution:**

```rust
@compute
fn main_adaptive(...) {
    var current = start_point;
    var iterations_on_current_path = 0u;

    for (var i = 0u; i < params.iterations_per_thread; i++) {
        // Check if current path is exhausted
        let pixel = world_to_pixel(current);
        if (in_bounds(pixel)) {
            let pixel_idx = pixel.y * params.width + pixel.x;
            let pixel_saturation = f32(pixel_paths[pixel_idx].hit_count) / params.target_samples;

            // If pixel is saturated (>95% of target), switch paths
            if (pixel_saturation > 0.95) {
                // Jump to a different path
                current = teleport_to_undersampled_region(&rng);
                iterations_on_current_path = 0u;
            }
        }

        // Continue iteration
        current = apply_transform(current, ...);
        iterations_on_current_path += 1u;
    }
}
```

**Impact:** Dynamically reallocate computation to undersampled regions.

### 3. Extreme Zoom Support

**Problem:** At extreme zoom levels, most paths don't contribute to visible region.

**Solution:**

```rust
// Analyze which paths contribute to viewport
fn paths_in_viewport(
    pixel_paths: &[PixelPathData],
    viewport: Rect,
    width: u32,
    height: u32
) -> HashSet<PathId> {
    let mut productive_paths = HashSet::new();

    for y in viewport.y..viewport.y + viewport.height {
        for x in viewport.x..viewport.x + viewport.width {
            let pixel_idx = y * width + x;
            let path_id = PathId(pixel_paths[pixel_idx].path_bits);
            productive_paths.insert(path_id);
        }
    }

    productive_paths
}

// GPU: Only sample productive paths
@compute
fn main_zoomed(...) {
    // Generate path
    let path_id = generate_path(&rng, DEPTH);

    // Check if this path contributes to viewport
    if (!is_productive_path(path_id)) {
        // Resample
        continue;
    }

    // Use this path
    iterate_path(path_id);
}
```

**Impact:** At 1000× zoom, only sample the ~1% of paths that contribute to visible region.

### 4. Deep Starting Points

**Problem:** First N iterations might always lead to same region (wasted burn-in).

**Solution:**

```rust
// Pre-compute "interesting" starting points from different path prefixes
let starting_points = precompute_deep_starts(&flame, depth: 20);

// GPU: Start from deep points
@compute
fn main_deep_start(...) {
    // Instead of random start, use pre-computed point
    let start_path_id = rng_next(&rng) % starting_points.len();
    var current = starting_points[start_path_id].position;
    var color_index = starting_points[start_path_id].color;

    // Continue from here (already 20 iterations deep)
    for (var i = 0u; i < params.iterations_per_thread; i++) {
        current = apply_transform(current, ...);
        plot_to_histogram(current, ...);
    }
}
```

**Impact:** Skip burn-in entirely, start directly on attractor.

### 5. Importance Sampling

**Problem:** Not all regions need equal sampling density.

**Solution:**

```rust
// Weight paths by their "importance" (contribution to visible detail)
fn compute_path_importance(
    path_stats: &HashMap<PathId, PathStats>,
    viewport: Rect
) -> HashMap<PathId, f32> {
    path_stats
        .iter()
        .map(|(path_id, stats)| {
            // Higher importance = more pixels hit, more detail
            let importance = stats.pixels_hit as f32 / stats.total_hits as f64;
            (*path_id, importance)
        })
        .collect()
}

// GPU: Sample paths proportional to importance
@compute
fn main_importance_sampled(...) {
    let path_id = sample_weighted_path(&rng, path_importance_table);
    iterate_path(path_id);
}
```

**Impact:** Focus computation on high-detail regions.

---

## Implementation Challenges

### 1. **Path Collision Handling**

**Problem:** Multiple paths can hit the same pixel. How to store multiple paths per pixel?

**Solutions:**

**Option A: Store most recent path (simplest)**
```rust
// Just overwrite with latest path
pixel_paths[pixel_idx].path_bits = current_path;
```

**Option B: Store most common path**
```rust
// Use hash table or bloom filter to track path frequency
struct PixelPathData {
    primary_path: u32,
    path_histogram: [u32; 16],  // Top 16 paths
}
```

**Option C: Store path distribution**
```rust
// Probabilistic data structure
struct PixelPathData {
    path_sketch: CountMinSketch,  // Probabilistic counter
}
```

**Recommendation:** Start with Option A, upgrade to Option B if needed.

### 2. **Limited Bit Storage**

**Problem:** 32-bit storage limits path depth:
- 2 transforms: 32 iterations max
- 4 transforms: 16 iterations max
- 8 transforms: 10 iterations max

**Solutions:**

**Option A: Use 64-bit or 128-bit storage**
```rust
struct PixelPathData {
    path_bits_low: u32,
    path_bits_high: u32,  // 64 bits total
}
```

**Option B: Hash long paths**
```rust
fn hash_path(path: &[usize]) -> u32 {
    // Hash instead of storing full path
    // Collisions possible but unlikely
    murmur3_hash(path)
}
```

**Option C: Store path prefix only**
```rust
// Only store first 20 iterations
// Sufficient for most correlation analysis
```

**Recommendation:** Option C for simplicity, Option A for precision.

### 3. **GPU Atomic Constraints**

**Problem:** Recording paths requires atomic operations on path data structure.

**Challenges:**
- Atomic operations are slow
- Limited atomic types (u32, i32 only)
- Cannot atomically update complex structures

**Solutions:**

**Option A: Separate buffers**
```rust
// One buffer per path metric
@group(0) @binding(7) var<storage, read_write> path_bits: array<u32>;
@group(0) @binding(8) var<storage, read_write> hit_counts: array<atomic<u32>>;
```

**Option B: Lock-free path recording**
```rust
// Use atomic CAS (compare-and-swap) loop
fn record_path_atomic(pixel_idx: u32, new_path: u32) {
    loop {
        let old_path = atomicLoad(&path_bits[pixel_idx]);
        if (atomicCompareExchangeWeak(&path_bits[pixel_idx], old_path, new_path).exchanged) {
            break;
        }
    }
}
```

**Option C: Post-process on CPU**
```rust
// Record paths in per-thread buffer during compute
// Merge on CPU after GPU finishes
```

**Recommendation:** Option C for correctness, Option A for performance.

### 4. **Path Analysis Overhead**

**Problem:** Analyzing paths on CPU after every frame is expensive.

**Solutions:**

**Option A: Analyze every N frames**
```rust
if (frame_count % ANALYSIS_INTERVAL == 0) {
    analyze_paths();
}
```

**Option B: Incremental analysis**
```rust
// Update statistics incrementally instead of recomputing
```

**Option C: GPU-side analysis**
```rust
// Compute path statistics on GPU using reduction
```

**Recommendation:** Option A (every 60 frames = 1 second at 60 FPS).

### 5. **Viewport Changes**

**Problem:** When viewport changes (pan/zoom), path correlations change.

**Solutions:**

**Option A: Invalidate on viewport change**
```rust
if (viewport_changed) {
    clear_path_data();
    rebuild_from_scratch();
}
```

**Option B: Transform-space correlation**
```rust
// Track paths in transform space (invariant to viewport)
// Only viewport projection changes
```

**Option C: Incremental update**
```rust
// Keep old data, gradually replace with new samples
```

**Recommendation:** Option B for zoom, Option A for major changes.

---

## Feasibility Analysis

### Technical Feasibility: ⭐⭐⭐ (3/5)

**Pros:**
- Memory requirements are reasonable (2-25 MB)
- Bit encoding is straightforward
- GPU can record paths during normal iteration
- Path analysis on CPU is fast enough

**Cons:**
- Atomic operations add overhead (~10-20% slowdown?)
- Path collision handling is complex
- Analysis logic is sophisticated
- Viewport dependency complicates things

### Implementation Complexity: ⭐⭐⭐⭐⭐ (5/5 - Very High)

**Challenges:**
- GPU atomic operations and synchronization
- Path encoding/decoding logic
- Statistical analysis algorithms
- Path-based sampling strategies
- Integration with existing renderer

**Estimated Effort:** 3-6 weeks for MVP, 2-3 months for production

### Potential Impact: ⭐⭐⭐⭐⭐ (5/5 - Revolutionary)

**If successful:**
- ✅ Extreme zoom capability (1000×+ zoom levels)
- ✅ Adaptive sampling (focus on detail regions)
- ✅ Faster convergence (avoid wasted paths)
- ✅ Path steering (controllable iteration)
- ✅ Novel rendering modes (path-based effects)

### Risk Assessment: ⭐⭐⭐⭐ (4/5 - High Risk)

**Major Risks:**
1. **Hypothesis might be wrong** - Spatial correlation might be weak
2. **Overhead might dominate** - Path tracking cost > benefit
3. **Implementation might be too complex** - Hard to get right
4. **Quality might suffer** - Blacklisting paths could create artifacts

---

## Testing Strategy

### Phase 1: Validate Hypothesis

**Goal:** Prove that spatial path correlation exists.

**Method:**
1. Implement CPU-only path recording
2. Render simple flame (2 transforms)
3. Visualize which paths hit which pixels
4. Measure correlation strength

**Success Criteria:**
- At least 70% of pixels have dominant path (>50% of hits)
- Different regions show different path patterns
- Zoomed regions show reduced path diversity

### Phase 2: GPU Path Recording

**Goal:** Record paths on GPU without breaking existing renderer.

**Method:**
1. Add path recording buffers
2. Modify compute shader to record paths
3. Verify path data is correct
4. Measure performance overhead

**Success Criteria:**
- Path recording overhead < 20%
- Path data matches CPU simulation
- No visual artifacts

### Phase 3: Path-Based Optimization

**Goal:** Use path data to improve rendering.

**Method:**
1. Implement one optimization (e.g., blacklist converged paths)
2. Measure performance improvement
3. Verify visual quality is maintained

**Success Criteria:**
- Performance improvement > path recording overhead
- Visual output identical to baseline
- Works across different flames

### Phase 4: Advanced Features

**Goal:** Implement extreme zoom, adaptive sampling, etc.

**Method:**
1. One feature at a time
2. Test on variety of flames
3. Measure quality and performance

**Success Criteria:**
- Each feature shows measurable benefit
- No regressions in quality or performance

---

## Alternative Approaches

### 1. **Hierarchical Sampling**

Instead of path-based, use spatial hierarchy:
- Quadtree of screen regions
- Sample more in high-detail regions
- Simpler than path tracking

### 2. **Adaptive Iteration Depth**

Instead of blacklisting paths, adjust iteration count:
- Converged regions: fewer iterations
- High-detail regions: more iterations
- No path tracking needed

### 3. **Importance Sampling via Density**

Use current density map to guide sampling:
- Low-density pixels: more samples
- High-density pixels: fewer samples
- Simpler implementation

---

## Open Questions

1. **How strong is spatial path correlation?**
   - Need empirical testing on real flames
   - May vary dramatically between flame types

2. **What is optimal path tracking depth?**
   - Too shallow: weak correlation
   - Too deep: storage explosion
   - Sweet spot: 10-30 iterations?

3. **How does this interact with weighted transform selection?**
   - Weighted paths are more likely
   - Does correlation still hold?

4. **Can we detect path correlation in real-time?**
   - Or do we need pre-analysis phase?

5. **What about 3D mode?**
   - Does spatial correlation exist in 3D?
   - Camera rotation affects correlation

6. **How does this handle animation?**
   - Path correlations change over time
   - Need dynamic updates?

---

## Success Criteria

### Minimum Viable Product (MVP)

- ✅ Path recording works on GPU
- ✅ Spatial path correlation is measurable (>50%)
- ✅ One optimization shows improvement (>10%)
- ✅ Performance overhead < 30%
- ✅ Visual quality unchanged

### Stretch Goals

- 🎯 Extreme zoom works (1000× zoom levels)
- 🎯 Adaptive sampling reduces noise
- 🎯 Path steering enables novel effects
- 🎯 Performance improvement > 50%

### Failure Conditions

- ❌ Spatial correlation too weak (<30%)
- ❌ Performance overhead too high (>50%)
- ❌ Implementation too complex (>3 months)
- ❌ Visual artifacts or quality loss

---

## Recommendation

**Worthiness:** ⭐⭐⭐⭐ (4/5 stars)

**Verdict:** Fascinating idea with revolutionary potential, but high risk and complexity.

**Pros:**
- Novel approach not seen in other flame renderers
- Could enable extreme zooms impossible with current approach
- Adaptive sampling could dramatically improve quality/performance
- Deep theoretical foundation (IFS attractor structure)

**Cons:**
- Hypothesis unproven - spatial correlation might be weak
- Very high implementation complexity
- Performance overhead uncertain
- May not generalize across all flame types

**Recommended Path:**

1. **Phase 0: Quick Validation (1 week)**
   - CPU-only proof-of-concept
   - Visualize path-to-pixel correlation
   - Measure correlation strength
   - **Decision point:** If correlation < 30%, abandon

2. **Phase 1: GPU Recording (2 weeks)**
   - If Phase 0 successful, implement GPU path tracking
   - Measure overhead
   - **Decision point:** If overhead > 50%, reconsider

3. **Phase 2: Single Optimization (2 weeks)**
   - Implement blacklist or adaptive switching
   - Measure benefit
   - **Decision point:** If benefit < overhead, stop

4. **Phase 3: Production (4-8 weeks)**
   - If Phase 2 successful, implement full system
   - Add extreme zoom, importance sampling, etc.

**Total Commitment:** Start with 1 week validation, expand if promising.

---

**Status:** Proposal - awaiting hypothesis validation
**Champion:** TBD
**Last Updated:** 2025-10-28

**Related Proposals:**
- [PROPOSAL-path-caching.md](PROPOSAL-path-caching.md) - Pre-computed path caching (complementary approach)
