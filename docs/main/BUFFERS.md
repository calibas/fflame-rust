# GPU Buffer Architecture

**Overview:** Complete reference for GPU buffer layouts, bind groups, and data structures used in the fractal flame renderer.

**See also:**
- [ARCHITECTURE.md](../ARCHITECTURE.md) - Overall system design
- [RENDERER.md](RENDERER.md) - Rendering pipeline *(coming soon)*
- [SHADERS.md](SHADERS.md) - Shader system *(coming soon)*
- [PIPELINE.md](PIPELINE.md) - Pipeline details *(coming soon)*

**Code locations:**
- [src/gpu/buffers.rs](../../src/gpu/buffers.rs) - Buffer creation and management
- [shaders/core/header.wgsl](../../shaders/core/header.wgsl) - WGSL struct definitions

---

## Bind Group Layouts

The renderer uses three different bind group configurations, one for each pipeline stage.

### Bind Group 0 (Compute Pass)

Used by the trajectory shaders (main_2d.wgsl / main_3d.wgsl) to generate fractal samples.

```wgsl
@group(0) @binding(0) - transforms: array<GpuTransform>           (storage buffer, read)
@group(0) @binding(1) - params: GpuParams                         (uniform buffer, read)
@group(0) @binding(2) - histogram: array<atomic<u32>>             (storage buffer, read_write)
@group(0) @binding(3) - palette_texture: texture_1d<f32>          (texture, sample)
@group(0) @binding(4) - palette_sampler: sampler                  (sampler)
@group(0) @binding(5) - variation_params: array<VariationParams>  (storage buffer, read)
```

**Purpose:**
- **transforms** - Array of up to 32 transforms (affine + variations + colors)
- **params** - Global render parameters (zoom, pan, rotation, etc.)
- **histogram** - Atomic accumulation buffer for color data (4× u32 per pixel)
- **palette_texture** - 1D color gradient (256 samples)
- **palette_sampler** - Linear sampling for smooth color interpolation
- **variation_params** - Parameter values for parameterized variations (50 variations × 8 params)

### Bind Group 0 (Accumulate Pass)

Used by accumulate.wgsl to blend new samples with previous accumulation.

```wgsl
@group(0) @binding(0) - prev_accumulation: texture_2d<f32>    (texture, sample)
@group(0) @binding(1) - histogram: array<u32>                 (storage buffer, read)
@group(0) @binding(2) - output_texture: texture_storage_2d    (texture, write)
@group(0) @binding(3) - params: AccumulateParams              (uniform buffer, read)
@group(0) @binding(4) - iteration_counts: array<u32>          (storage buffer, read)
```

**Purpose:**
- **prev_accumulation** - Previous frame's accumulated result (ping-pong buffer)
- **histogram** - Read new samples from compute pass (non-atomic read)
- **output_texture** - Write blended result here (becomes next prev_accumulation)
- **params** - Blend control parameters (rate, smoothing, compression)
- **iteration_counts** - Per-pixel iteration tracking for convergence limiting

**Ping-Pong Behavior:**
- Frame N: Read from texture A, write to texture B
- Frame N+1: Read from texture B, write to texture A
- Swapped via buffer index after each accumulate pass

### Bind Group 0 (Tonemap Pass)

Used by tonemap.wgsl to display the accumulated result on screen.

```wgsl
@group(0) @binding(0) - accumulation: texture_2d<f32>    (texture, sample)
@group(0) @binding(1) - palette: texture_1d<f32>         (texture, sample)
@group(0) @binding(2) - sampler_linear: sampler          (sampler)
@group(0) @binding(3) - params: TonemapParams            (uniform buffer, read)
```

**Purpose:**
- **accumulation** - Current accumulated result from accumulate pass
- **palette** - 1D color gradient for Speed color mode
- **sampler_linear** - Linear sampling for both accumulation and palette
- **params** - Tone mapping parameters (exposure, gamma, background color)

---

## GPU Data Structures

All GPU structs must follow **std140 (uniform)** or **std430 (storage)** layout rules for cross-platform compatibility.

### GpuTransform (Storage Buffer, std430 layout)

**Location:** [src/gpu/buffers.rs](../../src/gpu/buffers.rs) - `GpuTransform` struct

```rust
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuTransform {
    // Affine transformation (8 bytes)
    pub affine: [[f32; 2]; 2],    // 2x2 matrix [a,b; c,d]

    // Translation and Z offset (12 bytes)
    pub offset: [f32; 2],          // [e, f]
    pub g: f32,                    // Z offset (3D mode only)

    // Transform weight (4 bytes)
    pub weight: f32,               // Selection probability

    // Variation weights (96 bytes: 24 variations × 4 bytes)
    pub variations: [f32; 24],     // Indices 0-15: 2D, 16-23: 3D

    // Color (16 bytes with padding)
    pub color: [f32; 3],           // RGB color
    pub color_speed: f32,          // Color blend factor
}
```

**Total Size:** 136 bytes per transform
**Max Count:** 32 transforms (4,352 bytes buffer)

**Field Details:**
- **affine** - 2D linear transformation matrix
  - `[a, b]` - First row (affects x coordinate)
  - `[c, d]` - Second row (affects y coordinate)
  - Formula: `[x', y'] = [[a,b],[c,d]] × [x,y] + [e,f]`

- **offset** - Translation vector `[e, f]`

- **g** - Z-axis offset (only used in 3D mode)

- **weight** - Probability of selecting this transform (normalized to sum=1.0)

- **variations** - Blend weights for 24 variation functions
  - 0-15: Basic 2D and Advanced 2D variations
  - 16-17, 23: Z-only 3D variations (Zcone, Flatten, ZScale)
  - 18: Full 3D variation (Hemisphere)
  - 19-22: 3D rotation variations (PreRotate/PostRotate X/Y)
  - Sum doesn't need to equal 1.0 (additive blending)

- **color** - RGB color for Transform color mode `[r, g, b]` (0.0-1.0)

- **color_speed** - Blend rate between previous and current color (0.0-1.0)

**Alignment Notes:**
- std430 packs arrays tightly (no padding between array elements)
- vec3 still requires 16-byte alignment after large arrays
- Explicit padding not needed in this struct (fields naturally align)

### GpuParams (Uniform Buffer, std140 layout)

**Location:** [src/gpu/buffers.rs](../../src/gpu/buffers.rs) - `GpuParams` struct

```rust
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuParams {
    // Core render settings
    pub num_transforms: u32,           // Active transform count (1-32)
    pub iterations_per_thread: u32,    // Samples per GPU thread
    pub burn_in: u32,                  // Initial iterations to discard
    pub width: u32,                    // Viewport width (pixels)
    pub height: u32,                   // Viewport height (pixels)
    pub seed: u32,                     // Random seed (changes per frame)
    pub color_mode: u32,               // 0=Transform, 1=Palette, 2=Speed
    pub splat_size: f32,               // Unused (legacy field)

    // View transformation
    pub zoom: f32,                     // Zoom level (0.1-10.0)
    pub pan_x: f32,                    // Horizontal pan
    pub pan_y: f32,                    // Vertical pan
    pub rotation: f32,                 // 2D rotation (radians)
    pub speed_factor: f32,             // Speed color mode sensitivity

    // 3D mode fields (Added 2025-10-21)
    pub camera_pitch: f32,             // Camera X-axis rotation (radians)
    pub camera_yaw: f32,               // Camera Y-axis rotation (radians)
    pub projection_type: u32,          // 0=Orthographic, 1=Perspective
    pub perspective_strength: f32,     // Perspective intensity (0.0-10.0)

    // Histogram settings (Added 2025-10-27)
    pub histogram_color_scale: f32,    // U32 encoding scale (default: 100.0)

    // Padding for alignment
    pub _pad0: f32,
    pub _pad1: f32,
    pub _pad2: f32,
}
```

**Total Size:** 88 bytes

**Field Details:**

**Core Settings:**
- **num_transforms** - How many transforms to use (rest are ignored)
- **iterations_per_thread** - Samples generated per GPU thread (default: 256)
- **burn_in** - Initial iterations discarded to settle chaotic trajectory (default: 20)
- **width, height** - Render resolution in pixels
- **seed** - RNG seed (incremented each frame for non-deterministic rendering)
- **color_mode** - Color algorithm selection:
  - 0: Transform color mode (blend transform colors)
  - 1: Palette color mode (lookup by color index)
  - 2: Speed color mode (lookup by point movement speed)

**View Transform:**
- **zoom** - Magnification level (higher = zoomed in)
- **pan_x, pan_y** - Fractal-space translation
- **rotation** - 2D rotation angle (radians)
- **speed_factor** - Multiplier for speed-based color lookup

**3D Settings:**
- **camera_pitch** - Up/down camera rotation around X-axis (radians)
- **camera_yaw** - Left/right camera rotation around Y-axis (radians)
- **projection_type** - Projection mode:
  - 0: Orthographic (flat, ignores Z depth)
  - 1: Perspective (depth-aware, divides by Z)
- **perspective_strength** - How much Z affects perspective (higher = stronger)

**Histogram:**
- **histogram_color_scale** - Scale factor for f32→u32 color encoding (default: 100.0)

### TonemapParams (Uniform Buffer, std140 layout)

**Location:** [src/gpu/buffers.rs](../../src/gpu/buffers.rs) - `TonemapParams` struct

```rust
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TonemapParams {
    pub exposure: f32,              // Brightness multiplier (0.1-10.0)
    pub gamma: f32,                 // Gamma correction (0.5-3.0, default: 2.2)
    pub density_scale: f32,         // Alpha channel multiplier
    pub tonemap_mode: u32,          // 0=Logarithmic, 1=Linear

    pub background_color: [f32; 3], // RGB background (0.0-1.0)
    pub use_curve: u32,             // Apply S-curve adjustment (0=off, 1=on)

    pub tonemap_curve: f32,         // S-curve strength (0.0-10.0)
    pub _pad0: f32,                 // Alignment padding
    pub _pad1: f32,
    pub _pad2: f32,
}
```

**Total Size:** 48 bytes

**Field Details:**
- **exposure** - Pre-tone-mapping brightness adjustment
- **gamma** - Output gamma correction (2.2 for sRGB displays)
- **density_scale** - Multiplier for alpha channel (controls transparency)
- **tonemap_mode** - Tone mapping algorithm:
  - 0: Logarithmic (compresses HDR range, good for bright areas)
  - 1: Linear (no compression, preserves relative brightness)
- **background_color** - Color to blend with via alpha (RGB, 0.0-1.0)
- **use_curve** - Enable optional S-curve adjustment for contrast
- **tonemap_curve** - S-curve intensity (higher = more contrast)

**Tone Mapping Algorithm:**
```wgsl
// Logarithmic mode (default)
let intensity = dot(color.rgb, vec3(0.3, 0.59, 0.11));
let log_intensity = log(1.0 + intensity * exposure);
let scale = log_intensity / (intensity + 1e-6);
color = color * scale;

// Linear mode
color = color * exposure;

// Gamma correction (both modes)
color = pow(color, vec3(1.0 / gamma));

// Background blending
let alpha = density * density_scale;
color = mix(background_color, color, alpha);
```

### AccumulateParams (Uniform Buffer, std140 layout)

**Location:** [src/gpu/buffers.rs](../../src/gpu/buffers.rs) - `AccumulateParams` struct

```rust
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct AccumulateParams {
    pub width: u32,                           // Viewport width
    pub height: u32,                          // Viewport height
    pub blend_factor: f32,                    // Base blend rate (0.0-1.0)
    pub histogram_color_scale: f32,           // Must match compute shader

    pub low_density_smoothing: f32,           // Noise reduction (0.0-1.0)
    pub density_compression_strength: f32,    // Bright area detail (0.0-100.0)
    pub target_iterations_per_pixel: u32,     // Per-pixel limit (0=disabled)
    pub dynamic_blend_mode: u32,              // 0=exponential, 1=fixed rate

    // Padding to 48 bytes (std140 requires 16-byte alignment)
    pub _pad0: f32,
    pub _pad1: f32,
    pub _pad2: f32,
    pub _pad3: f32,
}
```

**Total Size:** 48 bytes

**Field Details:**

**Core Blending:**
- **width, height** - Resolution for pixel indexing
- **blend_factor** - Base blend rate for exponential moving average
  - Exponential mode: `1.0 / total_samples_accumulated`
  - Fixed rate mode: User-specified constant (0.01-1.0)
- **histogram_color_scale** - Must match compute shader value for correct decoding

**Accumulation Controls:**
- **low_density_smoothing** (0.0-1.0)
  - Reduces blend rate in sparse/dark areas to reduce noise
  - 0.0: No smoothing (uniform blend rate)
  - 1.0: Maximum smoothing (very slow accumulation in sparse areas)
  - Formula: `density_factor = pow(density, smoothing)`

- **density_compression_strength** (0.0-100.0)
  - Slows accumulation in bright areas to reveal detail
  - 0.0: No compression (default)
  - 25.0: Gentle (20% rate in bright areas)
  - 50.0: Moderate (2% rate)
  - 100.0: Strong (1% rate)
  - Formula: `compression_factor = 1.0 / (1.0 + density × strength × 0.01)`

- **target_iterations_per_pixel** (0 to 1,000,000)
  - Stops accumulating pixel after N hits (0 = disabled)
  - Prevents over-sampling dense areas while sparse areas catch up
  - Tracked via atomic counters in compute shader (~5% overhead)
  - Gated after initial density to avoid empty spots

- **dynamic_blend_mode**
  - 0: Exponential convergence (blend_factor = 1/N, old default)
  - 1: Fixed rate (blend_factor = constant, smoother at low sample counts)

**Blending Formula:**
```wgsl
// Calculate adjusted blend rate
let density_factor = pow(density, low_density_smoothing);
let compression_factor = 1.0 / (1.0 + density × density_compression_strength × 0.01);
let convergence_gate = (iteration_count < target) ? 1.0 : 0.0;

let adjusted_blend = blend_factor
    × density_factor
    × compression_factor
    × convergence_gate;

// Exponential moving average
let result = mix(prev_color, new_color, adjusted_blend);
```

### GpuVariationParams (Storage Buffer, std430 layout)

**Location:** [src/gpu/buffers.rs](../../src/gpu/buffers.rs) - `GpuVariationParams` struct

```rust
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuVariationParams {
    pub params: [f32; 400],  // 50 variations × 8 params each
}
```

**Total Size:** 1,600 bytes (50 variations × 8 params × 4 bytes)

**Layout:**
```
Variation 0 params: [0..7]
Variation 1 params: [8..15]
Variation 2 params: [16..23]
...
Variation 49 params: [392..399]
```

**Access Pattern (in shader):**
```wgsl
fn get_param(xform_id: u32, variation_id: u32, param_slot: u32) -> f32 {
    let idx = variation_id * 8u + param_slot;
    return variation_params[xform_id].params[idx];
}

// Example: Get JuliaN power parameter
let power = get_param(xform_id, 24u, 0u);  // Variation 24, param slot 0
```

**Purpose:**
- Stores custom parameters for parameterized variations
- Examples:
  - **JuliaN** (variation 24): `power` (int), `distance` (float)
  - **Blob** (variation 25): `high` (float), `low` (float), `waves` (float)
- Most variations use 0-3 params (rest are zeros)
- Maximum 8 params per variation (expandable if needed)

**Storage:**
- One `GpuVariationParams` struct per transform (up to 32)
- Uploaded to GPU as array of structs
- Indexed by `xform_id` in shader

---

## Memory Layout Rules

### std140 (Uniform Buffers)

**Used by:** GpuParams, TonemapParams, AccumulateParams

**Rules:**
- Scalars (f32, u32): 4-byte alignment
- vec2: 8-byte alignment
- vec3: **16-byte alignment** (padded to vec4 size)
- vec4: 16-byte alignment
- Arrays: Elements aligned to 16 bytes (wasteful!)
- Structs: Aligned to largest member

**Why use std140?**
- Universal compatibility (all GPUs support it)
- Simpler memory layout (predictable padding)
- Required for uniform buffers on some platforms

### std430 (Storage Buffers)

**Used by:** GpuTransform, GpuVariationParams, histogram, iteration_counts

**Rules:**
- Scalars: 4-byte alignment
- vec2: 8-byte alignment
- vec3: **16-byte alignment** (still padded!)
- vec4: 16-byte alignment
- Arrays: Elements packed tightly (no padding)
- Structs: Aligned to largest member

**Why use std430?**
- More efficient for large arrays (no per-element padding)
- Required for storage buffers
- vec3 still has 16-byte alignment issue

### Critical Alignment Issue: vec3 After Arrays

**Problem:** vec3 fields after large arrays may become misaligned.

**Example (WRONG):**
```rust
pub struct GpuTransform {
    pub variations: [f32; 24],  // 96 bytes
    pub color: [f32; 3],        // Expects 16-byte alignment, but at offset 96!
}
```

**Solution:** Add explicit padding:
```rust
pub struct GpuTransform {
    pub variations: [f32; 24],  // 96 bytes
    pub _pad: f32,              // Padding to 112 bytes (next 16-byte boundary)
    pub color: [f32; 3],        // Now aligned at offset 112 ✓
}
```

**Or reorder fields:**
```rust
pub struct GpuTransform {
    pub color: [f32; 3],        // Put vec3 first
    pub color_speed: f32,       // Natural padding
    pub variations: [f32; 24],  // Array at end, no alignment issues
}
```

---

## Buffer Update Patterns

### Frame-by-Frame Updates (Hot Path)

**GpuParams** - Updated every frame:
```rust
queue.write_buffer(&params_buffer, 0, bytemuck::bytes_of(&gpu_params));
```

**Changes:**
- `seed` - Incremented each frame for non-deterministic rendering
- View transform (zoom, pan, rotation) - If user drags/zooms
- Camera (pitch, yaw) - If 3D mode navigation

### On State Change (Cold Path)

**GpuTransform** - Updated when flame changes:
```rust
// Write all 32 transform slots (zero padding for unused)
let mut gpu_transforms = [GpuTransform::zeroed(); 32];
for (i, xform) in flame.transforms.iter().enumerate() {
    gpu_transforms[i] = xform.to_gpu();
}
queue.write_buffer(&transform_buffer, 0, bytemuck::cast_slice(&gpu_transforms));
```

**GpuVariationParams** - Updated when variation parameters change:
```rust
let mut variation_params = vec![GpuVariationParams::default(); 32];
for (i, xform) in flame.transforms.iter().enumerate() {
    variation_params[i] = xform.variation_params_to_gpu();
}
queue.write_buffer(&variation_params_buffer, 0, bytemuck::cast_slice(&variation_params));
```

**TonemapParams** - Updated when tone mapping settings change

**AccumulateParams** - Updated when blend settings change

### One-Time Upload (Initialization)

**Palette Texture** - Uploaded at init or when palette changes:
```rust
queue.write_texture(
    palette_texture.as_image_copy(),
    &palette_data,  // 256 RGBA pixels
    wgpu::ImageDataLayout { ... },
    wgpu::Extent3d { width: 256, height: 1, depth_or_array_layers: 1 },
);
```

---

## Buffer Sizes

| Buffer | Type | Size (bytes) | Update Frequency |
|--------|------|--------------|------------------|
| GpuTransform array | Storage | 4,352 (32 × 136) | On flame change |
| GpuVariationParams array | Storage | 51,200 (32 × 1,600) | On param change |
| GpuParams | Uniform | 88 | Every frame |
| TonemapParams | Uniform | 48 | On settings change |
| AccumulateParams | Uniform | 48 | On settings change |
| Histogram | Storage | width × height × 16 | Every frame (write) |
| Iteration counts | Storage | width × height × 4 | Every frame (write) |
| Palette texture | Texture 1D | 1,024 (256 × RGBA) | On palette change |
| Accumulation textures | Texture 2D | width × height × 8 (Rgba16Float) | Every frame (ping-pong) |

**Total GPU Memory (1920×1080 example):**
- Static buffers: ~56 KB
- Histogram: 31.5 MB
- Iteration counts: 7.9 MB
- Accumulation textures: 15.8 MB (2× for ping-pong)
- **Total:** ~55 MB

---

## Common Buffer Modification Tasks

| Task | Files to Modify |
|------|-----------------|
| Add field to GpuParams | [buffers.rs](../../src/gpu/buffers.rs), [header.wgsl](../../shaders/core/header.wgsl) |
| Add field to GpuTransform | [buffers.rs](../../src/gpu/buffers.rs), [header.wgsl](../../shaders/core/header.wgsl), [transforms.rs](../../src/scene/transforms.rs) |
| Change tone mapping params | [buffers.rs](../../src/gpu/buffers.rs) `TonemapParams`, [tonemap.wgsl](../../shaders/tonemap.wgsl) |
| Change accumulation behavior | [buffers.rs](../../src/gpu/buffers.rs) `AccumulateParams`, [accumulate.wgsl](../../shaders/accumulate.wgsl) |
| Add variation parameter | [variations/mod.rs](../../src/variations/mod.rs) `add_parameters()`, shader uses `get_param()` |
| Change histogram format | [buffers.rs](../../src/gpu/buffers.rs), [main_2d.wgsl](../../shaders/core/main_2d.wgsl), [accumulate.wgsl](../../shaders/accumulate.wgsl) |

**Important:** When adding fields, always check alignment! Use `bytemuck::offset_of!()` to verify struct layout matches shader expectations.

---

**Last Updated:** 2025-10-28
**Related Documentation:**
- [ARCHITECTURE.md](../ARCHITECTURE.md) - Overall system design
- [RENDERER.md](RENDERER.md) - How buffers are used in rendering *(coming soon)*
- [SHADERS.md](SHADERS.md) - WGSL shader access patterns *(coming soon)*
