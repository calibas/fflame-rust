# Per-Pixel Convergence: Analysis and New Approach

## History: What Was Tried Before

### 1. Per-Pixel Adaptive Scaling (Abandoned - Commit 7271fcd)

**Goal:** Dynamically adjust encoding scale per pixel to prevent overflow while maximizing precision.

**Why it failed:**
- Architectural incompatibility with batched histogram accumulation
- Histogram contains mixed data from different scales (scale changes between frames in a batch)
- Decode can't determine which frames used which scales
- Creates cumulative errors → progressive darkening

**Key insight from failure:** Per-pixel systems that modify **encoding** are incompatible with batched accumulation.

### 2. Convergence Masking (Reverted - Commit 6b9d426)

**Goal:** Stop accumulating pixels once they reach a density threshold to prevent overflow.

**Implementation:** Check density threshold before accumulating:
```wgsl
if (prev.a < convergence_threshold) {
    // accumulate normally
} else {
    // don't accumulate (converged)
}
```

**Why it failed:**
- Overflow still happened: `threshold × scale ≤ 65,535` (u16 max)
- For quality, needed `threshold=10,000 × scale=50 = 500,000` (7.6× overflow)
- Low thresholds (≤1,310) caused black patches in dense areas
- **This was before u32 histogram** - overflow was the primary concern

**Current status:** This problem is SOLVED by u32 histogram (no overflow possible now)

---

## What's Different Now?

### 1. U32 Histogram Eliminates Overflow

**Before (convergence masking era):**
- u16 histogram channels (max 65,535)
- Overflow was the primary concern
- Had to choose between quality (scale=100) and safety (scale=10)

**Now:**
- u32 histogram channels (max 4.2 billion)
- **Overflow is no longer a concern**
- Can use high scales (100+) without overflow risk

### 2. Understanding blend_factor's Role

**Previous understanding:**
- `blend_factor` was just a mathematical necessity
- Focus was on preventing overflow, not controlling convergence

**Current understanding:**
- `blend_factor` controls convergence behavior
- Dynamic blend (exponential) prevents overbrighten naturally
- Per-pixel control was always fighting against global convergence
- **The real issue:** Different areas need different amounts of rendering time

### 3. Density vs Iteration Count Confusion

**Previous attempts conflated two concepts:**
- **Density** = brightness/color accumulation (visual result)
- **Iteration count** = how many times a pixel was hit (sampling metric)

**Key distinction:**
- Dense/bright pixels don't necessarily need more iterations
- Sparse pixels may need more iterations to reduce noise
- **Iteration count ≠ density**

---

## New Approach: Per-Pixel Iteration Stopping

### Concept

**Goal:** Stop accumulating individual pixels once they've received enough **iteration hits**, regardless of their brightness.

**Key difference from previous attempts:**
- NOT modifying encoding/decoding (no scale changes)
- NOT using density as proxy for "doneness"
- TRACKING actual iteration count per pixel
- SETTING blend_factor=0 for converged pixels

### Architecture

**New Buffer: `iteration_count_buffer`**
```rust
// Storage buffer (read-write)
size: width × height × u32  // 4 bytes per pixel
initial: 0
```

**New Parameter: `target_iterations_per_pixel`**
```rust
type: u32
default: 10000
ui_range: 1000 to 1000000
```

**Compute Shader (trajectory.wgsl):**
```wgsl
// After writing to histogram
atomicAdd(&iteration_count_buffer[pixel_index], 1u);
```

**Accumulate Shader (accumulate.wgsl):**
```wgsl
// Check if pixel is converged
let pixel_iterations = iteration_count_buffer[pixel_index];
let is_converged = pixel_iterations >= params.target_iterations_per_pixel;

// Gate blend_factor
let final_blend = select(blend_factor, 0.0, is_converged);

// Use gated blend
rgb_accumulated = prev.rgb * (1.0 - final_blend) + new_color * final_blend;
```

### Why This Should Work

**1. No encoding changes**
- All data encoded/decoded with same scale
- No timing mismatches between frames
- No cumulative errors

**2. Works with u32 histogram**
- Overflow is not a concern
- Can track millions of iterations without wrapping

**3. Independent of density**
- Sparse bright pixels can converge (high density, low iterations needed)
- Dense dark pixels can converge (low density, many iterations accumulated)
- Tracks actual sampling, not visual result

**4. Compatible with blend_factor**
- Works with both dynamic blend (exponential convergence as fallback)
- Works with fixed blend (for testing density compression)
- Per-pixel gate multiplies with global blend_factor

**5. Simple semantics**
- "This pixel has been hit N times, it's done"
- Clear, measurable, predictable

---

## Potential Issues and Solutions

### Issue 1: Visual Discontinuities

**Problem:** Abruptly setting blend=0 might create visible boundaries between converged/unconverged regions.

**Solution:** Soft transition
```wgsl
let convergence_progress = saturate(f32(pixel_iterations) / f32(target));
let final_blend = blend_factor * (1.0 - convergence_progress);
```

This gradually reduces blend from 100% to 0% as pixel approaches target.

### Issue 2: Interaction with Dynamic Blend

**Problem:** If global blend_factor is already tiny (late in convergence), per-pixel gating is redundant.

**Analysis:** This is actually FINE. Two convergence mechanisms:
- Global: All pixels converge together (exponential)
- Per-pixel: Dense areas converge first

The faster mechanism wins. No conflict.

### Issue 3: When to Reset?

**Problem:** Should iteration counts persist across view changes?

**Solution:** Clear on reset, like histogram:
- View/zoom/pan change → clear counts
- Flame change → clear counts
- User reset → clear counts

### Issue 4: Memory Cost

**4 bytes × width × height**
- 800×600: 1.9 MB
- 1920×1080: 8.3 MB

**Acceptable** - similar to other buffers.

---

## Comparison to Previous Approaches

| Aspect | Adaptive Scaling | Convergence Masking | Per-Pixel Iteration Stop |
|--------|-----------------|---------------------|-------------------------|
| **Modifies encoding** | ✅ Yes (per-pixel scales) | ❌ No | ❌ No |
| **Timing issues** | ✅ Yes (scale changes) | ❌ No | ❌ No |
| **Overflow concern** | ✅ Yes (u16 histogram) | ✅ Yes (u16 histogram) | ❌ No (u32 histogram) |
| **Tracks iterations** | ❌ No (density proxy) | ❌ No (density threshold) | ✅ Yes (actual count) |
| **Blend control** | ❌ Complex interaction | ⚠️ Binary gate | ✅ Smooth gate |
| **Semantics** | Unclear (scale?) | Unclear (density?) | Clear (iteration count) |
| **Compatibility** | ❌ Batching issues | ⚠️ Caused black patches | ✅ Should work |

---

## Implementation Plan

### Phase 1: Infrastructure
1. Add `iteration_count_buffer` to FlameBuffers
2. Initialize to 0
3. Add to compute bind group layout (binding 7)
4. Add to accumulate bind group layout

### Phase 2: Counting
1. Add atomic increment to trajectory shaders (2D & 3D)
2. Verify counts are incrementing correctly

### Phase 3: Gating
1. Add `target_iterations_per_pixel` parameter to AccumulateParams
2. Read iteration count in accumulate.wgsl
3. Gate blend_factor based on threshold
4. Test with hard gate (on/off) first

### Phase 4: Polish
1. Implement soft transition (gradual fade)
2. Add UI control (slider for target iterations)
3. Add reset behavior
4. Test with various fractals

### Phase 5: Integration with Density Compression
1. Test how iteration gating + density compression interact
2. Verify no conflicts
3. Document combined behavior

---

## Key Takeaways

**Why previous approaches failed:**
1. Adaptive scaling: Modified encoding dynamically (timing mismatch with batching)
2. Convergence masking: Used density as proxy (overflow still happened with u16)

**Why this should work:**
1. U32 histogram eliminates overflow concern
2. Tracks actual iterations, not density
3. Doesn't modify encoding/decoding
4. Simple gate on blend_factor
5. Compatible with all existing systems

**The critical insight:**
> Per-pixel systems that **gate accumulation** (this approach) are compatible with batching.
> Per-pixel systems that **modify encoding** (adaptive scaling) are NOT compatible with batching.

**blend_factor's role:**
> It's not the enemy—it's the control mechanism. Per-pixel iteration stopping works WITH blend_factor,
> not against it.
