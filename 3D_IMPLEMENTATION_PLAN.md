# 3D Implementation Plan - Pseudo-3D Fractal Flames

## Overview
Upgrade the 2D fractal flame renderer to support pseudo-3D rendering using the "3D hack" from Apophysis 7X. This involves tracking a Z coordinate throughout the iteration process and projecting to 2D for display.

## Goals
- ✅ Maintain backward compatibility with existing 2D flames
- ✅ Support 3D variations (rotate_x/y, zscale, ztranslate, blur3D, etc.)
- ✅ Add projection modes (orthographic and perspective)
- ✅ Enable depth-based rendering effects (optional)
- ✅ Minimal performance impact

---

## Phase 1: Core Data Structure Changes (Day 1)

### 1.1 Update Transform Structure
**Files:** `src/scene/transforms.rs`, `src/gpu/buffers.rs`

- [ ] Add `RenderMode` enum (TwoD, ThreeD)
- [ ] Add `ProjectionType` enum (Orthographic, Perspective)
- [ ] Decide on affine representation:
  - **Option A:** Keep 2D affine (a,b,c,d,e,f) + separate Z parameters
  - **Option B:** Full 3x3 affine matrix + offset vector
  - **Recommendation:** Option A for backward compatibility
- [ ] Add Z offset field to Transform
- [ ] Update `GpuTransform` struct to match
- [ ] Update serialization (FractalConfig)

**New fields for Transform:**
```rust
pub z_offset: f32,           // Z translation component
pub preserve_z: bool,        // Whether to pass Z through variations unchanged
```

### 1.2 Update Flame/Config Structures
**Files:** `src/scene/transforms.rs`, `src/config.rs`

- [ ] Add `render_mode: RenderMode` to Flame
- [ ] Add projection settings to FractalConfig
- [ ] Update Default implementations
- [ ] Update existing presets to specify TwoD mode

### 1.3 Update GPU Buffers
**Files:** `src/gpu/buffers.rs`

- [ ] Add `render_mode: u32` to GpuParams (0=2D, 1=3D)
- [ ] Add `projection_type: u32` to GpuParams (0=Ortho, 1=Perspective)
- [ ] Add `perspective_strength: f32` to GpuParams
- [ ] Update buffer layouts and alignment
- [ ] Update `write_buffer` calls

---

## Phase 2: Shader Infrastructure (Day 1-2)

### 2.1 Create Shader Variants
**Files:** New shader files

**Option A: Single shader with conditionals**
- [ ] Keep existing `trajectory.wgsl`
- [ ] Add conditional compilation or runtime switches for 2D/3D

**Option B: Separate shaders (Recommended)**
- [ ] Rename current shader to `trajectory_2d.wgsl`
- [ ] Create new `trajectory_3d.wgsl`
- [ ] Create shared `variations_common.wgsl` (for included functions)
- [ ] Create `variations_3d.wgsl` (3D-specific variations)

### 2.2 Update Shader Structures
**Files:** `shaders/trajectory_3d.wgsl` (or unified shader)

- [ ] Update Transform struct to include z_offset
- [ ] Update Params struct for render_mode, projection_type, perspective_strength
- [ ] Change point representation from `vec2<f32>` to `vec3<f32>` (3D shader only)

### 2.3 Implement Projection Functions
**Files:** `shaders/trajectory_3d.wgsl`

```wgsl
// Orthographic projection (simple)
fn project_orthographic(p: vec3<f32>) -> vec2<f32> {
    return p.xy;
}

// Perspective projection
fn project_perspective(p: vec3<f32>, strength: f32) -> vec2<f32> {
    let scale = strength / (strength + p.z);
    return p.xy * scale;
}

// Dispatcher
fn project_to_2d(p: vec3<f32>) -> vec2<f32> {
    if params.projection_type == 0u {
        return project_orthographic(p);
    } else {
        return project_perspective(p, params.perspective_strength);
    }
}
```

- [ ] Implement projection functions
- [ ] Update `world_to_pixel()` to use projection
- [ ] Handle Z clipping (points too far/close)

### 2.4 Update Main Iteration Loop
**Files:** `shaders/trajectory_3d.wgsl`

- [ ] Change `current` from `vec2<f32>` to `vec3<f32>`
- [ ] Initialize Z coordinate (random_z in starting point)
- [ ] Update affine application for 3D
- [ ] Update variation application for 3D
- [ ] Add projection before pixel conversion

---

## Phase 3: 3D Variations Implementation (Day 2)

### 3.1 Update Existing 2D Variations
**Files:** `shaders/trajectory_3d.wgsl` or `variations_3d.wgsl`

All existing variations need to handle Z coordinate:
- [ ] **Pass-through approach:** Most 2D variations just pass Z unchanged
  ```wgsl
  fn variation_sinusoidal_3d(p: vec3<f32>) -> vec3<f32> {
      return vec3<f32>(sin(p.x), sin(p.y), p.z);
  }
  ```
- [ ] Update all 16 existing variations (linear, sinusoidal, spherical, etc.)

### 3.2 Implement Core 3D Variations
**Files:** `shaders/variations_3d.wgsl`

**Pre/Post Rotation:**
- [ ] `pre_rotate_x` - Rotate around X axis before variation
- [ ] `pre_rotate_y` - Rotate around Y axis before variation
- [ ] `pre_rotate_z` - Rotate around Z axis before variation
- [ ] `post_rotate_x` - Rotate around X axis after variation
- [ ] `post_rotate_y` - Rotate around Y axis after variation
- [ ] `post_rotate_z` - Rotate around Z axis after variation

**Z Manipulation:**
- [ ] `zscale` - Scale Z coordinate
- [ ] `ztranslate` - Translate Z coordinate
- [ ] `zcone` - Cone-shaped Z distortion
- [ ] `pre_zscale` - Scale Z before variation
- [ ] `pre_ztranslate` - Translate Z before variation

**3D Effects:**
- [ ] `flatten` - Compress Z toward plane
- [ ] `blur3D` - Random point on sphere
- [ ] `zblur` - Blur only in Z direction
- [ ] `hemisphere` - Project to hemisphere
- [ ] `curl3D` - 3D curl noise

### 3.3 Add Variation Parameters
**Problem:** Some 3D variations need parameters (rotation angles, scale factors)

**Solution Options:**
- **Option A:** Add parameter fields to Transform struct
- **Option B:** Encode in variation weights (e.g., `variations[16]` = angle)
- **Option C:** Add separate `variation_params: [f32; N]` array

**Recommendation:** Start with Option C
- [ ] Add `variation_params: [f32; 16]` to Transform
- [ ] Update GpuTransform
- [ ] Document which variations use params

---

## Phase 4: Pipeline and Renderer Updates (Day 2-3)

### 4.1 Pipeline Management
**Files:** `src/gpu/pipelines.rs`

- [ ] Add `trajectory_3d_pipeline` field to FlamePipelines
- [ ] Create shader module for 3D shader
- [ ] Create compute pipeline for 3D trajectory
- [ ] Add pipeline selection logic based on render_mode

### 4.2 Renderer Updates
**Files:** `src/renderer/compute_kernel.rs`

- [ ] Update `FlameRenderer::new()` to create both pipelines
- [ ] Update `compute_pass()` to select correct pipeline
- [ ] Add `current_render_mode` tracking for pipeline caching
- [ ] Update `load_config()` to handle render mode changes
- [ ] Update `update_flame()` to detect mode changes

### 4.3 Buffer Updates
**Files:** `src/renderer/compute_kernel.rs`, `src/gpu/buffers.rs`

- [ ] Update `write_params()` to include render_mode, projection_type, perspective_strength
- [ ] Update `update_transforms()` to include z_offset
- [ ] Ensure backward compatibility (2D transforms have z_offset=0)

---

## Phase 5: UI Controls (Day 3)

### 5.1 Mode Selection
**Files:** `src/ui/mod.rs`

- [ ] Add "Render Mode" radio buttons (2D / 3D)
- [ ] Show/hide 3D-specific controls based on mode
- [ ] Trigger accumulation reset on mode change

### 5.2 Projection Controls
**Files:** `src/ui/mod.rs`

- [ ] Add "Projection Type" dropdown (Orthographic / Perspective)
- [ ] Add "Perspective Strength" slider (only visible in Perspective mode)
- [ ] Add preview/tooltip explaining projection types

### 5.3 Transform Controls
**Files:** `src/ui/mod.rs`

- [ ] Add Z Offset slider to each transform (only in 3D mode)
- [ ] Add variation parameter controls (for rotations, scales)
- [ ] Consider collapsible "3D Parameters" section

### 5.4 View Controls
**Files:** `src/ui/mod.rs`

- [ ] Add camera controls (optional: rotate view in 3D)
- [ ] Add Z-axis visualization toggle
- [ ] Add depth color mapping toggle

---

## Phase 6: Testing and Presets (Day 3)

### 6.1 Create 3D Test Presets
**Files:** `src/scene/presets.rs`

- [ ] Simple 3D preset (linear + zscale)
- [ ] Rotation preset (pre_rotate_x + pre_rotate_y)
- [ ] Perspective preset (with perspective projection)
- [ ] 3D Julia set
- [ ] Hybrid 2D/3D preset

### 6.2 Validation
- [ ] Verify 2D mode unchanged (regression test)
- [ ] Test all existing 2D presets still work
- [ ] Test 3D variations render correctly
- [ ] Test projection switching
- [ ] Test serialization/deserialization

### 6.3 Documentation
**Files:** `CLAUDE.md`, `ARCHITECTURE.md`, `STATUS.md`

- [ ] Document 3D rendering mode
- [ ] Document projection types
- [ ] Document 3D variations
- [ ] Update architecture diagrams
- [ ] Add 3D variation examples

---

## Phase 7: Optional Enhancements

### 7.1 Depth-Based Effects
- [ ] Depth-based coloring (Z → color)
- [ ] Depth-based blur (further = blurrier)
- [ ] Depth-based opacity (Z-fog)
- [ ] Depth buffer visualization

### 7.2 Advanced 3D Features
- [ ] Camera rotation (orbit around fractal)
- [ ] Stereo rendering (side-by-side 3D)
- [ ] Anaglyph 3D (red/cyan glasses)
- [ ] Z-clipping planes

### 7.3 Performance Optimizations
- [ ] Benchmark 3D vs 2D performance
- [ ] Optimize projection calculations
- [ ] Early Z-clipping in shader

---

## Implementation Strategy

### Recommended Order

**Week 1: Core Implementation**
1. ✅ **Day 1 Morning:** Phase 1 - Data structures (Transform, Config, Buffers)
2. ✅ **Day 1 Afternoon:** Phase 2.1-2.3 - Shader infrastructure and projection
3. ✅ **Day 2 Morning:** Phase 3.1-3.2 - Convert 2D variations, add core 3D variations
4. ✅ **Day 2 Afternoon:** Phase 4 - Pipeline and renderer updates
5. ✅ **Day 3 Morning:** Phase 5 - UI controls
6. ✅ **Day 3 Afternoon:** Phase 6 - Testing and presets

**Week 2: Polish (Optional)**
7. Phase 7 - Optional enhancements

### Milestone Checkpoints

**Checkpoint 1: "3D Tracking Working"**
- 3D shader compiles and runs
- Points have Z coordinates
- Orthographic projection displays correctly
- Existing 2D presets still work

**Checkpoint 2: "3D Variations Working"**
- At least 5 3D variations implemented
- Rotation variations functional
- Z-scale and Z-translate working
- Can create recognizable 3D flame

**Checkpoint 3: "Feature Complete"**
- All planned 3D variations implemented
- Perspective projection working
- UI controls complete
- 3D presets available

**Checkpoint 4: "Polish Complete"**
- Documentation updated
- Performance validated
- Optional depth effects implemented
- Ready for release

---

## Technical Decisions to Make

### Decision 1: Affine Matrix Representation
- **A.** Keep 2D affine + Z offset (easier migration)
- **B.** Full 3x3 affine matrix (more flexible)

**Recommendation:** A for MVP, consider B later

### Decision 2: Shader Architecture
- **A.** Single shader with runtime conditionals
- **B.** Separate 2D and 3D shaders

**Recommendation:** B for cleaner code and performance

### Decision 3: Variation Parameters
- **A.** Hardcoded parameters in variations
- **B.** Per-transform parameter array
- **C.** Global variation parameter system

**Recommendation:** Start with A, add B if needed

### Decision 4: Backward Compatibility
- **A.** Auto-convert old configs to 2D mode
- **B.** Require manual migration
- **C.** Support both formats forever

**Recommendation:** A with migration warnings

---

## Risk Assessment

### High Risk
- 🔴 **Breaking existing presets** - Mitigate with careful serialization versioning
- 🔴 **Performance regression** - Benchmark early and often

### Medium Risk
- 🟡 **GPU buffer size limits** - 3D transforms are larger
- 🟡 **Shader complexity** - May hit instruction limits
- 🟡 **UI complexity** - Many new controls

### Low Risk
- 🟢 **Algorithm correctness** - Well-documented in Apophysis
- 🟢 **Platform compatibility** - No new GPU features needed

---

## Success Criteria

- ✅ All existing 2D flames render identically
- ✅ Can render recognizable 3D fractal flames
- ✅ Perspective projection works correctly
- ✅ At least 10 3D variations implemented
- ✅ UI intuitive for 2D ↔ 3D switching
- ✅ Performance within 20% of 2D mode
- ✅ Serialization preserves all 3D state

---

## Next Steps

1. Review this plan and confirm approach
2. Make technical decisions (affine representation, shader architecture)
3. Begin Phase 1: Data structure updates
4. Proceed step-by-step with checkpoints

**Ready to start? Let's begin with Phase 1.1: Transform structure updates.**
