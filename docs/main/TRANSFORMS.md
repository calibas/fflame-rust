# Transform and Flame Algorithm

**Overview:** Complete reference for the fractal flame algorithm, transform structure, and IFS (Iterated Function System) implementation.

**See also:**
- [ARCHITECTURE.md](../ARCHITECTURE.md) - Overall system design
- [BUFFERS.md](BUFFERS.md) - GpuTransform data structure
- [SHADERS.md](SHADERS.md) - Shader implementation *(coming soon)*
- [VARIATIONS.md](VARIATIONS.md) - Variation functions *(coming soon)*

**Code locations:**
- [src/scene/transforms.rs](../../src/scene/transforms.rs) - Transform struct and Flame algorithm
- [shaders/core/main_2d.wgsl](../../shaders/core/main_2d.wgsl) - 2D GPU implementation
- [shaders/core/main_3d.wgsl](../../shaders/core/main_3d.wgsl) - 3D GPU implementation

---

## What is a Fractal Flame?

A fractal flame is an **Iterated Function System (IFS)** that generates intricate patterns through:
1. **Random point iteration** - Start with random point, repeatedly transform it
2. **Weighted transform selection** - Choose transforms probabilistically
3. **Affine transformation** - Linear 2D transform (rotation, scale, translation)
4. **Variation blending** - Apply nonlinear "variation" functions (sin, spherical, swirl, etc.)
5. **Color blending** - Accumulate colors based on transform selection
6. **Histogram accumulation** - Plot point density across the canvas

The result: Beautiful, organic-looking fractals with infinite detail.

**Key Insight:** Instead of plotting a mathematical function, we plot the *attractor* of a chaotic dynamical system.

---

## Transform Structure

### Rust Definition

**Location:** [src/scene/transforms.rs](../../src/scene/transforms.rs)

```rust
pub struct Transform {
    // Affine transformation (2D matrix + translation)
    pub a: f32, pub b: f32,  // First row: affects x
    pub c: f32, pub d: f32,  // Second row: affects y
    pub e: f32, pub f: f32,  // Translation: [e, f]

    // Z offset (3D mode only)
    pub g: f32,              // Z translation

    // Transform selection probability
    pub weight: f32,         // Probability weight (normalized to sum=1.0)

    // Variation weights (26 core + 24 plugin slots)
    pub variations: HashMap<String, f32>,  // Name → weight

    // Variation parameters (for parameterized variations)
    pub variation_params: HashMap<String, HashMap<String, f32>>,  // Variation → (param → value)

    // Color
    pub color: [f32; 3],     // RGB [0.0-1.0]
    pub color_speed: f32,    // Color blend rate [0.0-1.0]
}
```

**Key Points:**
- Affine matrix stored as 6 scalars (a, b, c, d, e, f) instead of 2×3 matrix
- Variations stored as HashMap (flexible, supports plugins)
- GPU upload converts HashMap to fixed-size array (indices 0-49)

### Affine Transformation

The affine transform is a 2D linear transformation plus translation:

```
[x']   [a  b] [x]   [e]
[y'] = [c  d] [y] + [f]

// Expanded:
x' = a*x + b*y + e
y' = c*x + d*y + f
```

**Special Cases:**
- **Identity**: `a=1, d=1, b=c=e=f=0` (no change)
- **Translation**: `a=1, d=1, b=c=0, e≠0, f≠0` (shift only)
- **Scale**: `a=s, d=s, b=c=e=f=0` (uniform scale by s)
- **Rotation**: `a=cos(θ), b=-sin(θ), c=sin(θ), d=cos(θ), e=f=0`

**Visual Interpretation:**
- The affine transform maps the unit triangle `[(0,0), (1,0), (0.5, 0.866)]` to a new triangle
- Triangle editor in UI visualizes this mapping
- Dragging triangle vertices updates the affine coefficients

### 3D Extension

In 3D mode, the affine transform includes Z offset:

```
[x']   [a  b] [x]   [e]
[y'] = [c  d] [y] + [f]
[z']   [0  0] [z]   [g]

// Expanded:
x' = a*x + b*y + e
y' = c*x + d*y + f
z' = z + g  // Simple Z offset (no rotation in affine)
```

**Note:** Z rotation happens in variation functions (PreRotateY, PostRotateY, etc.)

---

## Flame Algorithm

### High-Level Overview

```
1. Start with random point p
2. Burn-in: Iterate N times without plotting (settle attractor)
3. For each iteration:
   a. Select transform T_i randomly (weighted by T_i.weight)
   b. Apply affine: p' = affine(p)
   c. Apply variations: p'' = blend_variations(p')
   d. Update color based on color mode
   e. Plot p'' to histogram (increment density + accumulate color)
   f. p = p'' (iterate)
4. Repeat millions of times
```

**Result:** Points cluster around the attractor, creating fractal structure.

**Key Insight: One Iteration = One Point Drawn**

Each iteration through the algorithm (steps 3a-3f) produces **one point** that is plotted to the histogram. If a thread performs 256 iterations, it draws approximately 236 points (256 total iterations minus 20 burn-in iterations that are discarded).

**The point "walks" through transform space:**
- Iteration 1: p₀ → Transform → p₁ (drawn)
- Iteration 2: p₁ → Transform → p₂ (drawn)
- Iteration 3: p₂ → Transform → p₃ (drawn)
- ...
- Iteration 256: p₂₅₅ → Transform → p₂₅₆ (drawn)

The same point variable is reused throughout the loop (step 3f: `p = p''`), creating a chaotic trajectory through the fractal's attractor. Each position along this trajectory is plotted, building up the final image through density accumulation.

### CPU Reference Implementation (2D only)

**Location:** [src/scene/transforms.rs](../../src/scene/transforms.rs) - `Flame::iterate()`

```rust
pub fn iterate(&self, p: [f32; 2], color_index: &mut f32) -> [f32; 2] {
    // 1. Select transform randomly (weighted)
    let xform = self.select_random_transform();

    // 2. Apply affine transformation
    let x = xform.a * p[0] + xform.b * p[1] + xform.e;
    let y = xform.c * p[0] + xform.d * p[1] + xform.f;
    let p_affine = [x, y];

    // 3. Apply variation blending
    let mut result = [0.0, 0.0];
    for (var_name, weight) in &xform.variations {
        if *weight == 0.0 { continue; }
        let varied = apply_variation(var_name, p_affine, xform);
        result[0] += weight * varied[0];
        result[1] += weight * varied[1];
    }

    // 4. Update color (Transform mode example)
    *color_index = *color_index * (1.0 - xform.color_speed) + xform.color_speed;

    result
}
```

**Key Details:**
- Transform selection uses cumulative probability (binary search or linear scan)
- Variations are applied to post-affine point, then blended by weight
- Color evolves via exponential moving average
- This is reference only; GPU implementation is much faster

### GPU Implementation (2D Mode)

**Location:** [shaders/core/main_2d.wgsl](../../shaders/core/main_2d.wgsl)

```wgsl
@compute @workgroup_size(8, 8)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let thread_id = global_id.y * 8u + global_id.x;

    // 1. Initialize RNG with unique seed
    var rng = init_rng(params.seed + thread_id);

    // 2. Random starting point
    var p = random_point_in_circle(&rng);
    var color_index = pcg_float(&rng);

    // 3. Burn-in (settle attractor)
    for (var i = 0u; i < params.burn_in; i++) {
        let xform_id = select_transform(&rng);
        p = apply_transform(p, xform_id, &rng, &color_index);
    }

    // 4. Accumulation iterations
    for (var i = 0u; i < params.iterations_per_thread; i++) {
        let xform_id = select_transform(&rng);
        p = apply_transform(p, xform_id, &rng, &color_index);

        // Project to screen space
        let screen_pos = world_to_pixel(p, params);

        if (in_bounds(screen_pos, params)) {
            // Get color based on color mode
            let color = get_color(color_index, xform_id);

            // Accumulate to histogram (atomic u32)
            let pixel_idx = screen_pos.y * params.width + screen_pos.x;
            let base_idx = pixel_idx * 4u;
            let scale = params.histogram_color_scale;

            atomicAdd(&histogram[base_idx + 0u], u32(color.r * scale));
            atomicAdd(&histogram[base_idx + 1u], u32(color.g * scale));
            atomicAdd(&histogram[base_idx + 2u], u32(color.b * scale));
            atomicAdd(&histogram[base_idx + 3u], u32(scale));
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

    // Affine transformation
    let x = xform.affine[0][0] * p.x + xform.affine[0][1] * p.y + xform.offset.x;
    let y = xform.affine[1][0] * p.x + xform.affine[1][1] * p.y + xform.offset.y;
    let p_affine = vec2(x, y);

    // Variation blending (dynamically generated by ShaderBuilder)
    let p_varied = apply_variations(p_affine, xform_id, rng);

    // Color update
    *color_index = *color_index * (1.0 - xform.color_speed) + xform.color_speed;

    return p_varied;
}
```

**Performance:**
- Default: 128 workgroups × 64 threads × 256 iterations = 2M iterations/frame
- At 60 FPS: 120M iterations/second
- Each thread is independent (fully parallel)

**Thread Isolation and Parallelism:**

Each of the **8,192 threads** (128 workgroups × 64 threads) operates **completely independently** with no communication during iteration:

**Per Thread:**
- **Own random seed:** `init_rng(params.seed + thread_id)` - ensures unique random sequence
- **Own starting point:** Random position in [-1, 1] range, different for each thread
- **Own iteration chain:** 256 iterations producing ~236 drawn points (after 20 burn-in)
- **Own point trajectory:** The point variable `p` is local to the thread, reused each iteration

**No Inter-Thread Communication:**
- Thread A's point never influences Thread B's point
- Threads do not share intermediate results
- Each thread follows its own chaotic path through transform space

**Only Synchronization Point:**
- Atomic writes to shared histogram buffer: `atomicAdd(&histogram[...], ...)`
- Multiple threads can hit the same pixel - atomics ensure thread-safe accumulation
- This is where all threads' work combines to form the final image

**Total Points Per Frame:**
- 8,192 threads × 236 points/thread ≈ **1.9 million points drawn**
- Each point increments density at one pixel location
- High-density areas (many points) become bright, low-density areas stay dark

**Why This Works:**
Despite each thread starting randomly and following an independent chaotic path, they all converge to draw the **same fractal structure**. This is the mathematical property of the IFS (Iterated Function System) - the attractor is the same regardless of starting point.

### GPU Implementation (3D Mode)

**Location:** [shaders/core/main_3d.wgsl](../../shaders/core/main_3d.wgsl)

**Key Differences from 2D:**
```wgsl
// 1. Point is vec3<f32> instead of vec2<f32>
var p = random_point_in_sphere(&rng);  // 3D starting point

// 2. Affine includes Z offset
fn apply_transform_3d(p: vec3<f32>, xform_id: u32, ...) -> vec3<f32> {
    let xform = transforms[xform_id];

    // Affine (2D XY + Z offset)
    let x = xform.affine[0][0] * p.x + xform.affine[0][1] * p.y + xform.offset.x;
    let y = xform.affine[1][0] * p.x + xform.affine[1][1] * p.y + xform.offset.y;
    let z = p.z + xform.g;  // Z offset
    let p_affine = vec3(x, y, z);

    // Variations (includes 3D variations that modify Z)
    let p_varied = apply_variations_3d(p_affine, xform_id, rng);

    return p_varied;
}

// 3. Camera rotation before projection
fn world_to_pixel_3d(p: vec3<f32>, params: GpuParams) -> vec2<u32> {
    // Apply camera rotation (pitch, yaw)
    let p_rotated = rotate_camera(p, params.camera_pitch, params.camera_yaw);

    // Apply projection (orthographic or perspective)
    var p_2d: vec2<f32>;
    if (params.projection_type == 0u) {
        // Orthographic: ignore Z
        p_2d = p_rotated.xy;
    } else {
        // Perspective: divide by Z
        let depth_factor = 1.0 + p_rotated.z * params.perspective_strength;
        p_2d = p_rotated.xy / depth_factor;
    }

    // Apply 2D view transform (zoom, pan, rotation)
    // ... same as 2D mode
}
```

**3D Variation Behavior:**
- **2D variations (0-15)**: Pass Z through unchanged `vec3(new_x, new_y, p.z)`
- **Z-only variations (16, 17, 23)**: Modify `result.z` directly (avoid affecting XY)
- **Full 3D variations (18)**: Modify all axes (Hemisphere)
- **Rotation variations (19-22)**: Apply rotation matrix to full vector

---

## Point Calculations

Helper functions for polar coordinates (used by many variations).

### Cartesian to Polar

```rust
// Distance from origin
pub fn r(p: [f32; 2]) -> f32 {
    (p[0] * p[0] + p[1] * p[1]).sqrt()
}

// Angle (theta) in radians
pub fn theta(p: [f32; 2]) -> f32 {
    p[1].atan2(p[0])
}

// Phi (used in 3D)
pub fn phi(p: [f32; 3]) -> f32 {
    p[2].atan2((p[0] * p[0] + p[1] * p[1]).sqrt())
}
```

**WGSL equivalents** in [shaders/core/utilities.wgsl](../../shaders/core/utilities.wgsl):
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

---

## Render Modes

### 2D Mode (Classic)

**Settings:**
- `render_mode = RenderMode::TwoD`
- Uses `main_2d.wgsl` shader
- Points are `vec2<f32>`
- Projection: Direct 2D screen mapping

**Use Case:** Traditional fractal flames (most presets)

### 3D Mode (Pseudo-3D)

**Settings:**
- `render_mode = RenderMode::ThreeD`
- Uses `main_3d.wgsl` shader
- Points are `vec3<f32>`
- Projection: Orthographic or Perspective

**Projection Types:**
1. **Orthographic** (`ProjectionType::Orthographic`)
   - Ignores Z coordinate (parallel projection)
   - Good for flat 3D structures

2. **Perspective** (`ProjectionType::Perspective { strength }`)
   - Divides XY by `(1 + Z * strength)`
   - Creates depth illusion
   - Strength 2.0-5.0 typical

**Camera Controls:**
- **Pitch** (camera_pitch): X-axis rotation (up/down orbit, -π to π)
- **Yaw** (camera_yaw): Y-axis rotation (left/right orbit, -π to π)

**Creating 3D Fractals:**
1. Enable 3D mode
2. Set projection to Perspective with strength ~3.0
3. Add 3D variations (Zcone, Flatten, Hemisphere, etc.)
4. Use different `g` values per transform for layering
5. Rotate camera to verify 3D structure

---

## Transform Selection

### Weighted Random Selection

Transforms are selected proportionally to their `weight` field.

**Algorithm:**
```rust
// 1. Normalize weights to sum to 1.0
let total_weight: f32 = transforms.iter().map(|t| t.weight).sum();
let normalized: Vec<f32> = transforms.iter()
    .map(|t| t.weight / total_weight)
    .collect();

// 2. Build cumulative distribution
let mut cumulative = vec![0.0];
for w in normalized {
    cumulative.push(cumulative.last().unwrap() + w);
}
// cumulative = [0.0, w0, w0+w1, w0+w1+w2, ..., 1.0]

// 3. Select transform
let r = random(0.0, 1.0);
let idx = cumulative.iter().position(|&x| x > r).unwrap() - 1;
let selected = transforms[idx];
```

**GPU Implementation:**
```wgsl
fn select_transform(rng: ptr<function, RngState>) -> u32 {
    let r = pcg_float(rng);
    var sum = 0.0;
    for (var i = 0u; i < params.num_transforms; i++) {
        sum += transforms[i].weight;
        if (r < sum) {
            return i;
        }
    }
    return params.num_transforms - 1u;  // Fallback
}
```

**Note:** Weights don't need to sum to 1.0 (GPU normalizes implicitly).

---

## Variation Blending

### Additive Blending

Variations are applied to the post-affine point and summed:

```rust
let mut result = [0.0, 0.0];
for (var_name, weight) in &xform.variations {
    let varied = apply_variation(var_name, p_affine);
    result[0] += weight * varied[0];
    result[1] += weight * varied[1];
}
```

**Example:**
```
p_affine = [0.5, 0.3]
variations = { "linear": 0.5, "sinusoidal": 0.5 }

linear(p) = [0.5, 0.3]
sinusoidal(p) = [sin(0.5), sin(0.3)] = [0.479, 0.296]

result = 0.5 * [0.5, 0.3] + 0.5 * [0.479, 0.296]
       = [0.25, 0.15] + [0.240, 0.148]
       = [0.490, 0.298]
```

**GPU Implementation:**

The shader builder generates a `apply_variations()` function with a switch statement for active variations:

```wgsl
fn apply_variations(p: vec2<f32>, xform_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    var result = vec2(0.0, 0.0);
    let xform = transforms[xform_id];

    // Dynamically generated based on active variations
    if (xform.variations[0] != 0.0) {
        result += xform.variations[0] * variation_linear(p);
    }
    if (xform.variations[1] != 0.0) {
        result += xform.variations[1] * variation_sinusoidal(p);
    }
    // ... etc for all active variations

    return result;
}
```

**Note:** Only active variations (weight > 0) are included in the generated shader.

---

## Color Modes

### Transform Color Mode (mode=0)

Colors blend via exponential moving average:

```rust
color_index = color_index * (1.0 - xform.color_speed) + xform.color_speed;
final_color = xform.color;  // RGB from transform
```

**Behavior:**
- Each transform has an associated color
- Color evolves slowly as transforms are selected
- `color_speed` controls blend rate (0.0=slow, 1.0=instant)

### Palette Color Mode (mode=1)

Color looked up from palette texture:

```rust
final_color = palette.sample(color_index);  // color_index ∈ [0, 1]
```

**Behavior:**
- Palette is 1D texture (256 RGB samples)
- `color_index` evolves same as Transform mode
- Allows smooth color gradients across fractal

### Speed Color Mode (mode=2)

Color based on point movement speed:

```rust
let speed = distance(p_after, p_before);
let normalized_speed = speed * speed_factor;
final_color = palette.sample(normalized_speed);
```

**Behavior:**
- Fast-moving points → different colors than slow-moving
- Creates heat map effect
- `speed_factor` controls sensitivity

---

## View Transformation

The view transform maps fractal coordinates to screen pixels.

### World to Screen Mapping

```rust
// 1. Apply zoom
let scaled_x = p.x * zoom;
let scaled_y = p.y * zoom;

// 2. Apply 2D rotation
let cos_r = rotation.cos();
let sin_r = rotation.sin();
let rotated_x = scaled_x * cos_r - scaled_y * sin_r;
let rotated_y = scaled_x * sin_r + scaled_y * cos_r;

// 3. Apply pan (fractal space)
let translated_x = rotated_x - pan_x;
let translated_y = rotated_y - pan_y;

// 4. Map to screen pixels (center at [width/2, height/2])
let screen_x = (translated_x * height / 2.0 + width / 2.0) as u32;
let screen_y = (translated_y * height / 2.0 + height / 2.0) as u32;
```

**WGSL implementation** in [shaders/core/utilities.wgsl](../../shaders/core/utilities.wgsl):
```wgsl
fn world_to_pixel(p: vec2<f32>, params: GpuParams) -> vec2<u32> {
    // Zoom
    var scaled = p * params.zoom;

    // Rotation
    let cos_r = cos(params.rotation);
    let sin_r = sin(params.rotation);
    let rotated = vec2(
        scaled.x * cos_r - scaled.y * sin_r,
        scaled.x * sin_r + scaled.y * cos_r
    );

    // Pan
    let translated = rotated - vec2(params.pan_x, params.pan_y);

    // Screen mapping
    let aspect = f32(params.width) / f32(params.height);
    let screen_x = (translated.x * f32(params.height) * 0.5 + f32(params.width) * 0.5);
    let screen_y = (translated.y * f32(params.height) * 0.5 + f32(params.height) * 0.5);

    return vec2<u32>(u32(screen_x), u32(screen_y));
}
```

---

## Common Flame Modification Tasks

| Task | Files to Modify |
|------|-----------------|
| Add new variation | See [VARIATIONS.md](VARIATIONS.md) *(coming soon)* |
| Change affine algorithm | [transforms.rs](../../src/scene/transforms.rs), shaders (main_2d/3d) |
| Add transform parameter | [transforms.rs](../../src/scene/transforms.rs), [buffers.rs](../../src/gpu/buffers.rs) |
| Change color blending | [transforms.rs](../../src/scene/transforms.rs), shaders (main_2d/3d) |
| Modify world-to-screen | [utilities.wgsl](../../shaders/core/utilities.wgsl) |
| Add new color mode | [transforms.rs](../../src/scene/transforms.rs), shaders, [ui/mod.rs](../../src/ui/mod.rs) |
| Change transform selection | [transforms.rs](../../src/scene/transforms.rs), shaders (select_transform) |

---

**Last Updated:** 2025-10-28
**Related Documentation:**
- [ARCHITECTURE.md](../ARCHITECTURE.md) - Overall system design
- [BUFFERS.md](BUFFERS.md) - GpuTransform structure details
- [VARIATIONS.md](VARIATIONS.md) - Variation function reference *(coming soon)*
- [SHADERS.md](SHADERS.md) - Shader implementation details *(coming soon)*
