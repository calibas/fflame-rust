# Shader Performance Analysis

**Date**: 2025-12-27
**Context**: Investigating 18% performance regression after Shader Builder v3 implementation

## Background

After implementing Shader Builder v3 (dynamic shader generation with only active variations), we observed an 18% performance regression on `misc-variations.fflame` and `misc-variations2.fflame` test configs. Investigation revealed two issues:

1. **Double shader builds**: ShaderCache was building both 2D and 3D shaders regardless of which mode was needed
2. **Potential shader execution inefficiencies**: The generated shader has buffer access patterns worth investigating

## Fixed Issues

### Double Shader Build (Fixed in commit 935511b)

**Problem**: `ShaderCache::new()` and `ensure_current_full()` always built both 2D and 3D shaders, even when only one mode was needed.

**Solution**:
- Added `current_render_mode` tracking to `ShaderCache`
- Only build the shader for the active render mode
- Clone the pipeline to the unused slot (valid but unused)

**Impact**: ~27% performance improvement for short renders (under 500ms), smaller gains for longer renders as expected (shader compilation is one-time cost).

### CLI Shader Debugging

Added `--dump-shader` flag to export command:
```bash
fractal_flame_wgpu.exe export -i config.fflame -o output.png --dump-shader
```
Writes generated shader to `debug_shader_2d.wgsl` or `debug_shader_3d.wgsl`.

## Remaining Performance Concerns

Analysis of `debug_shader_2d.wgsl` (898 lines) reveals potential hotspots:

### 1. Transform Struct Size (~420 bytes per transform)

```wgsl
struct Transform {
    a: f32, b: f32, c: f32, d: f32, e: f32, f: f32,  // 24 bytes (affine)
    g: f32,                                           // 4 bytes (Z offset)
    weight: f32,                                      // 4 bytes
    variations: array<f32, 100>,                      // 400 bytes (!!)
    color: f32, color_speed: f32, opacity: f32, _padding: f32,  // 16 bytes
}
```

**Issue**: Every `let xform = transforms[xform_idx]` loads ~420 bytes, but we typically only use:
- 6 affine coefficients
- weight
- ~5-27 variation weights (out of 100 slots)
- 3 color fields

**Potential fix**: Split into multiple buffers:
- `TransformAffine` (24 bytes): a,b,c,d,e,f
- `TransformMeta` (16 bytes): weight, color, color_speed, opacity
- `TransformVariations` (sparse): only active variation weights

### 2. Variation Parameters Buffer (4800 bytes per transform)

```wgsl
struct VariationParams {
    params: array<f32, 1200>,  // 100 variations × 12 params
}
```

**Issue**: `get_param()` reads from a 4800-byte struct per transform. Most flames use <10 parameterized variations.

```wgsl
fn get_param(xform_id: u32, variation_id: u32, param_slot: u32) -> f32 {
    let idx = variation_id * 12u + param_slot;
    return variation_params[xform_id].params[idx];
}
```

**Potential fix**: Pack only active variation parameters into a smaller buffer.

### 3. Transform Selection Loop (per iteration)

```wgsl
fn select_transform_const(rand_val: f32) -> u32 {
    var total_weight = 0.0;
    for (var i = 0u; i < NUM_TRANSFORMS; i++) {
        total_weight += transforms[i].weight;  // Buffer read per transform
    }
    // ... second loop for selection
}
```

**Issue**: Runs every iteration, reads `weight` from each transform twice (total weight + selection).

**Potential fix**: Precompute cumulative weights on CPU, upload as separate buffer:
```wgsl
@group(0) @binding(N) var<storage, read> cumulative_weights: array<f32>;
```

### 4. Variation Weight Checks (27 branches in this shader)

```wgsl
// 0: Linear (NORMAL)
if (xform.variations[0] != 0.0) {
    result += xform.variations[0] * variation_linear(temp);
}
// ... repeated 26 more times
```

**Issue**: 27 conditional branches per iteration. GPU branch divergence can hurt performance.

**Potential fix**:
- Generate switch statement instead of if-chain
- Or use indirect dispatch with only active variations

### 5. Buffer Binding Count

Current bindings (9 total):
```wgsl
@group(0) @binding(0) var<storage, read> transforms: array<Transform>;
@group(0) @binding(1) var<uniform> params: Params;
@group(0) @binding(2) var<storage, read_write> histogram: array<atomic<u32>>;
@group(0) @binding(3) var palette_texture: texture_2d<f32>;
@group(0) @binding(4) var palette_sampler: sampler;
@group(0) @binding(5) var<storage, read> variation_params: array<VariationParams>;
@group(0) @binding(6) var<storage, read_write> iteration_counts: array<atomic<u32>>;
@group(0) @binding(7) var<storage, read_write> path_buffer: array<PathEntry>;
@group(0) @binding(8) var<storage, read> path_filters: array<PathFilter>;
```

Not excessive, but `path_buffer` and `path_filters` are unused in non-PathMap mode (wasted bindings).

## Compile-Time Constants Opportunity

The shader already uses some hard-coded constants:
```wgsl
const NUM_TRANSFORMS: u32 = 2u;
const COLOR_MODE: u32 = 0u;
const HAS_FINAL_TRANSFORM: bool = false;
const FINAL_TRANSFORM_INDEX: u32 = 2u;
```

**All values that stay constant for an entire render could be compiled into the shader** instead of being read from buffers. This would:
1. Eliminate buffer reads entirely
2. Enable dead code elimination (e.g., if variation weight is 0.0, that branch is removed)
3. Enable constant folding and additional compiler optimizations

### Values that could become shader constants:

| Value | Current Source | Reads Per Iteration | Benefit |
|-------|---------------|---------------------|---------|
| Transform weights | `transforms[i].weight` | 2× (total + select) | Eliminates buffer reads in hot loop |
| Affine coefficients | `transforms[idx].a/b/c/d/e/f` | 6 per transform | Eliminates 420-byte struct load |
| Variation weights | `xform.variations[N]` | 27 checks | Dead code elimination if weight=0 |
| Variation parameters | `get_param()` | Varies | Eliminates buffer indirection |
| Color/opacity | `xform.color/color_speed/opacity` | 3 per iteration | Constant propagation |
| Cumulative weights | Computed | 2 loops | Single comparison possible |

### Example: Fully Inlined Shader

Instead of:
```wgsl
let xform = transforms[xform_idx];  // 420-byte buffer read
if (xform.variations[0] != 0.0) {   // Another read
    result += xform.variations[0] * variation_linear(temp);
}
```

Could generate:
```wgsl
// Transform 0: Linear=0.5, Spherical=0.3 (all others are 0.0 - eliminated!)
if (xform_idx == 0u) {
    let affine_p = vec2<f32>(
        0.7 * p.x + 0.1 * p.y + 0.0,  // a,b,e inlined
        0.2 * p.x + 0.7 * p.y + 0.0   // c,d,f inlined
    );
    result = 0.5 * variation_linear(affine_p) + 0.3 * variation_spherical(affine_p);
}
```

**Trade-off**: Shader recompilation required when flame parameters change. Currently the shader cache rebuilds when variation SET changes (variations added/removed). Full inlining would require rebuild on ANY parameter change (weight, affine, color).

**Hybrid approach**: Inline only the variation weights (enables dead code elimination for unused variations) while keeping affine/color in buffers (change frequently during editing).

## Recommendations (Priority Order)

### High Priority
1. **Inline variation weights as constants** - Enables dead code elimination for unused variations
   - When a variation weight is 0.0, the entire if-block can be compiled out
   - Currently 27 branches checked; could be reduced to just the 2-5 active ones
   - Rebuild on variation weight change (already rebuilding on variation add/remove)

2. **Precompute cumulative weights as constants** - Eliminates both loops in transform selection
   - Generate: `const CUMULATIVE_WEIGHTS: array<f32, N> = array(...);`
   - Single pass comparison instead of two loops

3. **Inline affine coefficients** - Eliminates 420-byte struct load per iteration
   - Generate per-transform constants for a,b,c,d,e,f
   - Trade-off: Rebuild shader when affine parameters change

### Medium Priority
4. **Pack active variation params as constants** - Eliminates buffer indirection
   - Only include params for variations actually used
   - `const BLOB_HIGH: f32 = 1.0;` instead of `get_param(xform_id, 25u, 0u)`

5. **Remove unused bindings** - Don't bind path_buffer/path_filters in simple mode
   - Already using conditional shader generation, can extend to bindings

### Low Priority (Complex, Marginal Gains)
6. **Sparse variation weights** - Use index+weight pairs instead of 100-slot array
7. **Indirect dispatch** - Separate kernel per variation (extreme complexity)

### Trade-off Analysis

| Approach | Rebuild Trigger | Performance Gain | Implementation |
|----------|----------------|------------------|----------------|
| Current | Variation set changes | Baseline | ✅ Done |
| + Variation weights | Weight changes | High (dead code elim) | Medium |
| + Affine coefficients | Any param change | High (no struct load) | Medium |
| + All constants | Any param change | Maximum | High |

**Recommended hybrid**: Inline variation weights (high gain, infrequent changes) but keep affine/color in buffers (change frequently during interactive editing). CLI export mode could use full inlining since there's only one render.

## Metrics to Track

When implementing optimizations, measure:
- Iterations per second (primary metric)
- Shader compilation time
- GPU memory usage
- Frame time stability

## Related Files

- `src/shader_cache.rs` - Shader caching and rebuild logic
- `src/shader_builder_v2.rs` - Dynamic shader generation
- `src/gpu/buffers.rs` - GPU buffer definitions
- `shaders/core/main_template.wgsl` - Main shader template
