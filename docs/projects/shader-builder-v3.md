# Shader Builder v3 - Performance & Code Organization

## Goals

1. **Performance**: Hard-code values into shaders to eliminate uniform buffer reads
2. **Feature Exclusion**: Completely exclude disabled features (path tracking, unused color modes)
3. **Modular Variations**: Separate variation files with hooks for 2D/3D/pre/post
4. **Reduce Duplication**: Single source files conditionally assembled (no 8 separate main variants)
5. **Animation Support**: Pre-declare variation superset to avoid rebuilds during animation

## Current State

### Files Involved
- `src/shader_cache.rs` (186 lines) - Caching layer
- `src/shader_builder_v2.rs` (714 lines) - Core assembly logic
- `src/gpu/pipelines.rs` (515 lines) - Pipeline creation
- `shaders/core/*.wgsl` - Shader components (8+ files)

### Current Pain Points
- 8 main shader variants (2D/3D × interactive/export/tiled/simple)
- `build_apply_variations_2d()` and `build_apply_variations_3d()` are 90% identical
- Z-only variations hardcoded by string matching instead of registry metadata
- All core variations compiled in even if unused (dead code)
- Values read from uniform buffers that rarely change

### What Triggers Rebuild Today
- Variation set changes (add/remove variation from any transform)
- Path features toggle
- NOT: weight changes, parameter changes, view changes

---

## Proposed Architecture

### Phase 1: Hard-Code Config Values

**Values to hard-code into shader source:**

| Value | Current | Proposed | Rebuild When |
|-------|---------|----------|--------------|
| `num_transforms` | Uniform buffer | `const NUM_TRANSFORMS: u32 = 3u;` | Transform add/remove |
| `render_mode` | Pipeline selection | Excluded code paths | Mode change |
| `color_mode` | Uniform buffer | Only include active mode code | Mode change |
| `has_final_transform` | Uniform buffer | `const HAS_FINAL: bool = false;` | Config change |
| `perspective_strength` | Uniform buffer | `const PERSPECTIVE: f32 = 2.5;` | Config change |
| `width/height` | Uniform buffer | Keep dynamic (resize) | - |
| `zoom/pan/rotation` | Uniform buffer | Keep dynamic (interactive) | - |

**Keep dynamic (uniform buffers):**
- `width`, `height` - window resize
- `zoom`, `pan_x`, `pan_y`, `rotation` - interactive navigation
- `camera_rotation_x/y`, `camera_z` - interactive camera
- `seed` - changes every frame
- `iterations_per_thread`, `burn_in` - may change during session

**Estimated performance gain:** 5-15% fewer buffer reads per iteration

### Phase 2: Feature Exclusion

**Path Tracking (biggest win):**
```
Current: main_2d.wgsl vs main_2d_simple.wgsl (separate files)
         + dummy 28-byte buffer when path tracking disabled
Proposed: Single main.wgsl with #ifdef-style conditional inclusion
          + dynamic bind group layouts (no dummy buffers)
```

When `color_mode != PathMap` AND no path filters:
- **Exclude path buffer binding from bind group layout entirely** (no dummy buffer)
- **Exclude filter buffer binding from bind group layout entirely**
- Exclude PathEntry struct from shader
- Exclude all path recording code from shader
- Exclude path filter logic from shader

**Dynamic Bind Group Layouts:**

Current architecture uses fixed layouts with dummy buffers:
```rust
// Current: Always 9 bindings in compute bind group
// Binding 7: path_buffer (real or 28-byte dummy)
// Binding 8: filter_buffer (real or 16-byte dummy)
```

Proposed architecture generates layouts based on features:
```rust
// Proposed: Bind group layout generated per-config
let entries = vec![
    // Bindings 0-6 always present
    binding_0_params,
    binding_1_histogram,
    // ...
];

if path_features_enabled {
    entries.push(binding_7_path_buffer);
    entries.push(binding_8_filter_buffer);
}

// Layout matches exactly what shader declares
```

**Benefits:**
- Eliminates dummy buffer allocation and helper methods
- Cleaner separation of concerns (no "fake" bindings)
- Slightly faster bind group validation
- Shader and pipeline always in sync

**Implementation Notes:**
- Requires separate pipeline for path-enabled vs path-disabled
- `FlameRenderer` tracks which pipeline variant is active
- Switching path mode triggers both shader rebuild AND pipeline rebuild
- Cache both pipeline variants to avoid rebuild on toggle

**Color Mode Exclusion:**
```wgsl
// Only include the active color mode's code:
// - Palette mode: palette sampling
// - Speed mode: velocity-based coloring
// - PathMap mode: path buffer + coloring
```

**Estimated performance gain:** 10-20% when path tracking disabled

### Phase 3: Modular Variation System

**New variation file structure:**
```
shaders/variations/
  linear.wgsl
  sinusoidal.wgsl
  spherical.wgsl
  julia.wgsl
  julian.wgsl
  ...
```

**Each variation file contains hooks:**
```wgsl
// shaders/variations/spherical.wgsl

// === VARIATION METADATA (parsed by builder) ===
// @name: spherical
// @category: Basic2D
// @phase: Normal
// @needs_rng: false
// @params: none

// === 2D IMPLEMENTATION ===
fn variation_spherical_2d(p: vec2<f32>) -> vec2<f32> {
    let r2 = dot(p, p) + 1e-6;
    return p / r2;
}

// === 3D IMPLEMENTATION ===
fn variation_spherical_3d(p: vec3<f32>) -> vec3<f32> {
    let r2 = dot(p.xy, p.xy) + 1e-6;
    return vec3(p.xy / r2, p.z);
}

// === PRE HOOK (optional) ===
// fn variation_spherical_pre(p: vec3<f32>) -> vec3<f32> { ... }

// === POST HOOK (optional) ===
// fn variation_spherical_post(p: vec3<f32>) -> vec3<f32> { ... }

// === INIT REQUIREMENTS (optional) ===
// Parsed by builder to include necessary setup code
```

**Z-only variations (zcone, zscale, flatten):**
```wgsl
// shaders/variations/zcone.wgsl

// @name: zcone
// @category: Depth3D
// @phase: Normal
// @z_only: true  // <-- New flag: only modifies Z component

fn variation_zcone_3d(p: vec3<f32>, weight: f32) -> vec3<f32> {
    return vec3(p.xy, p.z + weight * length(p.xy));
}

// No 2D implementation needed (z_only variations skip 2D)
```

**Builder generates `apply_variations()` by:**
1. Reading metadata from each active variation file
2. Including only the needed implementation (2D or 3D)
3. Generating weighted calls in correct phase order

### Phase 4: Single Main Template

**Replace 8 main variants with 1 template:**

```wgsl
// shaders/core/main_template.wgsl

// === GENERATED CONSTANTS ===
{{CONSTANTS}}
// const NUM_TRANSFORMS: u32 = 3u;
// const HAS_FINAL: bool = false;
// const RENDER_3D: bool = true;
// const COLOR_MODE: u32 = 0u;  // 0=Palette, 1=Speed, 2=PathMap
// const PATH_TRACKING: bool = false;

// === BINDINGS ===
{{BINDINGS}}
// Conditionally include path buffer binding

// === VARIATION FUNCTIONS ===
{{VARIATIONS}}
// Only active variations included

// === APPLY VARIATIONS ===
{{APPLY_VARIATIONS}}
// Generated function

// === MAIN LOGIC ===
@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    // ... iteration loop ...

    {{#if RENDER_3D}}
    var point = vec3<f32>(rng_next(&rng), rng_next(&rng), 0.0);
    {{else}}
    var point = vec2<f32>(rng_next(&rng), rng_next(&rng));
    {{/if}}

    // ... transform selection, affine, variations ...

    {{#if PATH_TRACKING}}
    // Path recording code
    {{/if}}

    {{#if COLOR_MODE == 0}}
    // Palette sampling
    {{else if COLOR_MODE == 1}}
    // Speed-based coloring
    {{else}}
    // PathMap coloring
    {{/if}}
}
```

**Template processing:**
- Simple string replacement for constants
- Conditional blocks with `{{#if CONDITION}}...{{/if}}`
- Section insertion with `{{SECTION_NAME}}`

### Phase 5: Animation Superset

**New API for pre-declaring variations:**

```rust
// Before starting animation
let variation_superset = vec![
    "linear", "sinusoidal", "spherical", "julia", "swirl"
];

renderer.prepare_for_animation(variation_superset);
// Compiles shader with all listed variations
// Config changes during animation won't trigger rebuilds
// as long as they stay within the superset
```

**Implementation:**
- `ShaderCache` stores `animation_superset: Option<HashSet<String>>`
- When set, uses superset for shader compilation instead of `flame.extract_active_variations()`
- `apply_variations()` still only calls variations with non-zero weights
- Unused variations are dead code but pre-compiled

**Use cases:**
- Keyframe animations interpolating between configs
- Random exploration (pre-load common variations)
- Batch export with varying configs

---

## Implementation Plan

### Step 1: Variation File Refactor
- [ ] Create `shaders/variations/` directory
- [ ] Extract each variation into its own file with metadata comments
- [ ] Update builder to parse variation files and extract metadata
- [ ] Update registry to load from variation files instead of hardcoded
- [ ] Verify all existing presets still work

### Step 2: Template System
- [ ] Create `main_template.wgsl` with conditional markers
- [ ] Implement simple template processor in Rust
- [ ] Replace `build_trajectory_2d/3d` with single `build_from_template()`
- [ ] Remove old `main_2d.wgsl`, `main_3d.wgsl`, etc.
- [ ] Verify 2D and 3D rendering still work

### Step 3: Hard-Code Constants ✅ COMPLETE
- [x] Add constants section to shader builder (`ShaderConstants` struct in `shader_builder_v2.rs`)
- [x] Generate constants from FractalConfig (`ShaderCache::constants_from_config()`)
- [x] Update shader main files to use constants (`NUM_TRANSFORMS`, `COLOR_MODE`, `HAS_FINAL_TRANSFORM`, `FINAL_TRANSFORM_INDEX`)
- [x] Add `select_transform_const()` function using hard-coded `NUM_TRANSFORMS` for loop unrolling
- [x] Update `ShaderCache::ensure_current_full()` to track and rebuild when constants change
- [ ] Remove corresponding fields from `GpuParams` struct (deferred - still needed for compatibility)
- [ ] Benchmark performance improvement (deferred to after full integration)

### Step 4: Feature Exclusion
- [ ] Add conditional blocks for path tracking in shader template
- [ ] Add conditional blocks for color modes in shader template
- [ ] Create dynamic bind group layout generation in `pipelines.rs`
- [ ] Remove dummy buffer creation from `buffers.rs`
- [ ] Remove `get_path_buffer_for_binding()` helper methods
- [ ] Update `FlameRenderer` to track active pipeline variant
- [ ] Cache both pipeline variants (path-enabled and path-disabled)
- [ ] Verify path mode still works when enabled
- [ ] Verify switching between path/non-path modes works
- [ ] Benchmark performance improvement

### Step 5: Animation Superset
- [ ] Add `prepare_for_animation()` API
- [ ] Store superset in ShaderCache
- [ ] Use superset in `ensure_current()` when set
- [ ] Add `end_animation()` to clear superset
- [ ] Test with animation system

### Step 6: Cleanup
- [ ] Remove dead code from old shader builder
- [ ] Update documentation
- [ ] Run full benchmark suite
- [ ] Visual regression tests

---

## Risk Assessment

| Risk | Mitigation |
|------|------------|
| Breaking existing presets | Visual regression tests before/after each step |
| WASM compatibility | Test on WASM after each step (uniform control flow) |
| Performance regression | Benchmark after each step, rollback if worse |
| Template complexity | Keep template syntax minimal (no full language) |
| Variation metadata drift | Single source of truth in variation files |

---

## Success Metrics

- [ ] **Performance**: 10-20% improvement in iterations/second
- [ ] **Code reduction**: 50%+ fewer lines in shader builder
- [ ] **Maintainability**: Adding new variation = 1 file, no builder changes
- [ ] **Animation**: Config changes during animation don't trigger rebuilds
- [ ] **All tests pass**: Visual regression, WASM, desktop

---

## Open Questions

1. **Template syntax**: Use handlebars-style `{{}}` or custom markers?
2. **Variation metadata**: WGSL comments vs separate JSON manifest?
3. **Rebuild granularity**: Rebuild all pipelines or just affected ones?
4. **Cache invalidation**: Hash-based or explicit versioning?

## Resolved Questions

1. **Dynamic bind group layouts vs dummy buffers?**
   - **Decision:** Dynamic bind group layouts
   - **Rationale:** Eliminates dummy buffer complexity, cleaner architecture
   - **Trade-off:** Requires pipeline caching for path-enabled/disabled variants
   - **Added to Phase 2**

---

## Timeline Estimate

Not providing time estimates per instructions. Steps are ordered by dependency and can be done incrementally with working code at each step.
