# Shader System Architecture

**Overview:** The fractal flame renderer uses a modular WGSL shader system with dynamic compilation based on active variations.

**See also:**
- [ARCHITECTURE.md](../ARCHITECTURE.md) - Overall system design
- [TRANSFORMS.md](TRANSFORMS.md) - Flame algorithm
- [BUFFERS.md](BUFFERS.md) - GPU data structures
- [VARIATIONS.md](VARIATIONS.md) - Variation functions *(coming soon)*

**Code locations:**
- [shaders/core/](../../shaders/core/) - Modular shader components
- [src/shader_builder_v2.rs](../../src/shader_builder_v2.rs) - Dynamic shader compilation
- [src/gpu/pipelines.rs](../../src/gpu/pipelines.rs) - Pipeline creation

---

## Shader Architecture

### Modular Component System

The shaders are **not monolithic files**. Instead, they're assembled at runtime from reusable components:

```
shaders/core/
├── header.wgsl           - Structs, bind groups (66 lines)
├── rng.wgsl              - Random number generation (34 lines)
├── utilities.wgsl        - Helper functions (135 lines)
├── variations_2d.wgsl    - 2D variation functions (152 lines)
├── variations_3d.wgsl    - All variations including 3D (202 lines)
├── main_2d.wgsl          - 2D entry point (75 lines)
└── main_3d.wgsl          - 3D entry point (76 lines)
```

**At runtime**, ShaderBuilder combines:
```
[header] + [rng] + [variations] + [GENERATED apply_variations()] + [utilities] + [main]
```

**Result:** A complete compute shader with only the active variations compiled in.

---

## Shader Components

### header.wgsl - Data Structures and Bindings

**Location:** [shaders/core/header.wgsl](../../shaders/core/header.wgsl)

**Contents:**
```wgsl
// Bind group layout (compute pass)
@group(0) @binding(0) var<storage, read> transforms: array<Transform>;
@group(0) @binding(1) var<uniform> params: DispatchParams;
@group(0) @binding(2) var<storage, read_write> histogram: array<atomic<u32>>;
@group(0) @binding(3) var palette_texture: texture_1d<f32>;
@group(0) @binding(4) var palette_sampler: sampler;
@group(0) @binding(5) var<storage, read> variation_params: array<VariationParams>;

// Data structures (must match Rust definitions)
struct Transform {
    affine: mat2x2<f32>,
    offset: vec2<f32>,
    g: f32,
    weight: f32,
    variations: array<f32, 24>,
    color: vec3<f32>,
    color_speed: f32,
}

struct DispatchParams {
    num_transforms: u32,
    iterations_per_thread: u32,
    burn_in: u32,
    width: u32,
    height: u32,
    seed: u32,
    color_mode: u32,
    splat_size: f32,
    zoom: f32,
    pan_x: f32,
    pan_y: f32,
    rotation: f32,
    speed_factor: f32,
    camera_pitch: f32,
    camera_yaw: f32,
    projection_type: u32,
    perspective_strength: f32,
    histogram_color_scale: f32,
    // ... padding
}

struct VariationParams {
    params: array<f32, 192>,  // 24 variations × 8 params
}
```

**Critical:** These structs must exactly match Rust `#[repr(C)]` structs in [src/gpu/buffers.rs](../../src/gpu/buffers.rs).

### rng.wgsl - Random Number Generation

**Location:** [shaders/core/rng.wgsl](../../shaders/core/rng.wgsl)

**Algorithm:** PCG (Permuted Congruential Generator)

```wgsl
struct RngState {
    state: u32,
}

fn init_rng(seed: u32) -> RngState {
    var rng: RngState;
    rng.state = seed;
    return rng;
}

fn pcg_u32(rng: ptr<function, RngState>) -> u32 {
    let oldstate = (*rng).state;
    (*rng).state = oldstate * 747796405u + 2891336453u;
    let word = ((oldstate >> ((oldstate >> 28u) + 4u)) ^ oldstate) * 277803737u;
    return (word >> 22u) ^ word;
}

fn pcg_float(rng: ptr<function, RngState>) -> f32 {
    return f32(pcg_u32(rng)) / 4294967296.0;
}

fn random_point_in_circle(rng: ptr<function, RngState>) -> vec2<f32> {
    let r = sqrt(pcg_float(rng));
    let theta = pcg_float(rng) * 6.28318530718;
    return vec2(r * cos(theta), r * sin(theta));
}

fn random_point_in_sphere(rng: ptr<function, RngState>) -> vec3<f32> {
    let r = pow(pcg_float(rng), 1.0 / 3.0);
    let theta = pcg_float(rng) * 6.28318530718;
    let phi = acos(2.0 * pcg_float(rng) - 1.0);
    return vec3(
        r * sin(phi) * cos(theta),
        r * sin(phi) * sin(theta),
        r * cos(phi)
    );
}
```

**Key Features:**
- Fast, lightweight PRNG (no global state)
- Each thread has independent RNG state
- Seeded with `params.seed + thread_id` for deterministic or non-deterministic rendering

### utilities.wgsl - Helper Functions

**Location:** [shaders/core/utilities.wgsl](../../shaders/core/utilities.wgsl)

**Key Functions:**

**Parameter Access:**
```wgsl
fn get_param(xform_id: u32, variation_id: u32, param_slot: u32) -> f32 {
    let idx = variation_id * 8u + param_slot;
    return variation_params[xform_id].params[idx];
}
```

**Point Calculations:**
```wgsl
fn calc_r(p: vec2<f32>) -> f32 {
    return length(p);
}

fn calc_theta(p: vec2<f32>) -> f32 {
    return atan2(p.y, p.x);
}

fn calc_phi(p: vec3<f32>) -> f32 {
    return atan2(p.z, length(p.xy));
}
```

**World to Screen Projection:**
```wgsl
fn world_to_pixel(p: vec2<f32>, params: DispatchParams) -> vec2<u32> {
    // 1. Zoom
    var scaled = p * params.zoom;

    // 2. Rotation
    let cos_r = cos(params.rotation);
    let sin_r = sin(params.rotation);
    let rotated = vec2(
        scaled.x * cos_r - scaled.y * sin_r,
        scaled.x * sin_r + scaled.y * cos_r
    );

    // 3. Pan
    let translated = rotated - vec2(params.pan_x, params.pan_y);

    // 4. Screen mapping
    let aspect = f32(params.width) / f32(params.height);
    let screen_x = translated.x * f32(params.height) * 0.5 + f32(params.width) * 0.5;
    let screen_y = translated.y * f32(params.height) * 0.5 + f32(params.height) * 0.5;

    return vec2<u32>(u32(screen_x), u32(screen_y));
}
```

**3D Camera Rotation (3D mode only):**
```wgsl
fn rotate_camera(p: vec3<f32>, pitch: f32, yaw: f32) -> vec3<f32> {
    // Yaw rotation (Y-axis)
    let cos_yaw = cos(yaw);
    let sin_yaw = sin(yaw);
    let p_yaw = vec3(
        p.x * cos_yaw - p.z * sin_yaw,
        p.y,
        p.x * sin_yaw + p.z * cos_yaw
    );

    // Pitch rotation (X-axis)
    let cos_pitch = cos(pitch);
    let sin_pitch = sin(pitch);
    return vec3(
        p_yaw.x,
        p_yaw.y * cos_pitch - p_yaw.z * sin_pitch,
        p_yaw.y * sin_pitch + p_yaw.z * cos_pitch
    );
}
```

### variations_2d.wgsl - 2D Variation Functions

**Location:** [shaders/core/variations_2d.wgsl](../../shaders/core/variations_2d.wgsl)

**Contains:** All 2D variations (indices 0-15) plus parameterized 2D (24-25)

**Function Signatures:**
```wgsl
// Basic variations (no extra parameters)
fn variation_linear(p: vec2<f32>) -> vec2<f32> { return p; }
fn variation_sinusoidal(p: vec2<f32>) -> vec2<f32> { return vec2(sin(p.x), sin(p.y)); }
fn variation_spherical(p: vec2<f32>) -> vec2<f32> { return p / (dot(p, p) + 1e-6); }

// Variations needing RNG
fn variation_julia(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    let r = sqrt(calc_r(p));
    let theta = calc_theta(p) * 0.5 + (pcg_float(rng) < 0.5 ? 0.0 : 3.14159265359);
    return vec2(r * cos(theta), r * sin(theta));
}

// Parameterized variations
fn variation_julian(p: vec2<f32>, xform_id: u32) -> vec2<f32> {
    let power = get_param(xform_id, 24u, 0u);  // Parameter 0
    let dist = get_param(xform_id, 24u, 1u);   // Parameter 1
    // ... use parameters
}

fn variation_blob(p: vec2<f32>, xform_id: u32) -> vec2<f32> {
    let high = get_param(xform_id, 25u, 0u);
    let low = get_param(xform_id, 25u, 1u);
    let waves = get_param(xform_id, 25u, 2u);
    // ... use parameters
}
```

**Note:** 2D shaders only include variations 0-15 and 24-25 (ignores 3D variations 16-23).

### variations_3d.wgsl - All Variations Including 3D

**Location:** [shaders/core/variations_3d.wgsl](../../shaders/core/variations_3d.wgsl)

**Contains:** All 2D variations (0-15, 24-25) PLUS 3D-specific variations (16-23)

**2D Variations in 3D Mode:**
```wgsl
// Pass Z through unchanged
fn variation_linear(p: vec3<f32>) -> vec3<f32> {
    return p;  // Identity for 3D
}

fn variation_sinusoidal(p: vec3<f32>) -> vec3<f32> {
    return vec3(sin(p.x), sin(p.y), p.z);  // Z unchanged
}

fn variation_spherical(p: vec3<f32>) -> vec3<f32> {
    let r2 = p.x * p.x + p.y * p.y + 1e-6;
    return vec3(p.x / r2, p.y / r2, p.z);  // Only XY affected
}
```

**Z-Only 3D Variations:**
```wgsl
// Zcone: Z = distance from origin
fn variation_zcone(p: vec3<f32>) -> vec3<f32> {
    let r = sqrt(p.x * p.x + p.y * p.y);
    return vec3(p.x, p.y, r);
}

// Flatten: Compress Z toward zero
fn variation_flatten(p: vec3<f32>) -> vec3<f32> {
    return vec3(p.x, p.y, p.z * 0.5);
}

// ZScale: Scale Z up or down
fn variation_zscale(p: vec3<f32>, xform_id: u32) -> vec3<f32> {
    let scale = get_param(xform_id, 23u, 0u);
    return vec3(p.x, p.y, p.z * scale);
}
```

**Full 3D Variations:**
```wgsl
// Hemisphere: Project onto sphere surface
fn variation_hemisphere(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    let r = sqrt(p.x * p.x + p.y * p.y + p.z * p.z);
    if (r < 1e-6) { return p; }

    let normalized = p / r;
    return normalized * (1.0 + pcg_float(rng) * 0.1);
}
```

**Rotation Variations:**
```wgsl
// PreRotateY: Rotate around Y before other variations
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

// PostRotateX: Rotate around X after other variations
fn variation_post_rotate_x(p: vec3<f32>, xform_id: u32) -> vec3<f32> {
    let angle = get_param(xform_id, 21u, 0u);
    let cos_a = cos(angle);
    let sin_a = sin(angle);
    return vec3(
        p.x,
        p.y * cos_a - p.z * sin_a,
        p.y * sin_a + p.z * cos_a
    );
}
```

### main_2d.wgsl - 2D Entry Point

**Location:** [shaders/core/main_2d.wgsl](../../shaders/core/main_2d.wgsl)

**Compute Shader Entry:**
```wgsl
@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let thread_id = global_id.y * 8u + global_id.x;

    // 1. Initialize RNG
    var rng = init_rng(params.seed + thread_id);

    // 2. Random starting point
    var p = random_point_in_circle(&rng);
    var color_index = pcg_float(&rng);

    // 3. Burn-in
    for (var i = 0u; i < params.burn_in; i++) {
        let xform_id = select_transform(&rng);
        p = apply_transform(p, xform_id, &rng, &color_index);
    }

    // 4. Accumulation loop
    for (var i = 0u; i < params.iterations_per_thread; i++) {
        let xform_id = select_transform(&rng);
        p = apply_transform(p, xform_id, &rng, &color_index);

        // Project and write to histogram
        let screen_pos = world_to_pixel(p, params);
        if (in_bounds(screen_pos, params)) {
            let color = get_color(color_index, xform_id);
            write_to_histogram(screen_pos, color);
        }
    }
}

fn apply_transform(
    p: vec2<f32>,
    xform_id: u32,
    rng: ptr<function, RngState>,
    color_index: ptr<function, f32>
) -> vec2<f32> {
    let xform = transforms[xform_id];

    // Affine
    let x = xform.affine[0][0] * p.x + xform.affine[0][1] * p.y + xform.offset.x;
    let y = xform.affine[1][0] * p.x + xform.affine[1][1] * p.y + xform.offset.y;
    let p_affine = vec2(x, y);

    // Variations (DYNAMICALLY GENERATED - see ShaderBuilder)
    let p_varied = apply_variations(p_affine, xform_id, rng);

    // Color update
    *color_index = *color_index * (1.0 - xform.color_speed) + xform.color_speed;

    return p_varied;
}

// This function is generated by ShaderBuilder!
// fn apply_variations(p: vec2<f32>, xform_id: u32, rng: ptr<function, RngState>) -> vec2<f32>
```

### main_3d.wgsl - 3D Entry Point

**Location:** [shaders/core/main_3d.wgsl](../../shaders/core/main_3d.wgsl)

**Differences from 2D:**
- Uses `vec3<f32>` for points
- `random_point_in_sphere()` instead of circle
- Affine includes Z offset: `p.z += xform.g`
- Camera rotation before projection
- Perspective or orthographic projection

```wgsl
fn apply_transform_3d(
    p: vec3<f32>,
    xform_id: u32,
    rng: ptr<function, RngState>,
    color_index: ptr<function, f32>
) -> vec3<f32> {
    let xform = transforms[xform_id];

    // Affine (2D XY + Z offset)
    let x = xform.affine[0][0] * p.x + xform.affine[0][1] * p.y + xform.offset.x;
    let y = xform.affine[1][0] * p.x + xform.affine[1][1] * p.y + xform.offset.y;
    let z = p.z + xform.g;
    let p_affine = vec3(x, y, z);

    // Variations (3D version)
    let p_varied = apply_variations_3d(p_affine, xform_id, rng);

    // Color update (same as 2D)
    *color_index = *color_index * (1.0 - xform.color_speed) + xform.color_speed;

    return p_varied;
}

fn world_to_pixel_3d(p: vec3<f32>, params: DispatchParams) -> vec2<u32> {
    // 1. Camera rotation
    let p_rotated = rotate_camera(p, params.camera_pitch, params.camera_yaw);

    // 2. Projection
    var p_2d: vec2<f32>;
    if (params.projection_type == 0u) {
        // Orthographic: ignore Z
        p_2d = p_rotated.xy;
    } else {
        // Perspective: divide by Z
        let depth_factor = 1.0 + p_rotated.z * params.perspective_strength;
        p_2d = p_rotated.xy / depth_factor;
    }

    // 3. View transform (same as 2D)
    return world_to_pixel(p_2d, params);
}
```

---

## Dynamic Shader Compilation (ShaderBuilder)

### Why Dynamic Compilation?

**Problem:** With 50 variation slots (26 core + 24 plugins), compiling all possible variations into every shader would:
- Generate massive shaders (thousands of lines)
- Waste GPU registers on unused variations
- Slow compilation and execution

**Solution:** Generate shaders at runtime with **only active variations**.

### ShaderBuilder Architecture

**Location:** [src/shader_builder_v2.rs](../../src/shader_builder_v2.rs)

```rust
pub struct ShaderBuilder {
    active_variations: HashSet<String>,  // e.g., {"linear", "sinusoidal", "zcone"}
    registry: &'static VariationRegistry,
}

impl ShaderBuilder {
    pub fn build_2d_shader(&self) -> String {
        let mut shader = String::new();

        // 1. Include modular components
        shader.push_str(include_str!("../shaders/core/header.wgsl"));
        shader.push_str(include_str!("../shaders/core/rng.wgsl"));
        shader.push_str(include_str!("../shaders/core/variations_2d.wgsl"));

        // 2. Generate apply_variations() function
        shader.push_str(&self.generate_apply_variations_2d());

        // 3. Include utilities and main
        shader.push_str(include_str!("../shaders/core/utilities.wgsl"));
        shader.push_str(include_str!("../shaders/core/main_2d.wgsl"));

        shader
    }

    fn generate_apply_variations_2d(&self) -> String {
        let mut code = String::from("fn apply_variations(\n");
        code.push_str("    p: vec2<f32>,\n");
        code.push_str("    xform_id: u32,\n");
        code.push_str("    rng: ptr<function, RngState>\n");
        code.push_str(") -> vec2<f32> {\n");
        code.push_str("    var result = vec2(0.0, 0.0);\n");
        code.push_str("    let xform = transforms[xform_id];\n\n");

        // Generate code only for active variations
        for var_name in &self.active_variations {
            let info = self.registry.get(var_name);
            let idx = info.shader_index;

            code.push_str(&format!("    if (xform.variations[{}] != 0.0) {{\n", idx));

            // Determine function signature
            let needs_params = !info.parameters.is_empty();
            let needs_rng = info.needs_rng;

            let call = match (needs_params, needs_rng) {
                (false, false) => format!("variation_{}(p)", var_name),
                (false, true)  => format!("variation_{}(p, rng)", var_name),
                (true, false)  => format!("variation_{}(p, xform_id)", var_name),
                (true, true)   => format!("variation_{}(p, xform_id, rng)", var_name),
            };

            code.push_str(&format!("        result += xform.variations[{}] * {};\n", idx, call));
            code.push_str("    }\n");
        }

        code.push_str("\n    return result;\n");
        code.push_str("}\n");

        code
    }
}
```

### Generated Code Example

**Input:** Active variations = `{"linear", "sinusoidal", "spherical"}`

**Generated `apply_variations()` function:**
```wgsl
fn apply_variations(
    p: vec2<f32>,
    xform_id: u32,
    rng: ptr<function, RngState>
) -> vec2<f32> {
    var result = vec2(0.0, 0.0);
    let xform = transforms[xform_id];

    if (xform.variations[0] != 0.0) {
        result += xform.variations[0] * variation_linear(p);
    }
    if (xform.variations[1] != 0.0) {
        result += xform.variations[1] * variation_sinusoidal(p);
    }
    if (xform.variations[2] != 0.0) {
        result += xform.variations[2] * variation_spherical(p);
    }

    return result;
}
```

**Result:** Shader only includes 3 variations instead of all 50.

---

## Other Shaders

### accumulate.wgsl - Progressive Refinement

**Location:** [shaders/accumulate.wgsl](../../shaders/accumulate.wgsl)

**Purpose:** Blend new samples from histogram with previous accumulation.

**Workgroup Size:** `@workgroup_size(16, 16)` (one thread per pixel)

**Key Operations:**
1. Read histogram (4× u32 per pixel)
2. Decode to f32 RGB + density
3. Read previous accumulation
4. Apply accumulation controls (smoothing, compression, iteration limiting)
5. Exponential moving average blend
6. Write to output texture
7. Clear histogram for next frame

**See [RENDERER.md](RENDERER.md)** for detailed accumulate pass documentation.

### tonemap.wgsl - Display Rendering

**Location:** [shaders/tonemap.wgsl](../../shaders/tonemap.wgsl)

**Purpose:** Convert HDR accumulation to displayable LDR image.

**Pipeline Type:** Render pipeline (not compute)

**Vertex Shader:**
```wgsl
@vertex
fn vs_main(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    // Fullscreen triangle (no vertex buffer needed)
    let x = f32((vertex_index << 1u) & 2u) - 1.0;
    let y = f32(vertex_index & 2u) - 1.0;

    var output: VertexOutput;
    output.position = vec4(x, y, 0.0, 1.0);
    output.tex_coord = vec2((x + 1.0) * 0.5, (1.0 - y) * 0.5);
    return output;
}
```

**Fragment Shader:**
```wgsl
@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let accum = textureSample(accumulation, sampler_linear, input.tex_coord);

    // Tone mapping (logarithmic or linear)
    var color = apply_tonemap(accum.rgb, params);

    // Speed color mode: lookup palette
    if (params.color_mode == 2u) {
        color = textureSample(palette, sampler_linear, accum.r).rgb;
    }

    // Gamma correction
    color = pow(color, vec3(1.0 / params.gamma));

    // Alpha blending with background
    let alpha = accum.a * params.density_scale;
    color = mix(params.background_color, color, alpha);

    return vec4(color, 1.0);
}
```

---

## Common Shader Modification Tasks

| Task | Files to Modify |
|------|-----------------|
| Add new 2D variation | [variations_2d.wgsl](../../shaders/core/variations_2d.wgsl), [variations_3d.wgsl](../../shaders/core/variations_3d.wgsl), [variations/mod.rs](../../src/variations/mod.rs) |
| Add new 3D variation | [variations_3d.wgsl](../../shaders/core/variations_3d.wgsl), [variations/mod.rs](../../src/variations/mod.rs) |
| Change affine algorithm | [main_2d.wgsl](../../shaders/core/main_2d.wgsl) `apply_transform()`, [main_3d.wgsl](../../shaders/core/main_3d.wgsl) `apply_transform_3d()` |
| Modify ShaderBuilder | [shader_builder_v2.rs](../../src/shader_builder_v2.rs) |
| Add new shader component | Create new .wgsl file in [shaders/core/](../../shaders/core/), include in ShaderBuilder |
| Change tone mapping | [tonemap.wgsl](../../shaders/tonemap.wgsl) fragment shader |
| Modify accumulation | [accumulate.wgsl](../../shaders/accumulate.wgsl) |
| Add shader parameter | [header.wgsl](../../shaders/core/header.wgsl) structs, [buffers.rs](../../src/gpu/buffers.rs) Rust structs |

---

**Last Updated:** 2025-10-28
**Related Documentation:**
- [ARCHITECTURE.md](../ARCHITECTURE.md) - Overall system design
- [TRANSFORMS.md](TRANSFORMS.md) - Flame algorithm
- [BUFFERS.md](BUFFERS.md) - GPU data structures
- [VARIATIONS.md](VARIATIONS.md) - Variation functions *(coming soon)*
- [RENDERER.md](RENDERER.md) - Rendering pipeline
