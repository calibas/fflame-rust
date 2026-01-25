# Renderer Architecture

**Overview:** The FlameRenderer orchestrates the 3-pass GPU rendering pipeline (compute → accumulate → tonemap) and manages rendering state.

**See also:**
- [ARCHITECTURE.md](../ARCHITECTURE.md) - Overall system design
- [BUFFERS.md](BUFFERS.md) - GPU buffer layouts
- [TRANSFORMS.md](TRANSFORMS.md) - Flame algorithm
- [SHADERS.md](SHADERS.md) - Shader implementation *(coming soon)*

**Code locations:**
- [src/renderer/compute_kernel.rs](../../src/renderer/compute_kernel.rs) - FlameRenderer implementation
- [src/renderer/render.rs](../../src/renderer/render.rs) - Unified render API for headless export
- [src/gpu/pipelines.rs](../../src/gpu/pipelines.rs) - Pipeline creation
- [src/gpu/buffers.rs](../../src/gpu/buffers.rs) - Buffer management

---

## Rendering Pipeline Overview

The renderer uses a **3-pass GPU pipeline** that runs every frame:

```
┌─────────────────────────────────────────────────────┐
│ 1. COMPUTE PASS (main_template.wgsl)               │
│    Generate fractal samples                         │
│    - Each thread: N iterations                      │
│    - Atomic write to histogram buffer (u32)         │
│    - Runtime: ~1-2ms @ 1080p                        │
└─────────────────────────────────────────────────────┘
                       ↓
┌─────────────────────────────────────────────────────┐
│ 2. ACCUMULATE PASS (accumulate.wgsl)                │
│    Progressive refinement                           │
│    - Read histogram, decode colors                  │
│    - Blend with previous frame (exponential avg)    │
│    - Clear histogram for next frame                 │
│    - Swap ping-pong buffers                         │
│    - Runtime: ~0.1ms @ 1080p                        │
└─────────────────────────────────────────────────────┘
                       ↓
┌─────────────────────────────────────────────────────┐
│ 3. TONEMAP PASS (tonemap.wgsl)                      │
│    Display rendering                                │
│    - Read accumulation texture                      │
│    - Log/linear tone mapping                        │
│    - Gamma correction                               │
│    - Background blending                            │
│    - Output to screen                               │
│    - Runtime: ~0.1ms @ 1080p                        │
└─────────────────────────────────────────────────────┘
```

**Total frame time:** ~1-2ms (500-1000 FPS GPU-bound, typically 60 FPS display-limited)

---

## FlameRenderer Structure

### Core Responsibilities

```rust
pub struct FlameRenderer {
    // GPU resources
    pipelines: FlamePipelines,
    buffers: FlameBuffers,

    // Rendering state
    samples_accumulated: u64,
    total_iterations: u64,
    current_buffer_index: usize,  // Ping-pong: 0 or 1

    // Configuration
    width: u32,
    height: u32,
    workgroups: u32,
    iterations_per_thread: u32,
}
```

**Key Methods:**
- `new()` - Initialize GPU resources
- `render()` - Execute 3-pass pipeline (main entry point)
- `reset()` - Clear accumulation buffers
- `update_flame()` - Upload new flame parameters
- `load_config()` - Atomic config loading
- `capture_png()` - Export to PNG (transparent or opaque)
- `resize()` - Handle window resize

---

## Pass 1: Compute Pass

**Purpose:** Generate fractal samples by iterating the flame algorithm on the GPU.

### Code Flow

**Location:** [src/renderer/compute_kernel.rs](../../src/renderer/compute_kernel.rs) - `compute_pass()`

```rust
pub fn compute_pass(&mut self, encoder: &mut CommandEncoder) {
    // 1. Update params buffer (seed, iterations, etc.)
    let gpu_params = GpuParams {
        seed: self.frame_counter,  // Changes each frame
        iterations_per_thread: self.iterations_per_thread,
        // ... other fields
    };
    queue.write_buffer(&self.buffers.params_buffer, 0, bytemuck::bytes_of(&gpu_params));

    // 2. Create compute pass
    let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
        label: Some("Compute Pass"),
    });

    // 3. Select pipeline (2D or 3D based on render mode)
    let pipeline = match self.render_mode {
        RenderMode::TwoD => &self.pipelines.compute_2d,
        RenderMode::ThreeD => &self.pipelines.compute_3d,
    };
    pass.set_pipeline(pipeline);

    // 4. Bind resources (transforms, histogram, palette, etc.)
    pass.set_bind_group(0, &self.buffers.compute_bind_group, &[]);

    // 5. Dispatch workgroups (default: 128 workgroups × 64 threads = 8,192 threads)
    pass.dispatch_workgroups(self.workgroups, 1, 1);
}
```

### Workload Calculation

**Default settings:**
- Workgroups: 128
- Threads per workgroup: 64 (8×8 from `@workgroup_size(8, 8)`)
- Iterations per thread: 256

**Total iterations per frame:**
```
128 workgroups × 64 threads × 256 iterations = 2,097,152 iterations/frame
```

**At 60 FPS:**
```
2M iterations/frame × 60 FPS = 120M iterations/second
```

### Output

**Histogram buffer updated:**
- Each iteration writes to histogram via `atomicAdd()`
- 4× u32 per pixel: R, G, B, Density
- Thread-safe atomic operations (no race conditions)

---

## Pass 2: Accumulate Pass

**Purpose:** Blend new samples with previous accumulation for progressive refinement.

### Code Flow

**Location:** [src/renderer/compute_kernel.rs](../../src/renderer/compute_kernel.rs) - `accumulate_pass()`

```rust
pub fn accumulate_pass(&mut self, encoder: &mut CommandEncoder) {
    // 1. Update accumulation params
    let blend_factor = if dynamic_blend {
        1.0 / (self.samples_accumulated as f32)  // Exponential
    } else {
        self.blend_rate  // Fixed rate
    };

    let params = AccumulateParams {
        blend_factor,
        histogram_color_scale: 100.0,
        low_density_smoothing: 0.5,
        density_compression_strength: 0.0,
        target_iterations_per_pixel: 0,
        // ... other fields
    };
    queue.write_buffer(&self.buffers.accumulate_params_buffer, 0, bytemuck::bytes_of(&params));

    // 2. Create compute pass
    let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
        label: Some("Accumulate Pass"),
    });

    // 3. Set pipeline
    pass.set_pipeline(&self.pipelines.accumulate);

    // 4. Bind resources (ping-pong buffers: read from prev, write to current)
    pass.set_bind_group(0, &self.buffers.accumulate_bind_groups[self.current_buffer_index], &[]);

    // 5. Dispatch (one thread per pixel)
    let dispatch_x = (self.width + 15) / 16;  // Round up to 16
    let dispatch_y = (self.height + 15) / 16;
    pass.dispatch_workgroups(dispatch_x, dispatch_y, 1);

    // 6. Swap ping-pong buffers for next frame
    self.current_buffer_index = 1 - self.current_buffer_index;

    // 7. Update counters
    self.samples_accumulated += 1;
    self.total_iterations += (self.workgroups * 64 * self.iterations_per_thread) as u64;
}
```

### Ping-Pong Buffers

**Two accumulation textures:**
- Buffer A (index 0)
- Buffer B (index 1)

**Swap pattern:**
```
Frame 0: Read from B, write to A  (current_buffer_index = 0)
Frame 1: Read from A, write to B  (current_buffer_index = 1)
Frame 2: Read from B, write to A  (current_buffer_index = 0)
...
```

**Why ping-pong?**
- Cannot read and write same texture in compute shader
- Avoids copying entire texture each frame
- Efficient progressive accumulation

### Blending Formula

**Shader code (accumulate.wgsl):**
```wgsl
// Decode histogram
let color_new = vec3(r_sum, g_sum, b_sum) / density;

// Read previous accumulation
let color_prev = textureLoad(prev_accumulation, pixel_coords);

// Apply accumulation controls
let density_factor = pow(density, low_density_smoothing);
let compression_factor = 1.0 / (1.0 + density × density_compression_strength × 0.01);
let convergence_gate = (iteration_count < target) ? 1.0 : 0.0;

let adjusted_blend = blend_factor × density_factor × compression_factor × convergence_gate;

// Exponential moving average
let color_result = mix(color_prev.rgb, color_new, adjusted_blend);
let density_result = mix(color_prev.a, density, adjusted_blend);

// Write to output
textureStore(output_texture, pixel_coords, vec4(color_result, density_result));

// Clear histogram
histogram[base_idx + 0] = 0u;
histogram[base_idx + 1] = 0u;
histogram[base_idx + 2] = 0u;
histogram[base_idx + 3] = 0u;
```

---

## Pass 3: Tonemap Pass

**Purpose:** Convert HDR accumulation to displayable LDR image with tone mapping.

### Code Flow

**Location:** [src/renderer/compute_kernel.rs](../../src/renderer/compute_kernel.rs) - `tonemap_pass()`

```rust
pub fn tonemap_pass(
    &self,
    encoder: &mut CommandEncoder,
    view: &TextureView,  // Screen surface
) {
    // 1. Update tonemap params
    let params = TonemapParams {
        exposure: self.exposure,
        gamma: self.gamma,
        density_scale: self.density_scale,
        tonemap_mode: self.tonemap_mode as u32,
        background_color: self.background_color,
        use_curve: self.use_curve as u32,
        tonemap_curve: self.tonemap_curve,
    };
    queue.write_buffer(&self.buffers.tonemap_params_buffer, 0, bytemuck::bytes_of(&params));

    // 2. Create render pass (draws to screen)
    let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
        label: Some("Tonemap Pass"),
        color_attachments: &[Some(RenderPassColorAttachment {
            view,  // Screen texture
            ops: Operations {
                load: LoadOp::Clear(Color::BLACK),
                store: StoreOp::Store,
            },
        })],
        // ... other fields
    });

    // 3. Set pipeline (render pipeline, not compute)
    pass.set_pipeline(&self.pipelines.tonemap);

    // 4. Bind resources (accumulation texture, palette, params)
    pass.set_bind_group(0, &self.buffers.tonemap_bind_group, &[]);

    // 5. Draw fullscreen quad (3 vertices, triangle strip)
    pass.draw(0..3, 0..1);
}
```

### Tone Mapping Modes

**Logarithmic (default):**
```wgsl
let intensity = dot(color.rgb, vec3(0.3, 0.59, 0.11));
let log_intensity = log(1.0 + intensity * exposure);
let scale = log_intensity / (intensity + 1e-6);
color = color * scale;
```
- Compresses bright areas
- Good for high-dynamic-range fractals
- Preserves detail in shadows

**Linear:**
```wgsl
color = color * exposure;
```
- No compression
- Direct brightness scaling
- Can blow out bright areas

**S-Curve (optional):**
```wgsl
// After tone mapping, apply contrast curve
color = color / (color + tonemap_curve);
```
- Increases contrast
- Strength controlled by `tonemap_curve` (0.0-10.0)

---

## State Management

### Reset Behavior

**What gets reset:**
```rust
pub fn reset(&mut self) {
    // 1. Clear accumulation textures (write zeros)
    // 2. Clear histogram buffer (write zeros)
    // 3. Reset counters
    self.samples_accumulated = 0;
    self.total_iterations = 0;
    self.current_buffer_index = 0;
}
```

**What does NOT get reset:**
- GPU parameters (transforms, palette, etc.)
- Pipeline configuration
- Buffer allocations

**When to reset:**
- Flame changed (transforms, variations, colors)
- View changed (zoom, pan, rotation)
- Palette changed
- Any parameter that affects sample generation

### Atomic Config Loading

**Problem:** Changing multiple parameters (e.g., loading preset) can cause visual glitches if done incrementally.

**Solution:** `load_config()` atomically updates all GPU state:

```rust
pub fn load_config(&mut self, config: &FractalConfig) {
    // 1. Update all transforms at once (zero-pad unused slots)
    let mut gpu_transforms = [GpuTransform::zeroed(); 32];
    for (i, xform) in config.flame.transforms.iter().enumerate() {
        gpu_transforms[i] = xform.to_gpu();
    }
    queue.write_buffer(&self.buffers.transform_buffer, 0, bytemuck::cast_slice(&gpu_transforms));

    // 2. Update variation parameters
    let variation_params = build_variation_params(&config.flame);
    queue.write_buffer(&self.buffers.variation_params_buffer, 0, bytemuck::cast_slice(&variation_params));

    // 3. Update palette texture
    queue.write_texture(/* ... palette data ... */);

    // 4. Update all uniform buffers (params, tonemap, accumulate)
    // ...

    // 5. Reset accumulation (new config = fresh start)
    self.reset();
}
```

**Key insight:** All GPU writes happen before reset(), ensuring consistent state.

---

## Unified Render API

**Added:** 2025-12-24

For headless rendering (CLI export, WASM export, thumbnails), use the unified `render()` API:

**Location:** [src/renderer/render.rs](../../src/renderer/render.rs)

```rust
use crate::renderer::{render, RenderJob, RenderProgress, NoProgress};

// Configure render job
let job = RenderJob::new(&config, width, height)
    .with_iterations_per_thread(256)
    .with_transparent(false);

// Execute render
let result = render(&device, &queue, job, &mut NoProgress).await?;

// Result contains:
// - result.rgba_data: Vec<u8> (RGBA8 pixel data)
// - result.width, result.height: dimensions
// - result.total_iterations: u64
// - result.render_time_ms: f64
```

**RenderJob builder methods:**
- `with_iterations(target)` - Override max_iterations from config
- `with_iterations_per_thread(n)` - GPU iterations per dispatch
- `with_burn_in(n)` - Skip first N iterations (default: 20)
- `with_transparent(bool)` - Transparent or opaque PNG

**RenderProgress trait:**
```rust
pub trait RenderProgress {
    fn on_progress(&mut self, completed: u64, total: u64);
    fn is_cancelled(&self) -> bool { false }
}
```

**Used by:**
- CLI headless export (`app/export.rs`)
- WASM headless export (`app/export.rs`)
- Thumbnail generation (`renderer/thumbnail.rs`)
- Animation frame rendering (`animation/export.rs`)

---

## PNG Export

The renderer supports two PNG export modes:

### Transparent PNG Export

**Purpose:** Preserve alpha channel for compositing.

**Method:** Read from accumulation buffer (Rgba16Float), apply tone mapping on CPU.

**Location:** [src/renderer/compute_kernel.rs](../../src/renderer/compute_kernel.rs) - `capture_from_accumulation_buffer()`

```rust
pub fn capture_transparent_png(&mut self) -> Result<Vec<u8>> {
    // 1. Create CPU-readable buffer
    let buffer = device.create_buffer(&BufferDescriptor {
        size: width * height * 8,  // Rgba16Float = 8 bytes/pixel
        usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        // ...
    });

    // 2. Copy accumulation texture → buffer
    encoder.copy_texture_to_buffer(
        accumulation_texture.as_image_copy(),
        buffer.as_image_copy(),
        extent,
    );
    queue.submit([encoder.finish()]);

    // 3. Map buffer and read f16 data
    let slice = buffer.slice(..);
    slice.map_async(MapMode::Read, /* ... */);
    device.poll(Maintain::Wait);
    let data = slice.get_mapped_range();

    // 4. Convert f16 → u8 with tone mapping
    let mut rgba8 = Vec::with_capacity(width * height * 4);
    for pixel in data.chunks_exact(8) {
        let r = f16::from_bits(u16::from_le_bytes([pixel[0], pixel[1]])).to_f32();
        let g = f16::from_bits(u16::from_le_bytes([pixel[2], pixel[3]])).to_f32();
        let b = f16::from_bits(u16::from_le_bytes([pixel[4], pixel[5]])).to_f32();
        let density = f16::from_bits(u16::from_le_bytes([pixel[6], pixel[7]])).to_f32();

        // Apply tone mapping (log scale)
        let intensity = 0.3 * r + 0.59 * g + 0.11 * b;
        let log_intensity = (1.0 + intensity * exposure).ln();
        let scale = log_intensity / (intensity + 1e-6);
        let tonemapped = [r * scale, g * scale, b * scale];

        // Apply gamma
        let gamma_corrected = tonemapped.map(|c| c.powf(1.0 / gamma));

        // Convert to u8 with alpha
        rgba8.push((gamma_corrected[0].clamp(0.0, 1.0) * 255.0) as u8);
        rgba8.push((gamma_corrected[1].clamp(0.0, 1.0) * 255.0) as u8);
        rgba8.push((gamma_corrected[2].clamp(0.0, 1.0) * 255.0) as u8);
        rgba8.push((density * density_scale).clamp(0.0, 1.0) * 255.0) as u8);
    }

    // 5. Encode PNG
    let mut png_data = Vec::new();
    let encoder = png::Encoder::new(&mut png_data, width, height);
    encoder.write_image(&rgba8, ColorType::Rgba)?;

    Ok(png_data)
}
```

**Why not use tonemap shader?** The tonemap shader blends RGB with background_color before outputting, destroying transparency.

### Opaque PNG Export

**Purpose:** Direct screenshot with background color.

**Method:** Render via tonemap pass to temporary texture, copy to CPU.

**Location:** [src/renderer/compute_kernel.rs](../../src/renderer/compute_kernel.rs) - `capture_from_tonemap_render()`

```rust
pub fn capture_opaque_png(&mut self) -> Result<Vec<u8>> {
    // 1. Create temporary Rgba8 texture
    let texture = device.create_texture(&TextureDescriptor {
        format: TextureFormat::Rgba8UnormSrgb,
        usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
        // ...
    });

    // 2. Render tonemap pass to this texture
    let view = texture.create_view(&Default::default());
    self.tonemap_pass(&mut encoder, &view);

    // 3. Copy texture → CPU buffer
    encoder.copy_texture_to_buffer(/* ... */);
    queue.submit([encoder.finish()]);

    // 4. Map buffer and read Rgba8 data
    // (already in correct format, no conversion needed)

    // 5. Encode PNG
    let encoder = png::Encoder::new(&mut png_data, width, height);
    encoder.write_image(&rgba8_data, ColorType::Rgba)?;

    Ok(png_data)
}
```

**Advantage:** Direct GPU tone mapping (fast), includes background color.

---

## Performance Characteristics

### Typical Frame Times (1920×1080)

| Pass | Time (ms) | Percentage |
|------|-----------|------------|
| Compute (256 iters) | 1.2 | 86% |
| Accumulate | 0.1 | 7% |
| Tonemap | 0.1 | 7% |
| **Total** | **1.4** | **100%** |

**Bottleneck:** Compute pass (iteration-bound)

### Scaling with Settings

**Iterations per thread:**
```
256 iters  → 1.2ms
512 iters  → 2.4ms
1024 iters → 4.8ms
```
Linear scaling (2× iterations = 2× time)

**Resolution:**
```
1920×1080 (2M pixels) → 1.4ms
3840×2160 (8M pixels) → 1.6ms
```
Small impact (only affects accumulate/tonemap)

**Workgroups:**
```
64 groups  → 0.6ms
128 groups → 1.2ms
256 groups → 2.4ms
```
Linear scaling (2× workgroups = 2× time)

### Throughput Calculation

**Default settings:**
- 128 workgroups × 64 threads × 256 iterations = 2,097,152 iterations/frame
- Frame time: 1.4ms
- Throughput: 2M / 0.0014s = **1.5 billion iterations/second**

**At maximum quality (4096 iters/thread, 256 workgroups):**
- 256 × 64 × 4096 = 67,108,864 iterations/frame
- Frame time: ~38ms (limited by speed multiplier to 60 FPS)
- Throughput: 67M / 0.038s = **1.8 billion iterations/second**

---

## Resize Handling

**When window resizes:**
```rust
pub fn resize(&mut self, width: u32, height: u32) {
    self.width = width;
    self.height = height;

    // 1. Recreate accumulation textures (new size)
    self.buffers.recreate_accumulation_textures(device, width, height);

    // 2. Recreate histogram buffer (new size)
    self.buffers.recreate_histogram(device, width, height);

    // 3. Recreate bind groups (reference new textures)
    self.buffers.recreate_bind_groups(device, &self.pipelines);

    // 4. Reset accumulation (different resolution = fresh start)
    self.reset();
}
```

**Critical:** Accumulation textures must match viewport size (1:1 pixel mapping).

---

## Common Renderer Modification Tasks

| Task | Files to Modify |
|------|-----------------|
| Add new rendering pass | [compute_kernel.rs](../../src/renderer/compute_kernel.rs) - add pass method, [pipelines.rs](../../src/gpu/pipelines.rs) - add pipeline |
| Change accumulation formula | [accumulate.wgsl](../../shaders/accumulate.wgsl) - shader logic, [buffers.rs](../../src/gpu/buffers.rs) - AccumulateParams |
| Modify tone mapping | [tonemap.wgsl](../../shaders/tonemap.wgsl) - shader logic, [buffers.rs](../../src/gpu/buffers.rs) - TonemapParams |
| Add export format | [compute_kernel.rs](../../src/renderer/compute_kernel.rs) - add capture method, [app/export.rs](../../src/app/export.rs) - CLI interface |
| Change pipeline selection | [compute_kernel.rs](../../src/renderer/compute_kernel.rs) - update `compute_pass()` |
| Optimize performance | Profile with GPU tools, adjust workgroup sizes, tune iteration counts |

---

**Last Updated:** 2025-12-24
**Related Documentation:**
- [ARCHITECTURE.md](../ARCHITECTURE.md) - Overall system design
- [BUFFERS.md](BUFFERS.md) - GPU buffer details
- [TRANSFORMS.md](TRANSFORMS.md) - Flame algorithm
- [SHADERS.md](SHADERS.md) - Shader implementation *(coming soon)*
