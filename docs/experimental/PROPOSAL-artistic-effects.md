# PROPOSAL: Artistic Effects (Depth of Field & Motion Blur)

**Status:** Proposed (Not yet implemented)
**Date:** 2025-10-28
**Priority:** Medium - Optional visual enhancements inspired by Terrarium
**Source:** [wrighter.xyz flame fractals article](https://wrighter.xyz/blog/2023_08_17_flame_fractals_in_comp_shader)

---

## Overview

Add two artistic effects commonly used in fractal flame rendering:
1. **Depth of Field (DOF)** - Blur areas away from focus distance for painterly effect
2. **Motion Blur** - Temporal dithering for time-based smoothing and artistic movement

Both effects are simple to implement and work in **both 2D and 3D modes**.

---

## 1. Depth of Field (DOF)

### Concept

Instead of accumulating samples at their exact pixel position, randomly offset the write position based on distance from focus plane. This creates a **natural blur** effect where unfocused areas appear soft and painterly.

**Key insight:** This is NOT post-processing blur - it's **accumulation-time blur** that happens during splatting. Free performance, natural look.

### Mathematical Approach

From wrighter.xyz article:
```glsl
const float focus_dist = 0.7;
const float dof_scale = 1.0;
vec2 dof_sample = dof_scale * get_random_point_in_disk() * (depth - focus_dist);
vec2 anti_aliasing = random_point_in_disk() / min(resolution.x, resolution.y);
ivec2 int_p = ivec2(uv + anti_aliasing + dof_sample) * ivec2(resolution);
```

**How it works:**
- Calculate depth metric (distance from origin, Z value, or custom)
- Offset pixel position by `random_disk × (depth - focus_dist) × dof_scale`
- Areas at `focus_dist` have zero offset (sharp)
- Areas far from focus scatter to neighboring pixels (blurred)
- Anti-aliasing bonus: sub-pixel jittering smooths edges

### Implementation Plan

#### Phase 1: Add DOF Parameters

**File:** `src/gpu/buffers.rs`

Add to `GpuParams` struct (replace `_pad3` with DOF params):
```rust
pub struct GpuParams {
    // ... existing fields ...
    pub histogram_color_scale: f32,
    pub dof_enabled: u32,           // 0 = disabled, 1 = enabled (NEW)
    pub dof_focus_distance: f32,    // Distance where image is sharpest (NEW)
    pub dof_blur_strength: f32,     // 0.0-5.0 blur amount (NEW)
    // pub _pad3: f32,               // REMOVE
}
```

**Alignment check:**
- `GpuParams` currently uses 1 padding slot (`_pad3`)
- We need 3× f32 (12 bytes) for DOF params
- **Action:** Need to check if we have enough padding or add more fields

#### Phase 2: Shader Implementation

**File:** `shaders/core/utilities.wgsl`

Add random disk sampling function:
```wgsl
// Generate random point in unit disk (for DOF and anti-aliasing)
fn random_point_in_disk(rng: ptr<function, RngState>) -> vec2<f32> {
    // Rejection sampling: generate points in [-1,1]² until one falls in unit circle
    loop {
        let x = rng_nextf(rng) * 2.0 - 1.0;
        let y = rng_nextf(rng) * 2.0 - 1.0;
        let len_sq = x * x + y * y;
        if (len_sq <= 1.0 && len_sq > 0.0001) {
            return vec2<f32>(x, y);
        }
    }
}

// Alternative: Faster polar coordinate method
fn random_point_in_disk_polar(rng: ptr<function, RngState>) -> vec2<f32> {
    let theta = rng_nextf(rng) * 6.28318530718; // 2π
    let r = sqrt(rng_nextf(rng)); // sqrt for uniform distribution
    return vec2<f32>(r * cos(theta), r * sin(theta));
}
```

**File:** `shaders/core/main_2d.wgsl` (lines 49-56)

Modify pixel write section to add DOF offset:
```wgsl
// Skip burn-in iterations
if (i >= params.burn_in) {
    // Convert to pixel coordinates
    var pixel = world_to_pixel(current);

    // Apply Depth of Field (if enabled)
    if (params.dof_enabled != 0u) {
        // Calculate depth metric (distance from origin)
        let depth = length(current);

        // Calculate focus offset
        let depth_delta = depth - params.dof_focus_distance;

        // Generate random offset in disk, scaled by depth delta
        let dof_offset = random_point_in_disk_polar(&rng) * depth_delta * params.dof_blur_strength;

        // Apply offset in pixel space
        pixel = pixel + vec2<i32>(i32(dof_offset.x), i32(dof_offset.y));

        // Optional: Add anti-aliasing jitter (sub-pixel smoothing)
        // let aa_jitter = random_point_in_disk_polar(&rng) * 0.5;
        // pixel = pixel + vec2<i32>(i32(aa_jitter.x), i32(aa_jitter.y));
    }

    // Check bounds
    if (pixel.x >= 0 && pixel.x < i32(params.width) &&
        pixel.y >= 0 && pixel.y < i32(params.height)) {
        // ... existing color accumulation code ...
    }
}
```

**File:** `shaders/core/main_3d.wgsl` (same location)

Identical implementation, but use `world_to_pixel_3d()` and can optionally use `p.z` directly for depth:
```wgsl
// For 3D mode: use Z coordinate directly as depth metric
let depth = p.z;  // or: length(p) for radial depth
```

#### Phase 3: UI Controls

**File:** `src/ui/mod.rs` (Render Settings panel)

Add UI sliders after existing render settings:
```rust
// DOF Controls
ui.separator();
ui.label("Depth of Field");

ui.horizontal(|ui| {
    ui.label("Enable DOF:");
    let mut dof_enabled = self.flame.dof_enabled;
    if ui.checkbox(&mut dof_enabled, "").changed() {
        self.flame.dof_enabled = dof_enabled;
        self.view_changed = true;
    }
});

if self.flame.dof_enabled {
    ui.horizontal(|ui| {
        ui.label("Focus Distance:");
        if ui.add(egui::Slider::new(&mut self.flame.dof_focus_distance, 0.0..=5.0)
            .step_by(0.1))
            .changed() {
            self.view_changed = true;
        }
    });

    ui.horizontal(|ui| {
        ui.label("Blur Strength:");
        if ui.add(egui::Slider::new(&mut self.flame.dof_blur_strength, 0.0..=10.0)
            .step_by(0.5))
            .changed() {
            self.view_changed = true;
        }
    });
}
```

#### Phase 4: Config Serialization

**File:** `src/config.rs` (FractalConfig struct)

Add DOF fields for save/load:
```rust
pub struct FractalConfig {
    // ... existing fields ...

    // Depth of Field settings (added 2025-10-28)
    #[serde(default)]
    pub dof_enabled: bool,

    #[serde(default = "default_dof_focus_distance")]
    pub dof_focus_distance: f32,

    #[serde(default = "default_dof_blur_strength")]
    pub dof_blur_strength: f32,
}

fn default_dof_focus_distance() -> f32 { 1.0 }
fn default_dof_blur_strength() -> f32 { 2.0 }
```

### Performance Impact

**Estimated:** <1% overhead
- 2 RNG calls per sample (for disk point)
- 1 distance calculation
- 2 multiply-adds for offset
- No extra texture reads or writes

**Benefit:** Free anti-aliasing from sub-pixel jittering

### Visual Examples (Expected Results)

**DOF Off:**
- Sharp everywhere
- Uniform detail across image

**DOF On (focus_dist=1.0, blur=3.0):**
- Sharp ring at distance 1.0 from origin
- Soft, painterly blur in foreground/background
- Dreamy, artistic look (similar to Terrarium)

---

## 2. Motion Blur

### Concept

Dither the time parameter when using time-based transform animations. This creates temporal smoothing that makes movement appear fluid and cinematic.

**Current limitation:** This codebase has **no animation system yet**, so motion blur would require implementing time-based parameter variation first.

### Mathematical Approach

From wrighter.xyz article:
```glsl
// Instead of fixed time t:
float time = global_time + random() * motion_blur_kernel;

// Use time in transformation functions
vec2 p = rotate(p, sin(time) * angle);
```

**How it works:**
- Each sample uses slightly different time value
- Transforms that depend on time create "smeared" results
- Blur kernel controls temporal spread (larger = more blur)

### Implementation Plan (Future)

**Prerequisite:** Animation system with keyframes
- Need time parameter in `GpuParams`
- Need time-varying transform parameters
- Need keyframe interpolation system

**Once animation exists:**

1. Add to `GpuParams`:
```rust
pub motion_blur_enabled: u32,
pub motion_blur_kernel: f32,  // Temporal spread (0.0-1.0)
pub global_time: f32,          // Current animation time
```

2. Modify shader (any variation that uses time):
```wgsl
// Early in compute shader main()
var time = params.global_time;
if (params.motion_blur_enabled != 0u) {
    // Dither time for this sample
    time += rng_nextf(&rng) * params.motion_blur_kernel;
}

// Later in transform application
// Use dithered 'time' for any time-dependent calculations
```

3. UI controls similar to DOF

**Status:** **Deferred** until animation system is implemented
**Priority:** Low (requires substantial prerequisite work)

---

## Comparison: Wrighter.xyz vs This Codebase vs STATUS.md

| Feature | Wrighter.xyz | This Proposal | STATUS.md Plan |
|---------|-------------|---------------|----------------|
| **DOF Type** | Accumulation-time offset | Same (accumulation-time) | Post-process blur |
| **DOF Complexity** | Very simple (~10 lines) | Very simple (~20 lines) | Complex (depth buffer + blur shader) |
| **DOF Performance** | <1% overhead | <1% overhead | ~10-20% overhead (blur pass) |
| **Works in 2D?** | ✅ Yes | ✅ Yes | ⚠️ Requires 3D depth |
| **Bokeh shapes?** | ❌ No (disk only) | ❌ No (disk only) | ✅ Yes (configurable) |
| **Motion blur** | ✅ Implemented | ⚠️ Needs animation | ⚠️ Needs animation |

**Key difference:** Wrighter.xyz DOF is **accumulation-time** (offset where samples land), while STATUS.md implies **post-process** (blur already-rendered image).

**Recommendation:** Implement wrighter.xyz approach first for simplicity. Can add post-process DOF later for advanced bokeh effects.

---

## Benefits

### Depth of Field
1. **Artistic flexibility** - Direct creative control over focus/blur
2. **Performance** - Essentially free (no extra passes)
3. **2D + 3D compatible** - Works with any depth metric
4. **Anti-aliasing bonus** - Sub-pixel jittering improves quality
5. **Simple implementation** - ~50 total lines of code

### Motion Blur
1. **Temporal smoothing** - Makes animations feel fluid
2. **Cinematic look** - Professional animation aesthetic
3. **Exploration aid** - Shows trajectory history during parameter sweeps
4. **No extra memory** - Uses same RNG system

---

## Alternatives Considered

### Alternative 1: Post-Process Blur (STATUS.md approach)

**Pros:**
- True bokeh shapes (hexagonal, circular, etc.)
- Better quality blur (Gaussian, bilateral filters)
- More control over blur characteristics

**Cons:**
- Much more complex (~500+ lines)
- Requires depth buffer storage
- 10-20% performance cost
- Doesn't work in 2D mode
- Need separate blur shader pass

**Verdict:** Save for future "advanced DOF" feature. Use accumulation-time DOF first.

### Alternative 2: Z-only DOF (3D-specific)

Use `p.z` directly instead of `length(p)`:
```wgsl
let depth = p.z;  // Planar depth (like camera distance)
```

**Pros:**
- More intuitive for 3D viewing
- Matches real camera behavior

**Cons:**
- Only works in 3D mode
- Requires 3D coordinates

**Verdict:** Support both via depth metric choice (add `dof_depth_mode` param later if needed)

---

## Implementation Checklist

### Phase 1: Core DOF (Essential)
- [ ] Add DOF parameters to `GpuParams` struct
- [ ] Add `random_point_in_disk_polar()` to `utilities.wgsl`
- [ ] Implement DOF offset in `main_2d.wgsl` compute shader
- [ ] Implement DOF offset in `main_3d.wgsl` compute shader
- [ ] Test basic functionality (focus_dist=1.0, blur=2.0)

### Phase 2: UI & Config (Polish)
- [ ] Add DOF controls to UI (checkbox + 2 sliders)
- [ ] Add DOF fields to `FractalConfig` struct
- [ ] Add default values and serialization
- [ ] Test config save/load with DOF enabled

### Phase 3: Documentation (Tracking)
- [ ] Update [docs/main/RENDERER.md](../main/RENDERER.md) with DOF section
- [ ] Update [CLAUDE.md](../../CLAUDE.md) to list DOF as implemented
- [ ] Add visual examples to docs (before/after screenshots)
- [ ] Move this file to `docs/projects/` when active

### Phase 4: Motion Blur (Future - requires animation system)
- [ ] Implement animation system with time parameter
- [ ] Add motion blur parameters to `GpuParams`
- [ ] Add time dithering to shader
- [ ] UI controls and config serialization

---

## Testing Plan

### Basic Functionality
1. **DOF disabled** - Should render identically to current behavior
2. **DOF at focus_dist** - Area at exact distance should remain sharp
3. **DOF far from focus** - Areas ±2.0 from focus should show clear blur
4. **Edge cases:**
   - `blur_strength = 0.0` → no effect (same as disabled)
   - `blur_strength = 10.0` → extreme painterly effect
   - `focus_dist = 0.0` → only origin sharp
   - `focus_dist = 5.0` → outer regions sharp

### 2D Mode
- Test with classic 2D presets (Sierpinski, Barnsley)
- Verify depth = `length(current)` works correctly
- Check anti-aliasing improvement on edges

### 3D Mode
- Test with 3D presets (Hemisphere, Zcone)
- Verify depth calculation respects Z coordinate
- Test with camera rotation (DOF should remain stable)

### Performance
- Measure FPS impact (expect <1%)
- Test with high iteration counts (blur quality)
- Profile RNG overhead (should be negligible)

### Config Persistence
- Save config with DOF enabled
- Reload and verify exact reproduction
- Test with different DOF settings (0.0, 1.0, 5.0, 10.0)

---

## Code References

### Key Files to Modify
- [src/gpu/buffers.rs:76-96](../../src/gpu/buffers.rs#L76-L96) - `GpuParams` struct
- [shaders/core/utilities.wgsl](../../shaders/core/utilities.wgsl) - Random disk sampling
- [shaders/core/main_2d.wgsl:49-56](../../shaders/core/main_2d.wgsl#L49-L56) - DOF offset (2D)
- [shaders/core/main_3d.wgsl:49-56](../../shaders/core/main_3d.wgsl#L49-L56) - DOF offset (3D)
- [src/ui/mod.rs](../../src/ui/mod.rs) - UI controls (find "Render Settings" section)
- [src/config.rs](../../src/config.rs) - Serialization

### Related Documentation
- [docs/main/RENDERER.md](../main/RENDERER.md) - 3-pass pipeline
- [docs/main/BUFFERS.md](../main/BUFFERS.md) - GPU buffer layouts
- [docs/main/SHADERS.md](../main/SHADERS.md) - Shader architecture
- [CLAUDE.md](../../CLAUDE.md) - Feature tracking

---

## Open Questions

1. **Depth metric choice:** Should we support multiple depth modes?
   - Radial: `length(current)` (works in 2D+3D)
   - Planar Z: `current.z` (3D only)
   - Custom: User-defined depth function?

   **Recommendation:** Start with radial (simplest), add modes later if needed

2. **Anti-aliasing integration:** Should DOF always include AA jitter?
   - Pro: Free quality improvement
   - Con: Changes existing render behavior slightly

   **Recommendation:** Separate toggle for AA jitter (independent of DOF)

3. **Depth visualization:** Add debug mode to visualize depth as grayscale?
   - Would help users understand focus_dist parameter
   - Could reuse density visualization mode

   **Recommendation:** Add later as debug feature

4. **Preset integration:** Should presets include DOF settings?
   - Some fractals look better with DOF (organic, 3D structures)
   - Others look better sharp (mathematical, geometric)

   **Recommendation:** Yes, include in `FractalConfig` so presets can showcase it

---

## Success Criteria

✅ **Minimum Viable Implementation:**
- DOF can be toggled on/off
- Focus distance and blur strength are adjustable
- Works in both 2D and 3D modes
- No noticeable performance impact (<1%)
- Config save/load preserves DOF settings

✅ **Polish Goals:**
- UI is intuitive with clear parameter names
- Visual feedback shows focus region clearly
- Documentation explains depth metric and artistic use
- Presets showcase DOF artistic potential

✅ **Stretch Goals:**
- Multiple depth modes (radial, planar, custom)
- Depth visualization debug mode
- Anti-aliasing jitter toggle (independent of DOF)
- Motion blur (once animation system exists)

---

## Related Work

- [wrighter.xyz flame fractals article](https://wrighter.xyz/blog/2023_08_17_flame_fractals_in_comp_shader) - Original inspiration
- [Terrarium](https://www.hailpixel.com/articles/terrarium-a-path-tracer) - DOF + motion blur example
- [STATUS.md](../STATUS.md) - Original 3D depth effects plan (post-process approach)
- [3D_IMPLEMENTATION_PLAN.md](../archive/3D_IMPLEMENTATION_PLAN.md) - Initial 3D feature design

---

**Next Steps:**
1. Review this proposal for technical feasibility
2. Decide on depth metric approach (radial vs planar vs both)
3. Implement Phase 1 (core DOF) in experimental branch
4. Test with variety of presets (2D + 3D)
5. Add UI/config (Phase 2) once shader logic is validated
6. Document and promote to main branch once stable
