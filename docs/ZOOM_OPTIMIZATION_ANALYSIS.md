# Zoom and Performance: The Real Story

## TL;DR - The Truth About Zoom

**Zoom does NOT slow down rendering.** Render time is purely a function of iteration count.

**What ACTUALLY happens:**
- At high zoom, you need MORE iterations to maintain image quality (more samples in smaller visible region)
- The rendering speed (iterations per second) is the SAME at all zoom levels
- Initial benchmark confusion: Test configs happened to have higher `max_iterations` at higher zoom levels (coincidence, not causation)

## Question 1: Does zoom create a performance hit?

### Answer: NO - Zoom has NO DIRECT performance impact

**Benchmark data revealed:**
```
Config "complex" (zoom 1.0):  ~1 billion iterations   → 170ms
Config "simple4" (zoom 21.1): ~10 billion iterations  → 1341ms (8× longer for 10× iterations)
Config "simple3" (zoom 25.5): ~40 billion iterations  → 5900ms (35× longer for 40× iterations)
```

**The pattern:** Render time scales linearly with iteration count, NOT with zoom level.

**Calculation:**
- 170ms / 1 billion = 0.17 μs per million iterations
- 1341ms / 10 billion = 0.134 μs per million iterations
- 5900ms / 40 billion = 0.148 μs per million iterations

**These are all roughly the same throughput!** (~6-7 billion iterations/second)

### What We Initially Thought (WRONG)

❌ "High zoom makes each iteration slower"
❌ "Wasted iterations outside viewport slow things down"
❌ "Atomic contention is worse at high zoom"

### What's Actually True (CORRECT)

✅ Each iteration takes the SAME time regardless of zoom
✅ Iterations outside viewport are still computed at full speed
✅ Render time = (# of iterations) ÷ (throughput)
✅ Throughput (~6 Giter/sec) is CONSTANT across all zoom levels

## Why Does Zoom Affect *Quality* Not *Speed*?

### The Quality Problem at High Zoom

At high zoom, the visible region is smaller:

```
Zoom 1.0:  Viewport sees [-1.0, 1.0] × [-1.0, 1.0] (4.0 square units)
Zoom 25.5: Viewport sees [-0.04, 0.04] × [-0.04, 0.04] (0.0064 square units)
```

**Visible area at zoom 25.5 is ~625× smaller!**

### Statistical Sampling Problem

Fractal flame algorithm generates points uniformly across fractal space (roughly [-2, 2] × [-2, 2]):

**At zoom 1.0:**
- 1 billion iterations
- ~95% land in viewport
- ~950 million visible samples
- Result: Smooth, high-quality image

**At zoom 25.5 (with SAME 1 billion iterations):**
- 1 billion iterations
- ~1.5% land in viewport (625× smaller area)
- ~15 million visible samples
- Result: Grainy, low-quality image

**To match zoom 1.0 quality, you need ~625× more iterations!**

This is why the test configs had higher `max_iterations` at high zoom - someone manually adjusted them to maintain quality.

## Question 2: Are we using Apophysis "Scale" method?

### Answer: YES, and that's the correct choice

**Evidence from code:**

#### 1. Apophysis XML Import (apophysis_xml.rs:170)
```rust
// Convert Apophysis scale/center to our zoom/pan
// Apophysis: scale = pixels per unit, where scale 200 ≈ zoom 1.0
let zoom = scale / 200.0; // Apophysis scale 200.0 = our zoom 1.0
```

**Confirmed:** We map Apophysis `scale` → our `zoom` parameter.

#### 2. Shader Application (utilities.wgsl:128)
```wgsl
// Apply zoom
transformed = transformed * params.zoom;
```

**Linear multiplication** = Apophysis "Scale" behavior

#### 3. UI Behavior (view.rs:24)
```rust
if ui.button("➕ Zoom In").clicked() {
    *zoom *= 1.5;  // Multiplicative scaling
    *view_changed = true;
}
```

**Multiplicative scaling** = Apophysis "Scale"

### Apophysis Scale vs Zoom Comparison

| Parameter | Type | Quality Adjustment | Speed | Use Case |
|-----------|------|-------------------|-------|----------|
| **Scale** (ours) | Linear | Manual | Instant | Quick preview/editing |
| **Zoom** (not implemented) | Logarithmic | Automatic (2^(2×Zoom)) | Slow | Animation rendering |

**Our implementation matches Apophysis "Scale" exactly** - fast interactive navigation with manual quality control.

## The Real Performance Characteristics

### What Affects Render Time

✅ **Iteration count** - Direct 1:1 relationship
✅ **Shader complexity** - More variations = slightly slower
✅ **Histogram vs textureStore** - Naive atomic is 8-20% faster at high zoom
❌ **Zoom level** - Has NO direct performance impact

### Histogram Performance Insight

**Why naive histogram gets faster at high zoom (relative to textureStore):**

At high zoom, the ~1-2% of iterations that DO land in viewport tend to hit the same pixels repeatedly:
- High zoom = small visible region = high pixel reuse
- textureStore: Race conditions cause "last write wins" data loss
- Naive atomic: All samples accumulate correctly
- GPU may optimize repeated atomics to same addresses (L1 cache)

**Result:** At zoom 25.5, naive histogram is **20% faster** than textureStore!

This has nothing to do with zoom making atomics faster - it's about correctness. textureStore wastes visible samples through race conditions, histogram doesn't.

## Potential Optimizations

### 1. **Quality Hint UI (EASY - Recommended)**

**Idea:** When user changes zoom, show suggested iteration adjustment.

```
Zoom increased to 25.5× (625× smaller area)
💡 Consider increasing iterations by ~625× to maintain quality
[Apply] [Dismiss]
```

**Formula:** `suggested_iterations = base_iterations × (new_zoom / old_zoom)²`

**Pros:**
- Educational (teaches zoom/quality relationship)
- Preserves user control
- No automatic behavior changes

**Cons:**
- Requires manual user action
- Could be dismissed as annoying

**Verdict:** Easy win, minimal code change.

### 2. **Adaptive Iteration Count (MEDIUM - Optional Feature)**

**Idea:** Optional "Auto Quality" mode that adjusts `max_iterations` automatically when zoom changes.

```rust
if auto_quality_mode {
    let adjusted_iterations = base_iterations * (zoom / 1.0).powi(2);
    self.max_iterations = Some(adjusted_iterations);
}
```

**Pros:**
- Matches Apophysis "Zoom" behavior
- Consistent quality at all zoom levels
- Simple to implement

**Cons:**
- Takes control away from user
- Could surprise users who don't expect automatic adjustments
- Needs clear UI indication that auto-adjustment is happening

**Verdict:** Worth prototyping as opt-in feature.

### 3. **Document Current Behavior (EASY - Do Now)**

Update CLAUDE.md and user documentation:
- Zoom is instant (no performance cost)
- Higher zoom = need more iterations for same quality
- Formula: `quality ∝ iterations / (zoom²)`
- Recommend manual iteration adjustment for high zoom

### 4. **Implement True Apophysis "Zoom" (HARD - Future Feature)**

**Idea:** Add separate "Zoom" mode alongside current "Scale" mode.

**Behavior:**
- Zoom = 0: Default quality (100%)
- Zoom = 1: Auto-increase iterations by 4× (2^(2×1) = 4)
- Zoom = 2: Auto-increase iterations by 16× (2^(2×2) = 16)
- Zoom = 3: Auto-increase iterations by 64× (2^(2×3) = 64)

**Challenges:**
- Need UI to switch between Scale and Zoom modes
- Need to track "base iteration count" for calculations
- Logarithmic slider behavior is less intuitive
- Animation rendering would benefit, but adds complexity

**Verdict:** Nice-to-have for animation workflows, not critical for interactive editing.

## Corrected Understanding

### What We Learned

1. **Zoom has NO direct performance impact** - throughput is constant
2. **Iteration count is the ONLY factor** for render time
3. **Quality degrades at high zoom** due to statistical sampling (fewer visible samples)
4. **Histogram atomic accumulation is faster** than textureStore at high zoom (correctness matters!)
5. **Test configs were misleading** - zoom and max_iterations coincidentally correlated

### Why This Matters

**Before:** "Zoom is slow, we need to optimize the shader!"
**After:** "Zoom is instant, users need more iterations for quality!"

This changes the optimization strategy from technical (shader optimization) to UX (helping users understand quality trade-offs).

## Recommendations

### Immediate (Do Now)
1. ✅ Update documentation to reflect correct understanding
2. ✅ Remove false assumptions about zoom causing slowdown
3. ⏳ Add quality hint UI when zoom changes

### Short Term (Next Sprint)
1. ⏳ Prototype "Auto Quality" mode (optional feature)
2. ⏳ Add user documentation about zoom/quality relationship
3. ⏳ Create example configs showing quality scaling

### Long Term (Future Consideration)
1. ⏳ Implement true Apophysis "Zoom" mode (logarithmic)
2. ⏳ Add animation workflow support (keyframe interpolation with quality preservation)

## Conclusion

**The "zoom performance problem" doesn't exist.** What DOES exist is a quality/sampling problem that requires user education and better UI hints.

Our current implementation is **correct and optimal** for interactive editing. The naive histogram optimization already provides the best possible performance. The only remaining work is helping users understand the zoom/quality relationship and providing tools to manage it.
