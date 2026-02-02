# Post-Affine Transforms

## Overview

Add per-transform **post-affine** support to the fractal flame renderer. Post-affines are an optional second affine transform applied **after** variations, complementing the existing pre-affine which is applied before variations.

**Iteration pipeline (current):**
```
point -> pre-affine -> variations -> store
```

**Iteration pipeline (with post-affine):**
```
point -> pre-affine -> variations -> post-affine -> store
```

Post-affines are a standard feature in Apophysis and other fractal flame software. They allow rotation, scaling, shearing, and translation of the variation output without affecting the input to the variation functions.

## Requirements

1. **7 fields per transform** matching the pre-affine: `post_a`, `post_b`, `post_c`, `post_d`, `post_e`, `post_f`, `post_g`
2. **Fully optional** with an `post_affine_enabled` flag per transform
3. **Near-zero shader performance impact** when disabled (compile-time elimination)
4. **All rendering routes** must support post-affines (interactive, export, tiled)
5. **FractalConfig & ConfigManager** integration with undo/redo
6. **Animation system** integration (animatable via ConfigPath)
7. **Triangle Editor** visualization when enabled
8. **UI toggle** under Advanced section in Transforms panel

## Apophysis Reference

In Apophysis XML, post-affines are stored as 6 coefficients per transform:

```xml
<xform weight="0.5" coefs="a c b d e f" post="pa pc pb pd pe pf" ...>
```

### Simultaneous Affine (Same Math as Pre-Affine)

The post-affine is a standard simultaneous affine transform, identical in form to the pre-affine. Both Apophysis and JWildfire compute it this way.

**Apophysis** (`XForm.pas:550-554`, `DoPostTransform`) preserves the original x via a temp variable:
```pascal
tmp := FPx;                              // save original
FPx := p00 * FPx + p10 * FPy + p20;     // new x from originals
FPy := p01 * tmp + p11 * FPy + p21;     // new y from originals (tmp = old FPx)
```

**JWildfire** ([TransformationPostAffineFlatStep.java](https://github.com/thargor6/JWildfire/blob/master/src/org/jwildfire/create/tina/base/TransformationPostAffineFlatStep.java)) reads from `pVarT` and writes to `pDstPoint` (separate variables):
```java
pDstPoint.x = xform.xyPostCoeff00 * pVarT.x + xform.xyPostCoeff10 * pVarT.y + xform.xyPostCoeff20;
pDstPoint.y = xform.xyPostCoeff01 * pVarT.x + xform.xyPostCoeff11 * pVarT.y + xform.xyPostCoeff21;
```

Both produce:
```
x' = a*x + b*y + e
y' = c*x + d*y + f
```

When no post-affine is specified, the identity is implied (no-op):
```
a=1, b=0, c=0, d=1, e=0, f=0, g=0
```

## Implementation Plan

### Step 1: Transform Struct (CPU)

**File:** `src/scene/transforms.rs`

Add 8 new fields to the `Transform` struct:

```rust
pub struct Transform {
    // Pre-affine (existing)
    pub a: f32, pub b: f32, pub c: f32, pub d: f32,
    pub e: f32, pub f: f32, pub g: f32,

    // Post-affine (NEW)
    pub post_affine_enabled: bool,
    pub post_a: f32, pub post_b: f32, pub post_c: f32, pub post_d: f32,
    pub post_e: f32, pub post_f: f32, pub post_g: f32,

    // ... rest unchanged ...
}
```

Default values: `post_affine_enabled = false`, identity matrix (`post_a=1, post_d=1`, rest `0`).

Add triangle conversion methods mirroring the existing pre-affine:
- `to_post_triangle_apophysis() -> ([f32; 2], [f32; 2], [f32; 2])`
- `from_post_triangle_apophysis(&mut self, o, x, y)`

Same math as existing `to_triangle_apophysis()` / `from_triangle_apophysis()` but reading/writing `post_*` fields.

### Step 2: GPU Transform Struct

**File:** `src/gpu/buffers.rs`

Add post-affine fields to `GpuTransform`:

```rust
#[repr(C)]
pub struct GpuTransform {
    // Pre-affine (existing)
    pub a: f32, pub b: f32, pub c: f32, pub d: f32,
    pub e: f32, pub f: f32, pub g: f32,
    pub weight: f32,

    // Post-affine (NEW) - 8 floats
    pub post_a: f32, pub post_b: f32, pub post_c: f32, pub post_d: f32,
    pub post_e: f32, pub post_f: f32, pub post_g: f32,
    pub post_enabled: f32,  // 0.0 = disabled, 1.0 = enabled (f32 for GPU alignment)

    // Variations (existing)
    pub variations: [f32; 100],

    // Color data (existing)
    pub color: f32,
    pub color_speed: f32,
    pub opacity: f32,
    pub _padding: f32,
}
```

**Alignment note:** The 8 new floats (32 bytes) sit between `weight` and `variations`. This is naturally aligned for std430 storage buffers since all fields are f32.

### Step 3: Shader Structs

**Files:** `shaders/core/header.wgsl`, `shaders/core/header_export.wgsl`, `shaders/core/header_tiled.wgsl`

Update the WGSL `Transform` struct to match:

```wgsl
struct Transform {
    a: f32, b: f32, c: f32, d: f32,
    e: f32, f: f32, g: f32,
    weight: f32,
    // Post-affine
    post_a: f32, post_b: f32, post_c: f32, post_d: f32,
    post_e: f32, post_f: f32, post_g: f32,
    post_enabled: f32,
    // Variations
    variations: array<f32, 100>,
    // Color
    color: f32, color_speed: f32, opacity: f32,
    _padding: f32,
}
```

### Step 4: Shader Post-Affine Functions

**Files:** `shaders/core/affine.wgsl`, `shaders/core/affine_3d.wgsl`

Add post-affine application functions. Same simultaneous affine math as `apply_affine()`:

```wgsl
// In affine.wgsl (2D)
fn apply_post_affine(xform: Transform, p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(
        xform.post_a * p.x + xform.post_b * p.y + xform.post_e,
        xform.post_c * p.x + xform.post_d * p.y + xform.post_f
    );
}

// In affine_3d.wgsl (3D)
fn apply_post_affine(xform: Transform, p: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        xform.post_a * p.x + xform.post_b * p.y + xform.post_e,
        xform.post_c * p.x + xform.post_d * p.y + xform.post_f,
        p.z + xform.post_g
    );
}
```

### Step 5: Shader Integration (Near-Zero Impact Strategy)

Use a **compile-time constant** `HAS_POST_AFFINE` (same pattern as `HAS_FINAL_TRANSFORM`) to completely eliminate post-affine code when no transform uses it.

**ShaderBuilder** (`src/renderer/compute_kernel.rs` or wherever shaders are assembled):
- Scan all transforms: if ANY has `post_affine_enabled = true`, set `HAS_POST_AFFINE = true`
- When `false`, the post-affine code block is dead-eliminated by the GPU compiler

**All 6 shader entry points** need the same change:

| File | Lines affected |
|------|---------------|
| `shaders/core/main_template.wgsl` | After line 93, after lines 160/163 |
| `shaders/core/main_2d_export.wgsl` | After line 62, after line 88 |
| `shaders/core/main_3d_export.wgsl` | After line 63, after line 89 |
| `shaders/core/main_2d_tiled.wgsl` | After line 65, after line 91 |
| `shaders/core/main_3d_tiled.wgsl` | After line 66, after line 92 |

**Pattern (main_template.wgsl example):**

```wgsl
// Apply affine + variations (existing)
let affine_p = apply_affine(xform, current);
current = apply_variations(xform, xform_idx, affine_p, &rng);

// Apply post-affine (NEW - compile-time gated)
if (HAS_POST_AFFINE) {
    if (xform.post_enabled > 0.5) {
        current = apply_post_affine(xform, current);
    }
}
```

**Performance characteristics:**
- When `HAS_POST_AFFINE = false`: Zero GPU cost (code eliminated at compile time)
- When `HAS_POST_AFFINE = true` but specific transform disabled: One branch per iteration (negligible)
- When enabled: One additional affine multiply-add per iteration (trivial cost)

**Shader recompilation:** Already happens when transforms change (variation set changes trigger recompilation). Adding post-affine enable/disable will trigger the same recompilation path.

### Step 6: FractalConfig Serialization

**File:** `src/config/fractal_config.rs`

Post-affine fields serialize with `skip_serializing_if` to keep .fflame files clean when not used:

```rust
#[serde(default, skip_serializing_if = "is_false")]
pub post_affine_enabled: bool,
#[serde(default = "default_post_a", skip_serializing_if = "is_identity_a")]
pub post_a: f32,
// ... etc
```

Backward compatibility: Old .fflame files without post-affine fields will deserialize with defaults (disabled, identity matrix). No migration needed.

### Step 7: ConfigManager / ConfigPath

**File:** `src/config/delta.rs`

Add new ConfigPath variants:

```rust
pub enum ConfigPath {
    // ... existing variants ...

    // Post-affine (NEW)
    TransformPostAffineEnabled { index: usize },
    TransformPostAffine { index: usize, param: AffineParam },

    // Final transform post-affine (NEW)
    FinalTransformPostAffineEnabled,
    FinalTransformPostAffine { param: AffineParam },
}
```

**UpdateType:** Post-affine changes return `UpdateType::Flame` (same as pre-affine) since they affect the IFS iteration and require accumulation reset.

Add corresponding `ConfigValue` conversion, `apply_delta()`, `get_value()`, and `to_string_key()` implementations following the existing `TransformAffine` pattern exactly.

### Step 8: Animation System

**File:** `src/animation/mod.rs`, `src/config/delta.rs`

The animation system is parameter-agnostic via ConfigPath string keys. Adding post-affine ConfigPath variants automatically makes them animatable:

- `TransformPostAffineEnabled { index }` -> `"transform.0.post_affine_enabled"`
- `TransformPostAffine { index, param: A }` -> `"transform.0.post_affine.a"`

No changes needed to `AnimationController` or `interpolation.rs`. The existing `evaluate_at_time()` -> `ConfigPath::from_string_key()` -> `config_manager.update_param_silent()` pipeline handles it automatically.

The animation panel's track editor will show post-affine parameters in the parameter picker once ConfigPath variants exist.

### Step 9: Transforms Panel UI

**File:** `src/ui/transforms.rs`

Add under the existing **Advanced** collapsing section:

1. **Enable Post-Affine checkbox** - toggles `post_affine_enabled`
2. **Post-Affine matrix controls** - same DragValue layout as pre-affine (a/b, c/d, e/f rows + g for 3D)
3. Controls only visible when post-affine is enabled
4. Same Apophysis sign convention (negate b, c, f for display)
5. "Reset to Identity" button to quickly reset post-affine to no-op

```
Advanced
  [Pre-Affine Matrix]
    a: [___] b: [___]
    c: [___] d: [___]
    e: [___] f: [___]
    g: [___]            (3D only)

  [x] Enable Post-Affine    <-- NEW
  [Post-Affine Matrix]      <-- NEW (visible when enabled)
    a: [___] b: [___]
    c: [___] d: [___]
    e: [___] f: [___]
    g: [___]            (3D only)
    [Reset to Identity]
```

### Step 10: Triangle Editor

**File:** `src/ui/triangle_editor.rs`

When a transform has post-affine enabled, show both triangles:

1. **Pre-affine triangle** - Existing solid lines, full color (current behavior)
2. **Post-affine triangle** - Dashed lines, lighter shade of same color

**Interaction:**
- Add a toggle or tab to switch between editing pre-affine and post-affine triangles
- Only the active affine type responds to drag/rotate/scale interactions
- Both triangles always visible for the selected transform (when post-affine is enabled)
- Non-selected transforms show pre-affine only (to avoid visual clutter)

**Triangle styling reference:**
- Pre-affine: Solid lines (existing, lines 512-591)
- Post-affine: Dashed/dotted lines (same technique as final transform, lines 593-664)
- Post-affine vertices: Hollow circles vs filled circles for pre-affine

**Quick Actions** (Move, Scale, Rotate buttons) apply to whichever affine type is currently selected.

### Step 11: Apophysis XML Import

**File:** `src/scene/transforms.rs` (or wherever XML import lives)

Parse the `post` attribute from Apophysis `<xform>` elements:

```xml
<xform ... post="pa pc pb pd pe pf" ...>
```

Note the Apophysis coefficient order: `pa pc pb pd pe pf` (not `pa pb pc pd pe pf`). Map to our fields:
- `post_a = pa`, `post_c = pc`, `post_b = pb`, `post_d = pd`, `post_e = pe`, `post_f = pf`
- `post_affine_enabled = true` (if post attribute is present and non-identity)
- `post_g = 0.0` (Apophysis doesn't have 3D post-affine Z)

## Files Affected

### Core Data Structures
| File | Change |
|------|--------|
| `src/scene/transforms.rs` | Add post-affine fields to Transform, triangle conversion methods |
| `src/gpu/buffers.rs` | Add post-affine fields to GpuTransform |
| `src/config/fractal_config.rs` | Serialization with skip_serializing_if |
| `src/config/delta.rs` | New ConfigPath variants, ConfigValue conversion, apply/get |
| `src/config/manager.rs` | No changes needed (generic over ConfigPath) |

### Shaders (6 entry points + 2 affine files + 3 headers)
| File | Change |
|------|--------|
| `shaders/core/header.wgsl` | Add post-affine fields to Transform struct |
| `shaders/core/header_export.wgsl` | Same |
| `shaders/core/header_tiled.wgsl` | Same |
| `shaders/core/affine.wgsl` | Add `apply_post_affine()` (2D) |
| `shaders/core/affine_3d.wgsl` | Add `apply_post_affine()` (3D) |
| `shaders/core/main_template.wgsl` | Post-affine step after variations |
| `shaders/core/main_2d_export.wgsl` | Same |
| `shaders/core/main_3d_export.wgsl` | Same |
| `shaders/core/main_2d_tiled.wgsl` | Same |
| `shaders/core/main_3d_tiled.wgsl` | Same |

### Shader Builder
| File | Change |
|------|--------|
| `src/renderer/compute_kernel.rs` | Detect post-affine usage, set `HAS_POST_AFFINE` constant |

### UI
| File | Change |
|------|--------|
| `src/ui/transforms.rs` | Enable checkbox + post-affine matrix controls in Advanced |
| `src/ui/triangle_editor.rs` | Dual-triangle rendering, affine type toggle |

### Animation
| File | Change |
|------|--------|
| `src/animation/mod.rs` | No changes (ConfigPath-agnostic) |
| `src/config/delta.rs` | String key serialization for new ConfigPath variants |

### Import/Export
| File | Change |
|------|--------|
| Apophysis XML import code | Parse `post` attribute |

## Testing

1. **Unit tests**: Post-affine identity produces same output as no post-affine
2. **Serialization roundtrip**: .fflame save/load with post-affine enabled and disabled
3. **Backward compatibility**: Load old .fflame files (no post-affine fields) without errors
4. **Visual regression**: Export with post-affine enabled vs known Apophysis output
5. **Performance**: Benchmark with `HAS_POST_AFFINE = false` to confirm zero overhead
6. **All rendering routes**: Verify interactive, export (2D/3D), and tiled (2D/3D) all produce identical results
7. **Animation**: Animate post-affine parameters, verify smooth interpolation
8. **Triangle Editor**: Drag post-affine triangle vertices, verify affine values update correctly

## Implementation Order

1. Transform struct + defaults + serialization (Step 1, 6)
2. GPU struct + shader structs (Step 2, 3)
3. Shader functions + iteration loop integration (Step 4, 5)
4. ShaderBuilder compile-time constant (Step 5)
5. ConfigPath + ConfigManager integration (Step 7)
6. Transforms panel UI (Step 9)
7. Triangle Editor (Step 10)
8. Animation system verification (Step 8)
9. Apophysis XML import (Step 11)
10. Testing across all routes

## References

- [Apophysis XML format analysis](../archive/apophysis-phase3/apophysis-xml-import-analysis.md) - Post-affine coefficient order
- [TRANSFORMS.md](../main/TRANSFORMS.md) - Current transform architecture
- [CONFIG.md](../main/CONFIG.md) - ConfigManager delta system
