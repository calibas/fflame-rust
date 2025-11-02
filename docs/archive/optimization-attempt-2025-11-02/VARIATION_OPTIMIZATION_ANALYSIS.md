# Variation Performance Optimization Analysis

## ❌ OPTIMIZATION REVERTED (2025-11-02)

The Apophysis-style precalculation optimization was implemented, tested, and **REVERTED** due to zero performance improvement.

**What Happened:**
1. ✅ Implemented precalculation block in `apply_variations()` for both 2D and 3D shaders
2. ✅ Updated variation function signatures to accept precalculated values
3. ✅ Updated shader builder to generate correct function calls
4. ❌ **Benchmark showed 0% improvement, actually ~1% slower**
5. ✅ Reverted changes after discovering root cause

**Benchmark Results:**
- Before optimization: 1701.80ms ± 14.50ms (commit 711c947)
- After optimization:  1718.09ms ± 12.52ms (commit b9a3f35)
- **Difference: +16.29ms slower (+0.96%)**

**Root Cause:**
Modern GPU shader compilers (SPIR-V, DXC, Metal) already perform **Common Subexpression Elimination (CSE)** automatically. The compiler was already deduplicating redundant `atan2()` and `length()` calls without our manual intervention.

**Why It Made Things Worse:**
1. Forced calculations even when unused → register pressure
2. Added function call overhead instead of inline code
3. Prevented compiler from doing platform-specific optimizations

**Conclusion:**
What worked for Apophysis CPU rendering in 2005 doesn't apply to modern GPU shader compilers in 2025. **Trust the compiler.**

See full analysis in [SHADER_COMPILER_CSE_ANALYSIS.md](SHADER_COMPILER_CSE_ANALYSIS.md)

---

## Original Analysis (Historical)

### Current Implementation (Before Optimization)

**Approach:** Each variation independently calculates what it needs
```wgsl
fn variation_polar(p: vec2<f32>) -> vec2<f32> {
    let theta = atan2(p.x, p.y);  // Expensive
    let r = length(p);             // sqrt() internally
    return vec2<f32>(theta / PI, r - 1.0);
}

fn variation_disc(p: vec2<f32>) -> vec2<f32> {
    let theta = atan2(p.x, p.y);  // Duplicate calculation!
    let r = length(p);             // Duplicate calculation!
    // ...
}
```

**Problem:** Redundant expensive calculations across multiple variations

**Measured Cost (from shader analysis):**
- 23× `atan2()` calls (one per variation using angles)
- 29× `sin()` calls
- 12× `cos()` calls
- 4× `sqrt()` calls (via `length()`)
- Total: ~68 expensive operations per iteration

## Apophysis Optimization

**Approach:** Precalculate common values once
```pascal
// XForm.pas:351-361 - Precalculation step
FLength := sqrt(FTx^ * FTx^ + FTy^ * FTy^);  // r
FAngle := arctan2(FTx^, FTy^);               // θ
SinCos(FAngle, FSinA, FCosA);                // sin(θ), cos(θ) together

// Then variations use precalculated values:
FPx := FPx + vars[10] * FSinA * cos(FLength);  // Uses FSinA instead of recalculating
```

**Benefits:**
1. **`atan2`**: Called once (saves ~22 redundant calls)
2. **`sin`/`cos`**: Called once together via `SinCos` hardware instruction
3. **`sqrt`**: Called once for radius
4. **Memory locality**: Precalculated values in registers

**Potential Speedup:**
- Worst case: All variations active → ~95% reduction in trig calls
- Typical case: 2-3 variations active → ~50-70% reduction
- Best case: Single variation → No benefit

## Proposed Optimization

### Option 1: Apophysis-Style Precalculation

**Implementation:**
```wgsl
fn apply_variations(xform: Transform, xform_id: u32, p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    // Precalculate common values
    let r = length(p);
    let r2 = dot(p, p);
    let theta = atan2(p.x, p.y);
    let sin_theta = sin(theta);
    let cos_theta = cos(theta);

    var result = vec2<f32>(0.0, 0.0);

    // Pass precalculated values to variations
    if (xform.variations[5] > 0.0) {  // Polar
        result += xform.variations[5] * variation_polar_opt(theta, r);
    }
    // ... etc

    return result;
}

fn variation_polar_opt(theta: f32, r: f32) -> vec2<f32> {
    return vec2<f32>(theta / PI, r - 1.0);
}
```

**Pros:**
- Massive reduction in trig calls (23 → 1 for atan2, 29+12 → 2 for sin/cos)
- Follows proven Apophysis approach
- GPU-friendly (values in registers)

**Cons:**
- Always calculates even if not all values needed
- Requires refactoring all variations
- Less modular code

### Option 2: Lazy Precalculation

**Implementation:**
```wgsl
struct PrecalcCache {
    p: vec2<f32>,
    r: f32,
    theta: f32,
    sin_theta: f32,
    cos_theta: f32,
    r_valid: bool,
    theta_valid: bool,
    trig_valid: bool,
}

fn get_r(cache: ptr<function, PrecalcCache>) -> f32 {
    if (!(*cache).r_valid) {
        (*cache).r = length((*cache).p);
        (*cache).r_valid = true;
    }
    return (*cache).r;
}

fn get_theta(cache: ptr<function, PrecalcCache>) -> f32 {
    if (!(*cache).theta_valid) {
        (*cache).theta = atan2((*cache).p.x, (*cache).p.y);
        (*cache).theta_valid = true;
    }
    return (*cache).theta;
}
```

**Pros:**
- Only calculates what's actually used
- More flexible for sparse variation usage

**Cons:**
- Branching overhead on GPU
- More complex implementation
- Pointer indirection costs

### Option 3: Hybrid - Precalc Only Common Values

**Implementation:**
```wgsl
fn apply_variations(...) {
    // Only precalculate if multiple angle-based variations are active
    let needs_angle = (xform.variations[5] > 0.0)  // polar
                   || (xform.variations[6] > 0.0)  // handkerchief
                   || (xform.variations[7] > 0.0)  // heart
                   // ... etc
                   ;

    let theta = select(0.0, atan2(p.x, p.y), needs_angle);
    let r = select(0.0, length(p), needs_angle);

    // Pass to variations that need it
}
```

**Pros:**
- Conditional calculation based on active variations
- Compiler can optimize at shader build time

**Cons:**
- Still have branching
- More complex logic

## Recommendation

**Best Approach: Option 1 (Eager Precalculation)**

**Rationale:**
1. **GPU architecture favors uniform execution** - all threads do same work
2. **Cost is bounded** - always exactly 1 atan2, 1 sin, 1 cos, 1 sqrt
3. **Typical flames use 2-5 variations** - almost always saves work
4. **Proven in Apophysis** - battle-tested approach
5. **Simple implementation** - no complex branching logic

**Expected Performance Impact:**
- **Minimal overhead** when 1 variation active (~4 extra ops)
- **Significant gain** when 2+ variations active (~50-95% reduction)
- **Memory impact** - minimal (4-5 extra f32 registers per thread)

## Implementation Plan

1. Modify `build_apply_variations_2d()` in shader builder
2. Add precalculation block at start of function
3. Update all variation function signatures to accept precalculated values
4. Keep original variation functions for reference/testing
5. Benchmark before/after with typical flames

## Additional Optimizations

### 1. Use `sincos()` Intrinsic (if available)
Some GPU APIs provide `sincos()` that computes both simultaneously:
```wgsl
// If supported:
let sin_cos = sincos(theta);  // Returns vec2(sin, cos) faster than separate calls
```

### 2. Constant Folding
```wgsl
// Instead of:
let theta_pi = theta / 3.14159265359;

// Use:
const INV_PI: f32 = 0.318309886184;  // 1/π precomputed
let theta_pi = theta * INV_PI;
```

### 3. Fast Math Approximations (Optional)
For non-critical variations, use polynomial approximations:
```wgsl
// Fast sin approximation (bhaskara I, ~2% error)
fn sin_fast(x: f32) -> f32 {
    let x_norm = x - floor(x / TAU) * TAU;  // Normalize to [0, 2π]
    // ... polynomial approximation
}
```
**Trade-off:** Speed vs accuracy (probably not worth it for fractals)

## Performance Metrics to Track

1. **Frame time** before/after optimization
2. **Iteration throughput** (iterations/second)
3. **GPU occupancy** (warp/wavefront utilization)
4. **Memory bandwidth** usage

## References

- Apophysis source: XForm.pas:351-361 (Prepare method)
- GPU optimization guides: Prefer uniform execution over branching
- WGSL spec: Builtin functions and their relative costs
