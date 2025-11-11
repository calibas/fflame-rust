# Supersampling Anti-Aliasing (SSAA)

**Status:** Planning / Design
**Priority:** Medium (quality improvement for export, optional for interactive)
**Complexity:** Medium (add 1 render pass, modify buffer allocation)

---

## Problem Statement

Current rendering uses **temporal accumulation** for progressive refinement, but lacks **spatial anti-aliasing**. This results in:

1. **Jagged edges** at low iteration counts (before temporal accumulation smooths them)
2. **Stairstepping artifacts** on diagonal lines and curves
3. **No spatial filtering** between neighboring pixels (unlike Apophysis's Gaussian filter)

### Comparison to Apophysis

**Apophysis Method** (spatial oversampling):
```
1. Render to histogram at (width × oversample) × (height × oversample)
2. Divide samples by oversample² (fewer per bucket, but more buckets)
3. Apply Gaussian spatial filter during downsample
4. Result: Smooth edges via spatial blur
```

**Our Current Method** (temporal accumulation):
```
1. Render to histogram at output resolution (1:1)
2. Accumulate samples over time (ping-pong buffers)
3. Result: Progressive quality, but no spatial AA
```

---

## Proposed Solution: Resolution-Based Supersampling

Instead of Apophysis's spatial oversampling + Gaussian filter, render at **higher resolution** then downsample. This is simpler and arguably **better** than Apophysis's approach.

### Architecture

#### Current Pipeline (3 passes)
```
┌─────────────────────────────────┐
│ 1. Compute Pass                 │
│    - Trajectory shader          │
│    - Write to histogram (u32)   │
│    - Resolution: width × height │
└─────────────────────────────────┘
          ↓
┌─────────────────────────────────┐
│ 2. Accumulate Pass              │
│    - Read histogram             │
│    - Blend with previous        │
│    - Swap ping-pong textures    │
│    - Resolution: width × height │
└─────────────────────────────────┘
          ↓
┌─────────────────────────────────┐
│ 3. Tonemap Pass                 │
│    - Log/linear tone mapping    │
│    - Gamma correction           │
│    - Output to screen           │
│    - Resolution: width × height │
└─────────────────────────────────┘
```

#### Proposed Pipeline (4 passes, with supersampling)
```
┌─────────────────────────────────────────────┐
│ 1. Compute Pass                             │
│    - Trajectory shader                      │
│    - Write to histogram (u32)               │
│    - Resolution: render_width × render_height │
│    - (render = display × supersample_factor) │
└─────────────────────────────────────────────┘
          ↓
┌─────────────────────────────────────────────┐
│ 2. Accumulate Pass                          │
│    - Read histogram                         │
│    - Blend with previous                    │
│    - Swap ping-pong textures                │
│    - Resolution: render_width × render_height │
└─────────────────────────────────────────────┘
          ↓
┌─────────────────────────────────────────────┐
│ 3. Downsample Pass (NEW)                    │
│    - Read high-res accumulation             │
│    - Average supersample² pixels            │
│    - Write to display-res texture           │
│    - Resolution: render → display           │
└─────────────────────────────────────────────┘
          ↓
┌─────────────────────────────────────────────┐
│ 4. Tonemap Pass                             │
│    - Read downsampled texture               │
│    - Log/linear tone mapping                │
│    - Gamma correction                       │
│    - Output to screen                       │
│    - Resolution: display_width × display_height │
└─────────────────────────────────────────────┘
```

---

## Implementation Plan

### Phase 1: Core Infrastructure

#### 1.1 Add Resolution Tracking to FlameRenderer

**File:** `src/renderer/compute_kernel.rs`

```rust
pub struct FlameRenderer {
    // ... existing fields ...

    // NEW: Separate render and display resolutions
    pub render_width: u32,        // Actual rendering resolution (may be supersample × display)
    pub render_height: u32,
    pub display_width: u32,       // Output/screen resolution
    pub display_height: u32,
    pub supersample_factor: u32,  // 1 (off), 2 (4× pixels), or 4 (16× pixels)

    // ... rest of fields ...
}
```

**Changes:**
- Replace `width/height` with `render_width/render_height` for buffers
- Add `display_width/display_height` for final output
- Calculate: `render_width = display_width * supersample_factor`

#### 1.2 Update FlameBuffers Size Calculation

**File:** `src/gpu/buffers.rs`

```rust
impl FlameBuffers {
    pub fn new(device: &Device, queue: &Queue, render_width: u32, render_height: u32, flame: &Flame) -> Self {
        // All buffers now use render_width/render_height (high-res)

        // Histogram buffer: width × height × 4 u32 (R, G, B, density)
        let histogram_size = (render_width * render_height * 4 * std::mem::size_of::<u32>()) as u64;

        // Ping-pong accumulation textures: Rgba16Float at render resolution
        let accumulation_texture_a = device.create_texture(&TextureDescriptor {
            size: Extent3d {
                width: render_width,
                height: render_height,
                depth_or_array_layers: 1,
            },
            format: TextureFormat::Rgba16Float,
            // ...
        });

        // NEW: Downsampled texture at display resolution
        let downsampled_texture = device.create_texture(&TextureDescriptor {
            size: Extent3d {
                width: display_width,
                height: display_height,
                depth_or_array_layers: 1,
            },
            format: TextureFormat::Rgba16Float,  // Match accumulation format
            usage: TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING,
            // ...
        });

        // ...
    }
}
```

**New Fields:**
```rust
pub struct FlameBuffers {
    // ... existing fields ...

    // NEW: Downsampled texture for display
    pub downsampled_texture: Texture,
    pub downsampled_view: TextureView,

    // Track resolutions
    pub render_width: u32,
    pub render_height: u32,
    pub display_width: u32,
    pub display_height: u32,
}
```

#### 1.3 Add Downsample Pipeline

**File:** `src/gpu/pipelines.rs`

```rust
pub struct FlamePipelines {
    // ... existing pipelines ...
    pub downsample_pipeline: ComputePipeline,
    pub downsample_bind_group_layout: BindGroupLayout,
}

impl FlamePipelines {
    pub fn new(device: &Device, surface_format: TextureFormat, flame: &Flame) -> Self {
        // ... existing shader loads ...

        let downsample_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Downsample Shader"),
            source: ShaderSource::Wgsl(include_str!("../../shaders/downsample.wgsl").into()),
        });

        // Bind group layout for downsample pass
        let downsample_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Downsample Bind Group Layout"),
            entries: &[
                // High-res accumulation texture (input)
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: false },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Downsampled texture (output)
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture {
                        access: StorageTextureAccess::WriteOnly,
                        format: TextureFormat::Rgba16Float,
                        view_dimension: TextureViewDimension::D2,
                    },
                    count: None,
                },
                // Params (supersample factor)
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Create pipeline
        let downsample_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Downsample Pipeline"),
            layout: Some(&device.create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("Downsample Pipeline Layout"),
                bind_group_layouts: &[&downsample_bind_group_layout],
                push_constant_ranges: &[],
            })),
            module: &downsample_shader,
            entry_point: "main",
            compilation_options: Default::default(),
            cache: None,
        });

        Self {
            // ... existing fields ...
            downsample_pipeline,
            downsample_bind_group_layout,
        }
    }

    pub fn create_downsample_bind_group(&self, device: &Device, buffers: &FlameBuffers, params_buffer: &Buffer) -> BindGroup {
        device.create_bind_group(&BindGroupDescriptor {
            label: Some("Downsample Bind Group"),
            layout: &self.downsample_bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(buffers.current_accumulation_view()),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(&buffers.downsampled_view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: params_buffer.as_entire_binding(),
                },
            ],
        })
    }
}
```

---

### Phase 2: Downsample Shader

**File:** `shaders/downsample.wgsl`

```wgsl
// Downsample shader - reduces high-res accumulation to display resolution
// Uses simple box filter (average supersample² pixels)

struct DownsampleParams {
    supersample_factor: u32,
    render_width: u32,
    render_height: u32,
    display_width: u32,
    display_height: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
}

@group(0) @binding(0) var high_res_texture: texture_2d<f32>;
@group(0) @binding(1) var downsampled_texture: texture_storage_2d<rgba16float, write>;
@group(0) @binding(2) var<uniform> params: DownsampleParams;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let out_pixel = vec2<i32>(i32(global_id.x), i32(global_id.y));

    // Bounds check
    if (global_id.x >= params.display_width || global_id.y >= params.display_height) {
        return;
    }

    // Box filter: average supersample_factor × supersample_factor pixels
    var sum = vec4<f32>(0.0);
    let ss = params.supersample_factor;

    for (var dy = 0u; dy < ss; dy++) {
        for (var dx = 0u; dx < ss; dx++) {
            let in_x = i32(global_id.x * ss + dx);
            let in_y = i32(global_id.y * ss + dy);
            let in_pixel = vec2<i32>(in_x, in_y);

            // Bounds check for high-res texture
            if (in_x < i32(params.render_width) && in_y < i32(params.render_height)) {
                sum += textureLoad(high_res_texture, in_pixel, 0);
            }
        }
    }

    // Average (divide by number of samples)
    let sample_count = f32(ss * ss);
    let average = sum / sample_count;

    // Write to output
    textureStore(downsampled_texture, out_pixel, average);
}
```

**Alternative: Bilinear Filter** (higher quality, slightly more expensive)
```wgsl
// Instead of box filter, sample with bilinear interpolation
// Requires texture sampler instead of direct textureLoad

@group(0) @binding(0) var high_res_texture: texture_2d<f32>;
@group(0) @binding(1) var high_res_sampler: sampler;  // Bilinear filtering
@group(0) @binding(2) var downsampled_texture: texture_storage_2d<rgba16float, write>;
@group(0) @binding(3) var<uniform> params: DownsampleParams;

@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let out_pixel = vec2<i32>(i32(global_id.x), i32(global_id.y));

    // Bounds check
    if (global_id.x >= params.display_width || global_id.y >= params.display_height) {
        return;
    }

    // Calculate UV coordinates (center of output pixel maps to center of NxN input block)
    let u = (f32(global_id.x) + 0.5) / f32(params.display_width);
    let v = (f32(global_id.y) + 0.5) / f32(params.display_height);

    // Sample with hardware bilinear filtering (free on GPU)
    let color = textureSample(high_res_texture, high_res_sampler, vec2<f32>(u, v));

    textureStore(downsampled_texture, out_pixel, color);
}
```

---

### Phase 3: Render Pass Integration

**File:** `src/renderer/compute_kernel.rs`

```rust
impl FlameRenderer {
    /// NEW: Downsample pass - reduce high-res accumulation to display resolution
    pub fn downsample_pass(&self, encoder: &mut CommandEncoder) {
        let mut compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("Downsample Pass"),
            timestamp_writes: None,
        });

        compute_pass.set_pipeline(&self.pipelines.downsample_pipeline);
        compute_pass.set_bind_group(0, &self.downsample_bind_group, &[]);

        // Dispatch one thread per output pixel (8x8 tiles)
        let workgroups_x = (self.display_width + 7) / 8;
        let workgroups_y = (self.display_height + 7) / 8;
        compute_pass.dispatch_workgroups(workgroups_x, workgroups_y, 1);

        drop(compute_pass);
    }

    /// Update tonemap pass to read from downsampled texture instead of accumulation
    pub fn tonemap_pass(&self, encoder: &mut CommandEncoder, target_view: &TextureView) {
        let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Tonemap Pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: target_view,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(Color::BLACK),
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
        });

        render_pass.set_pipeline(&self.pipelines.tonemap_pipeline);
        render_pass.set_bind_group(0, &self.tonemap_bind_group, &[]);
        render_pass.draw(0..3, 0..1); // Fullscreen triangle

        drop(render_pass);
    }
}
```

**File:** `src/app/mod.rs` (update render loop)

```rust
fn render(&mut self, window: &Window) -> Result<(), SurfaceError> {
    // ... existing setup ...

    if let Some(renderer) = self.flame_renderer.as_mut() {
        let mut encoder = self.gpu.device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        // 1. Compute pass (generate samples at render resolution)
        let samples = renderer.compute_pass(&mut encoder, &self.gpu.queue, num_workgroups,
            config.iterations_per_thread, config.zoom, config.pan_x, config.pan_y,
            config.rotation, config.camera_rotation_x, config.camera_rotation_y,
            config.camera_z, config.speed_factor, true);

        // 2. Accumulate pass (blend at render resolution)
        renderer.accumulate_pass(&mut encoder, &self.gpu.queue, &self.gpu.device, samples);

        // 3. Downsample pass (only if supersample_factor > 1)
        if renderer.supersample_factor > 1 {
            renderer.downsample_pass(&mut encoder);
        }

        // 4. Tonemap pass (display)
        renderer.tonemap_pass(&mut encoder, &view);

        // ... rest of render function ...
    }

    Ok(())
}
```

---

### Phase 4: Configuration and UI

#### 4.1 Add to FractalConfig

**File:** `src/config/fractal_config.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FractalConfig {
    // ... existing fields ...

    #[serde(default = "default_supersample_factor")]
    pub supersample_factor: u32,  // 1, 2, or 4
}

fn default_supersample_factor() -> u32 { 1 }  // Off by default
```

**File:** `src/config/defaults.rs`

```rust
pub const DEFAULT_SUPERSAMPLE_FACTOR: u32 = 1;  // Off
pub const MAX_SUPERSAMPLE_FACTOR: u32 = 4;      // 16× pixels
```

#### 4.2 Add ConfigPath Variant

**File:** `src/config/delta.rs`

```rust
#[derive(Debug, Clone, PartialEq)]
pub enum ConfigPath {
    // ... existing variants ...
    SupersampleFactor,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConfigValue {
    // ... existing variants ...
    U32(u32),
}
```

#### 4.3 Add UI Control

**File:** `src/ui/mod.rs` (in render settings section)

```rust
// Supersampling (anti-aliasing)
ui.label("Supersampling:");
ui.horizontal(|ui| {
    let mut ss_factor = config.supersample_factor;
    if ui.radio_value(&mut ss_factor, 1, "Off (1×)").clicked() {
        config_manager.update_param(ConfigPath::SupersampleFactor, ConfigValue::U32(1), false)?;
    }
    if ui.radio_value(&mut ss_factor, 2, "2× (4× pixels)").clicked() {
        config_manager.update_param(ConfigPath::SupersampleFactor, ConfigValue::U32(2), false)?;
    }
    if ui.radio_value(&mut ss_factor, 4, "4× (16× pixels)").clicked() {
        config_manager.update_param(ConfigPath::SupersampleFactor, ConfigValue::U32(4), false)?;
    }
});

// Show memory impact
let render_pixels = (config.width * ss_factor * config.height * ss_factor) as f32;
let memory_mb = (render_pixels * 8.0 * 2.0) / (1024.0 * 1024.0);  // Rgba16Float × 2 (ping-pong)
ui.label(format!("Memory: {:.1} MB", memory_mb));
```

---

## Performance Impact

### Memory Usage

| Resolution | 1× (off) | 2× SS | 4× SS |
|------------|----------|-------|-------|
| 800×600 | 5.8 MB | 23.0 MB | 92.2 MB |
| 1920×1080 | 31.1 MB | 124.4 MB | 497.7 MB |
| 3840×2160 | 124.4 MB | 497.7 MB | 1991 MB |

**Formula:** `width × height × supersample² × 8 bytes × 2 (ping-pong)`

### Computation Cost

| Supersample Factor | Pixels to Render | Frame Time Impact |
|--------------------|------------------|-------------------|
| 1× (off) | 1× | Baseline |
| 2× | 4× | ~4× slower |
| 4× | 16× | ~16× slower |

**Example:**
- 1× @ 1920×1080: 60 FPS (16.7ms/frame)
- 2× @ 1920×1080: 15 FPS (67ms/frame)
- 4× @ 1920×1080: 4 FPS (268ms/frame)

### Downsample Pass Cost

**Negligible** - Box filter is extremely cheap:
- 1920×1080 → ~0.1ms (single pass over output pixels)
- No texture sampling, just arithmetic averaging

---

## Advantages Over Apophysis Method

| Aspect | Apophysis (spatial oversample) | Our Method (resolution supersample) |
|--------|-------------------------------|-------------------------------------|
| **Sample Efficiency** | Divides samples by SS² (fewer per bucket) | Keeps iteration count (more total samples) |
| **Implementation** | Complex (Gaussian filter kernel, gutter) | Simple (box/bilinear filter) |
| **Filter Quality** | Gaussian blur (slight blur everywhere) | Box/bilinear (sharp with AA) |
| **GPU Utilization** | N/A (CPU-based) | Fully GPU-accelerated |
| **Code Complexity** | ~200 lines (filter + downsampling) | ~50 lines (simple averaging) |
| **Memory** | Same (histogram + accumulation) | Same (histogram + accumulation) |

**Key Difference:** We don't reduce sample density - we render at higher resolution, so each output pixel gets `supersample²` subpixel samples naturally. This is **standard SSAA** used in games.

---

## Recommended Settings

### Interactive Use
- **Default:** 1× (off) - full frame rate for editing
- **Optional:** 2× for high-quality preview (4× slower but acceptable)

### Export/Final Render
- **Recommended:** 2× for smooth edges (good balance)
- **High Quality:** 4× for publication-quality output (16× slower)

### UI Behavior
- Show radio buttons: Off (1×), 2×, 4×
- Display memory usage estimate
- Warn if VRAM usage exceeds GPU capacity (query via wgpu)
- Auto-reset to 1× if resize causes OOM

---

## Migration Path

### Phase 1: Core Implementation (2-3 days)
1. Add resolution tracking to FlameRenderer
2. Update FlameBuffers with render/display separation
3. Create downsample shader (box filter)
4. Add downsample pipeline and bind group

### Phase 2: Integration (1 day)
1. Update render loop to call downsample pass
2. Modify tonemap pass to read downsampled texture
3. Handle supersample_factor = 1 (skip downsample pass)

### Phase 3: Configuration (1 day)
1. Add supersample_factor to FractalConfig
2. Add ConfigPath/ConfigValue variants
3. Wire up ConfigManager updates
4. Add UI controls with memory display

### Phase 4: Testing & Polish (1 day)
1. Test all supersample factors (1×, 2×, 4×)
2. Verify memory usage calculations
3. Test window resize with supersampling enabled
4. Add error handling for OOM scenarios

**Total Estimate:** 5-6 days

---

## Future Enhancements

### Adaptive Supersampling
Only supersample edges (detect via gradient analysis):
- Render at 1× base resolution
- Detect high-gradient areas (edges)
- Supersample only those regions
- Blend results

**Benefit:** 2-4× faster than full SSAA with similar quality

### Temporal Anti-Aliasing (TAA)
Combine with our existing temporal accumulation:
- Jitter camera slightly each frame
- Accumulate subpixel-offset samples
- Free anti-aliasing without resolution increase

**Benefit:** Better AA with no memory overhead

### Multi-Sample Anti-Aliasing (MSAA)
Use hardware MSAA instead of SSAA:
- Let GPU handle subpixel samples
- 2×/4×/8× MSAA with lower memory cost

**Downside:** Doesn't work with compute shaders (would need to switch accumulation to render pass)

---

## Related Documentation

- [RENDERER.md](../main/RENDERER.md) - Current 3-pass pipeline
- [BUFFERS.md](../main/BUFFERS.md) - GPU buffer layouts
- [CONFIG.md](../main/CONFIG.md) - Configuration management
- [apophysis-remaining-features.md](apophysis-remaining-features.md) - Apophysis comparison

---

## Open Questions

1. **Default Setting:** Should 2× be default for export, or always require explicit enable?
2. **WASM:** How does this affect WASM memory limits? (may need to disable on web)
3. **Mobile:** Should we disable or limit to 1×/2× on mobile GPUs?
4. **Auto-Detection:** Detect GPU VRAM and auto-select safe supersample factor?
5. **Bilinear vs Box:** Which filter provides better quality? (test both and compare)

---

**Last Updated:** 2025-01-10
**Author:** Claude (AI Assistant)
