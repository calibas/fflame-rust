# Iterations Per Thread Quality Issue

## Problem Statement

When `iterations_per_thread` is increased, the visual quality of the fractal rendering degrades. The tone mapping doesn't appear as smooth when rendering happens too quickly.

**User Report**: "When I increase this, the quality isn't the same anymore. It's like the tone mapping isn't as smooth when it renders too quickly."

## Observations

- Higher `iterations_per_thread` → faster rendering but degraded quality
- Lower `iterations_per_thread` → slower rendering but better quality
- The issue appears to be related to tone mapping smoothness
- Quality degradation is independent of total iteration count (same final iteration count shows different quality at different per-thread settings)

## Current Architecture

### 3-Pass Rendering Pipeline

1. **Compute Pass** ([shaders/core/main_2d.wgsl](../shaders/core/main_2d.wgsl))
   - Each thread runs `iterations_per_thread` iterations (line 19)
   - Each hit writes `vec4(color, 0.01)` to temp texture (line 71)
   - Alpha channel = 0.01 per hit (density accumulation)

2. **Accumulate Pass** ([shaders/accumulate.wgsl](../shaders/accumulate.wgsl))
   - RGB: Weighted blend using `blend_factor = samples_this_frame / samples_accumulated` (line 33)
   - Alpha: **Additive** accumulation `prev.a + new_sample.a` (line 37)
   - Ping-pong buffers swapped each frame

3. **Tonemap Pass** ([shaders/tonemap.wgsl](../shaders/tonemap.wgsl))
   - Applies `sqrt(density * density_scale)` normalization (line 53)
   - Logarithmic tone mapping (line 68)
   - Gamma correction (line 72)

### Sample Count Calculation

From [compute_kernel.rs:140-144](../src/renderer/compute_kernel.rs#L140-L144):

```rust
// Track total iterations: workgroups * threads_per_workgroup * iterations_per_thread
let threads_per_workgroup = 64u64;
let samples_this_frame = num_workgroups as u64 * threads_per_workgroup * iterations_per_thread as u64;
```

**Examples:**
- `iterations_per_thread = 64`: 128 × 64 × 64 = 524,288 samples/frame @ 60 FPS
- `iterations_per_thread = 256`: 128 × 64 × 256 = 2,097,152 samples/frame @ 60 FPS
- `iterations_per_thread = 1024`: 128 × 64 × 1024 = 8,388,608 samples/frame @ 60 FPS

## Asymmetry in Accumulation

### RGB Accumulation (Weighted)
```wgsl
let rgb_accumulated = prev.rgb * (1.0 - params.blend_factor) + new_sample.rgb * params.blend_factor;
```
- Properly weighted by sample count
- Independent of `iterations_per_thread`
- Converges smoothly

### Density Accumulation (Additive)
```wgsl
let alpha_accumulated = prev.a + new_sample.a;
```
- **Not weighted** - just adds density
- Density grows faster when `iterations_per_thread` is higher
- More hits per frame → larger `new_sample.a` → faster density growth

## Theory: Why Quality Degrades

### Hypothesis 1: sqrt() Compression Artifacts
The tonemap shader applies `sqrt(density)` to compress high-density areas:

```wgsl
let normalized_density = sqrt(density * tonemap_params.density_scale);
```

**Effect of iterations_per_thread:**
- **Low setting**: Density accumulates slowly → sqrt() operates on smaller increments → smooth brightness curve
- **High setting**: Density accumulates rapidly → sqrt() operates on larger jumps → visible brightness stepping

**Math Example** (assuming same pixel gets hit):
- Frame 1: `density = 1.0`, `sqrt(1.0) = 1.0`
- Frame 2 (low): `density = 1.5`, `sqrt(1.5) = 1.22` (Δ = 0.22)
- Frame 2 (high): `density = 4.0`, `sqrt(4.0) = 2.0` (Δ = 1.0) ← larger visible step

### Hypothesis 2: Temporal Aliasing
Higher `iterations_per_thread` means fewer frames to reach same total iteration count:
- **Low setting**: More frames → finer temporal resolution → smoother convergence
- **High setting**: Fewer frames → coarse temporal resolution → visible "chunking"

### Hypothesis 3: Uneven Sample Distribution
Within a single frame, the RNG may not distribute samples evenly across all pixels:
- With more iterations per thread, some pixels get many hits while others get few
- This creates uneven density growth patterns
- The sqrt() compression then amplifies these uneven patterns differently at different scales

## Attempted Solutions

### ❌ Attempt 1: Weighted Density Accumulation
**Change**: Modified `alpha_accumulated` to use same weighted blend as RGB
```wgsl
let alpha_accumulated = prev.a * (1.0 - params.blend_factor) + new_sample.a * params.blend_factor;
```

**Result**: **FAILED** - Created dark patches inside fractals, lowered quality across all settings

**Why it failed**:
- RGB blending works because each new sample is a complete color value
- Density needs to accumulate the total hit count, not average it
- Averaging density loses information about how many samples actually hit each pixel
- The weighted blend essentially "forgets" previous samples, causing under-exposure

### ❌ Attempt 2: Normalized Per-Hit Density (Option B)
**Change**: Scale the per-hit density by `1/iterations_per_thread` in compute shaders
```wgsl
// main_2d.wgsl and main_3d.wgsl
let density_per_hit = 0.01 / f32(params.iterations_per_thread);
textureStore(output_texture, pixel, vec4<f32>(final_color, density_per_hit));
```

**Result**: **FAILED** - Made quality worse at all iterations_per_thread levels

**Why it failed**:
- Drastically reduces the density values written per hit
- At high iterations_per_thread (e.g., 1024), writes only 0.0000097656 per hit
- This makes the accumulated density grow extremely slowly
- The tonemap shader's sqrt() and density_scale can't compensate enough
- Results in under-exposed, dim rendering regardless of settings
- **Key insight**: The problem is likely not about *normalization* but about temporal aliasing or sqrt() compression behavior

## Potential Solutions (Not Yet Tested)

### Option A: Remove sqrt() Compression
**Pros:**
- Simplest change
- Makes density-to-brightness relationship linear
- Should eliminate the sqrt() stepping artifacts

**Cons:**
- May cause over-brightness in high-density areas
- Changes the aesthetic of existing fractals
- Backward compatibility concerns

**Implementation:**
```wgsl
// tonemap.wgsl line 53
let normalized_density = density * tonemap_params.density_scale;  // Linear instead of sqrt
```

### Option B: Scale Per-Hit Density by 1/iterations_per_thread
**Pros:**
- Normalizes density accumulation rate regardless of iterations_per_thread
- Maintains backward compatibility (just need to adjust density_scale once)

**Cons:**
- Requires passing iterations_per_thread to compute shader (already there!)
- Need to scale the 0.01 constant dynamically
- May affect existing presets

**Implementation:**
```wgsl
// main_2d.wgsl line 71
let density_per_hit = 0.01 / f32(params.iterations_per_thread);
textureStore(output_texture, pixel, vec4<f32>(final_color, density_per_hit));
```

### Option C: Adaptive sqrt() Power
**Pros:**
- Could provide best of both worlds
- Allows user control via parameter

**Cons:**
- More complex
- Requires new parameter
- May not fully solve the issue

**Implementation:**
```wgsl
// Add to TonemapParams
compression_power: f32,  // 0.5 = sqrt, 1.0 = linear, 0.3 = more compression

// tonemap.wgsl
let normalized_density = pow(density * tonemap_params.density_scale, tonemap_params.compression_power);
```

### Option D: Frame-Rate Normalization
**Pros:**
- Targets the temporal aliasing hypothesis
- Could provide smoother convergence

**Cons:**
- Complex to implement
- May not address the core issue
- Requires tracking frame timing

**Implementation:**
- Calculate expected density per second
- Normalize density accumulation by actual frame time
- Adjust blend_factor based on temporal frequency

## Analysis: Why Both Attempts Failed

Both attempts tried to make density accumulation "independent" of `iterations_per_thread`, but this fundamentally misunderstands the problem:

1. **Density IS the sample count** - It's supposed to grow with more samples
2. **RGB blending compensates** - The weighted RGB blend already handles different sample rates
3. **The real issue is temporal** - It's about **when** density updates happen, not **how much** accumulates

The problem is likely **Hypothesis 2: Temporal Aliasing**:
- Low `iterations_per_thread`: Many small density updates → smooth progressive refinement
- High `iterations_per_thread`: Few large density updates → visible "chunking" or "stepping"

The sqrt() compression makes these temporal jumps more visible because:
- sqrt() has diminishing returns (derivative decreases as x increases)
- Large density jumps get compressed differently than many small jumps
- Visual perception sees the discontinuity

## Potential Solution: Total Iterations-Based Tone Mapping

### Hypothesis: Normalize by Total Sample Count
Instead of using raw density, normalize tone mapping by the total iterations rendered:

```wgsl
// tonemap.wgsl
let normalized_density = sqrt(density / total_iterations) * tonemap_params.density_scale;
```

**Rationale:**
- Density grows linearly with iterations: `density ≈ 0.01 * hit_count`
- Total iterations is known and constant once rendering completes
- Dividing by total iterations normalizes density to range [0, 1] regardless of iteration count
- This makes density represent "hit probability" rather than "absolute hit count"

**Benefits:**
- Tone mapping would be deterministic for a given total iteration count
- Quality should be consistent regardless of `iterations_per_thread` setting
- The temporal chunking would still exist during progressive rendering, but final result would be identical

**Challenges:**
- Requires passing `total_iterations` to tonemap shader (add to TonemapParams)
- Only helps for final output, not during progressive refinement
- May need to update during rendering (requires dynamic buffer updates)
- Might still show artifacts during convergence, only final frame would match

**Implementation:**
1. Add `total_iterations: u32` to TonemapParams struct
2. Update tonemap shader to use `density / f32(total_iterations)`
3. Update renderer to pass current total_iterations each frame
4. Test at different `iterations_per_thread` values with same total

### Alternative: Iteration-Scaled Density Target
Instead of normalizing in tonemap, set a target density based on total iterations:

```rust
// In renderer
let target_density = (total_iterations as f32 / 1_000_000.0) * base_density_scale;
```

This allows auto-adjustment of density_scale based on iteration count, making the exposure automatic.

## Next Steps

1. **CLI parameter added** - Can now test different iterations_per_thread values via `--iterations-per-thread`
2. **Test total iterations normalization** - Implement Option: Total Iterations-Based Tone Mapping
3. **Quantify difference** - Export at 64, 256, 1024 iterations_per_thread and compare PSNRs
4. **Accept temporal artifacts** - May be unavoidable during progressive rendering
5. **Document workaround** - Use lower iterations_per_thread for quality, higher for speed

## References

- [compute_kernel.rs:140-144](../src/renderer/compute_kernel.rs#L140-L144) - Sample count calculation
- [accumulate.wgsl:33-37](../shaders/accumulate.wgsl#L33-L37) - RGB vs density accumulation asymmetry
- [tonemap.wgsl:53](../shaders/tonemap.wgsl#L53) - sqrt() compression
- [main_2d.wgsl:71](../shaders/core/main_2d.wgsl#L71) - Per-hit density write (0.01)
