# Variation System Architecture

**Overview:** The variation system provides a registry of nonlinear transformation functions that create fractal structure. Supports 26 core variations plus unlimited plugin variations with parameters.

**See also:**
- [ARCHITECTURE.md](../ARCHITECTURE.md) - Overall system design
- [TRANSFORMS.md](TRANSFORMS.md) - Flame algorithm and variation blending
- [SHADERS.md](SHADERS.md) - Shader implementation details

**Code locations:**
- [src/variations/mod.rs](../../src/variations/mod.rs) - VariationRegistry and core registration
- [src/shader_builder_v2.rs](../../src/shader_builder_v2.rs) - Dynamic WGSL variation code generation
- **Note:** Variation shader code is generated at runtime, not stored in separate files

---

## What is a Variation?

A **variation** is a nonlinear function that transforms a 2D or 3D point:

```
v: ℝ² → ℝ²  (2D variation)
v: ℝ³ → ℝ³  (3D variation)
```

**Examples:**
- **Linear**: `v(x, y) = (x, y)` - Identity (no change)
- **Sinusoidal**: `v(x, y) = (sin(x), sin(y))` - Wraps space periodically
- **Spherical**: `v(x, y) = (x/r², y/r²)` where `r² = x² + y²` - Inverts through unit circle
- **Swirl**: `v(x, y) = (x·sin(r²) - y·cos(r²), x·cos(r²) + y·sin(r²))` - Rotates by distance

In the fractal flame algorithm, variations are **blended additively**:
```
result = w₁·v₁(p) + w₂·v₂(p) + ... + wₙ·vₙ(p)
```

This creates infinite variety from a small set of building blocks.

---

## Variation Registry

### Architecture

The registry is a **global singleton** that stores metadata for all variations:

**Location:** [src/variations/mod.rs](../../src/variations/mod.rs)

```rust
pub struct VariationRegistry {
    // Name → metadata
    variations: HashMap<String, VariationInfo>,

    // Core variations (fixed indices 0-25)
    ordered_names: Vec<String>,
}

pub struct VariationInfo {
    pub name: String,              // Internal name ("linear")
    pub display_name: String,      // UI name ("Linear")
    pub category: VariationCategory,
    pub shader_index: usize,       // GPU array index (0-25 for core)
    pub needs_rng: bool,           // Requires random number generator
    pub parameters: Vec<VariationParameter>,
}

pub enum VariationCategory {
    Basic2D,      // 0-4: Linear, Sinusoidal, Spherical, Swirl, Horseshoe
    Advanced2D,   // 5-15: Polar, Handkerchief, Heart, Disc, Spiral, etc.
    Depth3D,      // 16-17, 23: Zcone, Flatten, ZScale (modify Z only)
    Full3D,       // 18: Hemisphere (full 3D structure)
    Rotation3D,   // 19-22: PreRotate/PostRotate X/Y
}
```

### Global Access

```rust
use once_cell::sync::Lazy;

static GLOBAL_REGISTRY: Lazy<VariationRegistry> = Lazy::new(|| {
    VariationRegistry::new()
});

pub fn global_registry() -> &'static VariationRegistry {
    &GLOBAL_REGISTRY
}
```

**Why singleton?**
- All code paths use same variation ID mapping
- UI, shader builder, GPU upload stay synchronized
- Avoids passing registry around everywhere

### Two-Tier ID System

**Core Variations (0-25):** Fixed indices, never change
```
ordered_names[0] = "linear"      → shader index 0
ordered_names[1] = "sinusoidal"  → shader index 1
...
ordered_names[25] = "blob"       → shader index 25
```

**Plugin Variations (26-49):** Dynamic indices per-flame
- Registry can hold unlimited plugins (hundreds/thousands)
- Only active plugins get shader indices 26-49
- Shader is dynamically compiled with active set
- Example: If flame uses "custom1" and "custom2", they get indices 26 and 27

**Why this design?**
- Core variations have stable IDs (backward compatibility with presets)
- Plugin system is flexible (no recompilation to add variations)
- Shader stays small (only active variations compiled)

---

## Core Variations (0-25)

### Basic 2D Variations (0-4)

**0. Linear**
```rust
fn variation_linear(p: vec2<f32>) -> vec2<f32> {
    return p;  // Identity
}
```
- No transformation
- Often used as base blend

**1. Sinusoidal**
```rust
fn variation_sinusoidal(p: vec2<f32>) -> vec2<f32> {
    return vec2(sin(p.x), sin(p.y));
}
```
- Wraps space periodically
- Creates wave patterns

**2. Spherical**
```rust
fn variation_spherical(p: vec2<f32>) -> vec2<f32> {
    let r2 = dot(p, p) + 1e-6;
    return p / r2;
}
```
- Inverts through unit circle
- Near points go far, far points come near

**3. Swirl**
```rust
fn variation_swirl(p: vec2<f32>) -> vec2<f32> {
    let r2 = dot(p, p);
    let sin_r2 = sin(r2);
    let cos_r2 = cos(r2);
    return vec2(
        p.x * sin_r2 - p.y * cos_r2,
        p.x * cos_r2 + p.y * cos_r2
    );
}
```
- Rotates points by their distance from origin
- Creates spiral patterns

**4. Horseshoe**
```rust
fn variation_horseshoe(p: vec2<f32>) -> vec2<f32> {
    let r = length(p) + 1e-6;
    return vec2(
        (p.x - p.y) * (p.x + p.y) / r,
        2.0 * p.x * p.y / r
    );
}
```
- Bends space into horseshoe shape

### Advanced 2D Variations (5-15)

**5. Polar**
```rust
fn variation_polar(p: vec2<f32>) -> vec2<f32> {
    let r = length(p);
    let theta = atan2(p.y, p.x);
    return vec2(theta / 3.14159265359, r - 1.0);
}
```
- Converts cartesian → polar, then treats as cartesian

**6. Handkerchief**
```rust
fn variation_handkerchief(p: vec2<f32>) -> vec2<f32> {
    let r = length(p);
    let theta = atan2(p.y, p.x);
    return vec2(
        r * sin(theta + r),
        r * cos(theta - r)
    );
}
```

**7. Heart**
```rust
fn variation_heart(p: vec2<f32>) -> vec2<f32> {
    let r = length(p);
    let theta = atan2(p.y, p.x);
    return vec2(
        r * sin(theta * r),
        -r * cos(theta * r)
    );
}
```
- Creates heart-shaped patterns

**8. Disc**
```rust
fn variation_disc(p: vec2<f32>) -> vec2<f32> {
    let r = length(p);
    let theta = atan2(p.y, p.x);
    let factor = theta / 3.14159265359;
    return vec2(
        factor * sin(3.14159265359 * r),
        factor * cos(3.14159265359 * r)
    );
}
```

**9. Spiral**
```rust
fn variation_spiral(p: vec2<f32>) -> vec2<f32> {
    let r = length(p) + 1e-6;
    let theta = atan2(p.y, p.x);
    return vec2(
        (cos(theta) + sin(r)) / r,
        (sin(theta) - cos(r)) / r
    );
}
```

**10. Hyperbolic**
```rust
fn variation_hyperbolic(p: vec2<f32>) -> vec2<f32> {
    let r = length(p) + 1e-6;
    let theta = atan2(p.y, p.x);
    return vec2(sin(theta) / r, r * cos(theta));
}
```

**11. Diamond**
```rust
fn variation_diamond(p: vec2<f32>) -> vec2<f32> {
    let r = length(p);
    let theta = atan2(p.y, p.x);
    return vec2(sin(theta) * cos(r), cos(theta) * sin(r));
}
```

**12. Ex**
```rust
fn variation_ex(p: vec2<f32>) -> vec2<f32> {
    let r = length(p);
    let theta = atan2(p.y, p.x);
    let p0 = sin(theta + r);
    let p1 = cos(theta - r);
    let p0_cubed = p0 * p0 * p0;
    let p1_cubed = p1 * p1 * p1;
    return vec2(r * (p0_cubed + p1_cubed), r * (p0_cubed - p1_cubed));
}
```

**13. Julia (needs RNG)**
```rust
fn variation_julia(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    let r = sqrt(length(p));
    let theta = atan2(p.y, p.x) * 0.5;
    let offset = if pcg_float(rng) < 0.5 { 0.0 } else { 3.14159265359 };
    return vec2(r * cos(theta + offset), r * sin(theta + offset));
}
```
- Randomly selects branch of Julia set
- Requires RNG (non-deterministic)

**14. Bent**
```rust
fn variation_bent(p: vec2<f32>) -> vec2<f32> {
    let x = if p.x >= 0.0 { p.x } else { p.x * 2.0 };
    let y = if p.y >= 0.0 { p.y } else { p.y * 0.5 };
    return vec2(x, y);
}
```
- Stretches space in different directions by quadrant

**15. Waves**
```rust
fn variation_waves(p: vec2<f32>, xform_id: u32) -> vec2<f32> {
    // Note: In CPU implementation, uses transform params b, c, e, f
    // GPU version would need to access transform directly
    let b = 0.5;  // Placeholder
    let c = 0.5;
    let e = 0.5;
    let f = 0.5;
    return vec2(
        p.x + b * sin(p.y / (c * c + 1e-6)),
        p.y + e * sin(p.x / (f * f + 1e-6))
    );
}
```
- Creates wave distortions

### 3D Depth Variations (16-17, 23)

These variations modify **only the Z coordinate**, preserving XY structure.

**16. Zcone**
```rust
fn variation_zcone(p: vec3<f32>) -> vec3<f32> {
    let r = sqrt(p.x * p.x + p.y * p.y);
    return vec3(p.x, p.y, r);  // Z = distance from origin
}
```
- Creates cone shape in Z
- Good for adding depth to 2D patterns

**17. Flatten**
```rust
fn variation_flatten(p: vec3<f32>) -> vec3<f32> {
    return vec3(p.x, p.y, p.z * 0.5);  // Compress Z
}
```
- Reduces Z range
- Controls depth extent

**23. ZScale (parameterized)**
```rust
fn variation_zscale(p: vec3<f32>, xform_id: u32) -> vec3<f32> {
    let scale = get_param(xform_id, 23u, 0u);
    return vec3(p.x, p.y, p.z * scale);
}
```
- User-controlled Z scaling
- Parameter: scale factor

### Full 3D Variations (18)

**18. Hemisphere**
```rust
fn variation_hemisphere(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    let r = sqrt(p.x * p.x + p.y * p.y + p.z * p.z);
    if r < 1e-6 { return p; }

    let normalized = p / r;
    return normalized * (1.0 + pcg_float(rng) * 0.1);
}
```
- Projects onto sphere surface
- Creates true 3D structure
- Needs RNG for variation

### 3D Rotation Variations (19-22)

Apply rotation matrices to the full 3D vector.

**19. PreRotateX**
```rust
fn variation_pre_rotate_x(p: vec3<f32>, xform_id: u32) -> vec3<f32> {
    let angle = get_param(xform_id, 19u, 0u);
    let cos_a = cos(angle);
    let sin_a = sin(angle);
    return vec3(
        p.x,
        p.y * cos_a - p.z * sin_a,
        p.y * sin_a + p.z * cos_a
    );
}
```

**20. PreRotateY**
```rust
fn variation_pre_rotate_y(p: vec3<f32>, xform_id: u32) -> vec3<f32> {
    let angle = get_param(xform_id, 20u, 0u);
    let cos_a = cos(angle);
    let sin_a = sin(angle);
    return vec3(
        p.x * cos_a - p.z * sin_a,
        p.y,
        p.x * sin_a + p.z * cos_a
    );
}
```

**21. PostRotateX** - Same as PreRotateX but applied after other variations

**22. PostRotateY** - Same as PreRotateY but applied after other variations

### Parameterized Variations (24-25)

**24. JuliaN**
```rust
fn variation_julian(p: vec2<f32>, xform_id: u32) -> vec2<f32> {
    let power = get_param(xform_id, 24u, 0u);     // Integer: -10 to 10
    let dist = get_param(xform_id, 24u, 1u);      // Float: 0.0 to 10.0

    let r = pow(dot(p, p), dist / power * 0.5);
    let theta = (atan2(p.y, p.x) + 6.28318530718 * floor(pcg_float(rng) * abs(power))) / power;

    return vec2(r * cos(theta), r * sin(theta));
}
```
- Generalized Julia set
- Parameters: power (symmetry), distance (scaling)

**25. Blob**
```rust
fn variation_blob(p: vec2<f32>, xform_id: u32) -> vec2<f32> {
    let high = get_param(xform_id, 25u, 0u);     // Float: 0.0 to 10.0
    let low = get_param(xform_id, 25u, 1u);      // Float: 0.0 to 10.0
    let waves = get_param(xform_id, 25u, 2u);    // Float: 1.0 to 20.0

    let r = length(p);
    let theta = atan2(p.y, p.x);
    let blob_factor = low + (high - low) * 0.5 * (sin(waves * theta) + 1.0);

    return vec2(r * blob_factor * cos(theta), r * blob_factor * sin(theta));
}
```
- Creates blob-like distortions
- Parameters: high/low (size range), waves (frequency)

---

## Variation Parameters

### Parameter System

**Structure:**
```rust
pub struct VariationParameter {
    pub name: String,          // Internal name ("power")
    pub display_name: String,  // UI name ("Power")
    pub param_type: ParamType,
    pub default_value: f32,
    pub min_value: Option<f32>,
    pub max_value: Option<f32>,
}

pub enum ParamType {
    Float,    // Continuous value
    Integer,  // Whole numbers
    Angle,    // 0-360° (stored as radians)
}
```

### Adding Parameters to Variation

**Registration:**
```rust
impl VariationRegistry {
    pub fn new() -> Self {
        let mut registry = Self::default();

        // Register variation
        registry.register_core("julian", "JuliaN", VariationCategory::Advanced2D, true);

        // Add parameters
        registry.add_parameters("julian", vec![
            VariationParameter {
                name: "power".to_string(),
                display_name: "Power".to_string(),
                param_type: ParamType::Integer,
                default_value: 2.0,
                min_value: Some(-10.0),
                max_value: Some(10.0),
            },
            VariationParameter {
                name: "dist".to_string(),
                display_name: "Distance".to_string(),
                param_type: ParamType::Float,
                default_value: 1.0,
                min_value: Some(0.0),
                max_value: Some(10.0),
            },
        ]);

        registry
    }
}
```

### GPU Storage

Parameters are uploaded to a dedicated storage buffer:

```rust
// In src/gpu/buffers.rs
struct GpuVariationParams {
    params: [f32; 400],  // 50 variations × 8 params each
}

// Access in shader
fn get_param(xform_id: u32, variation_id: u32, param_slot: u32) -> f32 {
    let idx = variation_id * 8u + param_slot;
    return variation_params[xform_id].params[idx];
}
```

### UI Display

Parameter sliders automatically appear below active variations in the Transforms window:

```rust
// Pseudo-code for UI rendering
for (var_name, weight) in &transform.variations {
    // Weight slider
    ui.add(Slider::new(weight, 0.0..=2.0).text(var_name));

    // Parameter sliders (if variation has parameters)
    if let Some(params) = registry.get_parameters(var_name) {
        for param in params {
            let value = transform.get_variation_param(var_name, &param.name);
            match param.param_type {
                ParamType::Float => {
                    ui.add(Slider::new(value, param.min..=param.max).text(param.display_name));
                }
                ParamType::Integer => {
                    ui.add(Slider::new(value, param.min..=param.max).step_by(1.0).text(param.display_name));
                }
                ParamType::Angle => {
                    ui.add(Slider::new(value, 0.0..=360.0).suffix("°").text(param.display_name));
                }
            }
        }
    }
}
```

---

## Adding New Variations

### Step-by-Step Guide

**1. Register in VariationRegistry**

Edit [src/variations/mod.rs](../../src/variations/mod.rs):

```rust
impl VariationRegistry {
    pub fn new() -> Self {
        let mut registry = Self::default();

        // ... existing registrations

        // Add your variation
        registry.register_core(
            "myvariation",           // Internal name
            "My Variation",          // Display name
            VariationCategory::Advanced2D,
            false                    // needs_rng: true if uses random numbers
        );

        registry
    }
}
```

**2. Add WGSL Code Generation**

Edit [src/shader_builder_v2.rs](../../src/shader_builder_v2.rs) to generate the variation function.
The shader builder generates WGSL code dynamically - there are no separate variation files.

**2D Implementation** (generated for 2D mode):
```wgsl
fn variation_myvariation(p: vec2<f32>) -> vec2<f32> {
    // Your transformation here
    let r = length(p);
    let theta = atan2(p.y, p.x);
    return vec2(r * cos(theta * 2.0), r * sin(theta * 2.0));
}
```

**3D Implementation** (generated for 3D mode):
```wgsl
fn variation_myvariation(p: vec3<f32>) -> vec3<f32> {
    // For 2D variations in 3D mode, pass Z through
    let r = length(p.xy);
    let theta = atan2(p.y, p.x);
    return vec3(r * cos(theta * 2.0), r * sin(theta * 2.0), p.z);
}
```

**Note:** Both 2D and 3D code are generated by the same function in shader_builder_v2.rs based on the `render_3d` flag.

**4. (Optional) Add Parameters**

```rust
registry.add_parameters("myvariation", vec![
    VariationParameter {
        name: "strength".to_string(),
        display_name: "Strength".to_string(),
        param_type: ParamType::Float,
        default_value: 1.0,
        min_value: Some(0.0),
        max_value: Some(5.0),
    },
]);
```

Then update shader to use parameter:
```wgsl
fn variation_myvariation(p: vec2<f32>, xform_id: u32) -> vec2<f32> {
    let strength = get_param(xform_id, VARIATION_INDEX, 0u);
    // Use strength in calculation
}
```

**5. Done!**

Variation automatically appears in UI under its category. ShaderBuilder will generate correct function calls based on `needs_rng` and parameter presence.

---

## UI Ordering Rule

**CRITICAL:** Variations must be listed in registration order, NOT HashMap order.

**Wrong (random order):**
```rust
for var_info in registry.variations.values() {
    // HashMap iteration is RANDOM!
}
```

**Correct (registration order):**
```rust
for var_name in registry.ordered_names() {
    let var_info = registry.get(var_name);
    // Preserves registration order
}
```

**Why?** Registration order determines shader indices. UI must match to avoid confusion.

---

## Common Variation Tasks

| Task | Files to Modify |
|------|-----------------|
| Add 2D variation | [variations/mod.rs](../../src/variations/mod.rs) (register), [shader_builder_v2.rs](../../src/shader_builder_v2.rs) (generate WGSL) |
| Add 3D variation | [variations/mod.rs](../../src/variations/mod.rs) (register), [shader_builder_v2.rs](../../src/shader_builder_v2.rs) (generate WGSL) |
| Add parameters | [variations/mod.rs](../../src/variations/mod.rs) `add_parameters()`, shader uses `get_param()` |
| Change category | [variations/mod.rs](../../src/variations/mod.rs) registration |
| Fix UI ordering | [ui/mod.rs](../../src/ui/mod.rs) - use `ordered_names()` not `values()` |

---

**Last Updated:** 2026-01-24
**Related Documentation:**
- [TRANSFORMS.md](TRANSFORMS.md) - Flame algorithm and variation blending
- [SHADERS.md](SHADERS.md) - Shader implementation
- [ARCHITECTURE.md](../ARCHITECTURE.md) - Overall system
