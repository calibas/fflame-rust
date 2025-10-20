# Architecture Overview

Quick reference guide to understanding the codebase structure and data flow.

---

## 🏗️ Module Organization

```
fractal_flame_wgpu/
├── Core Entry Points
│   ├── main.rs                 Desktop entry → calls lib::desktop_main()
│   └── lib.rs                  Library root + WASM entry
│
├── Application Layer
│   ├── app.rs                  Main App struct
│   │                           - Event loop
│   │                           - Input handling (keyboard, mouse)
│   │                           - Render coordination
│   │                           - Config import/export
│   │                           - Undo/redo management
│   │
│   └── util.rs                 PerformanceMetrics
│                               - FPS tracking
│                               - Frame time statistics
│
├── UI Layer (egui)
│   └── ui/
│       ├── mod.rs              EguiLayer + all UI panels
│       │                       - Performance window
│       │                       - Transforms editor
│       │                       - Palette editor
│       │                       - View controls
│       │                       - Config import/export dialogs
│       │
│       └── panels.rs           (unused/empty)
│
├── Scene Layer
│   └── scene/
│       ├── mod.rs              Module exports
│       │
│       ├── transforms.rs       🔥 CORE ALGORITHM
│       │                       - Transform struct (affine + variations)
│       │                       - Point calculations (r, θ, φ)
│       │                       - 16 Variation functions (CPU reference)
│       │                       - Flame struct (transform collection + name)
│       │                       - CPU iteration reference
│       │
│       ├── presets.rs          Preset system
│       │                       - PresetLibrary (stores Vec<FractalConfig>)
│       │                       - Built-in preset creation functions
│       │                       - Auto-loads from assets/presets/ (desktop)
│       │                       - flame_to_config() helper
│       │
│       ├── assets.rs           Asset loading (desktop only)
│       │                       - load_palettes_from_dir()
│       │                       - load_configs_from_dir()
│       │                       - Filesystem-based asset discovery
│       │
│       └── palette.rs          Color system
│                               - ColorMode enum (Transform/Palette/Speed)
│                               - ColorStop gradient system
│                               - Palette with interpolation
│                               - PaletteLibrary (auto-loads from assets/)
│
├── GPU Layer
│   └── gpu/
│       ├── mod.rs              Module exports
│       │
│       ├── device.rs           GpuContext
│       │                       - wgpu instance, surface, device, queue
│       │                       - Window resize handling
│       │
│       ├── pipelines.rs        FlamePipelines
│       │                       - Compute pipeline (trajectory)
│       │                       - Compute pipeline (accumulate)
│       │                       - Render pipeline (tonemap)
│       │                       - Bind group layouts
│       │
│       └── buffers.rs          FlameBuffers + GPU data structures
│                               - Transform buffer (storage)
│                               - Palette texture (1D)
│                               - Params uniform buffers
│                               - Accumulation textures (ping-pong)
│                               - Temp samples texture
│                               - GpuTransform, GpuParams, TonemapParams
│
├── Renderer Layer
│   └── renderer/
│       ├── mod.rs              Module exports
│       │
│       └── compute_kernel.rs  🎨 RENDERING CORE
│                               FlameRenderer
│                               - Manages rendering state
│                               - Orchestrates GPU passes:
│                                 1. compute_pass() - generate samples
│                                 2. accumulate_pass() - blend samples
│                                 3. tonemap_pass() - display
│                               - Tracks samples/iterations
│                               - PNG capture (dual path):
│                                 • Transparent: Read Rgba16Float accumulation buffer
│                                 • Opaque: Render with tonemap shader
│                               - Parameter updates:
│                                 • update_flame() - individual flame updates
│                                 • load_config() - atomic FractalConfig loading
│                                 • reset() - clear accumulation only (no params)
│
├── State Management
│   ├── config.rs               FractalConfig
│   │                           - Serializable app state
│   │                           - JSON import/export
│   │                           - File save/load (.flame files)
│   │
│   └── undo.rs                 UndoHistory
│                               - 50-state circular buffer
│                               - Undo/redo tracking
│
└── Shaders (WGSL)
    ├── trajectory.wgsl         🔥 COMPUTE: Flame iteration
    │                           - PCG random number generator
    │                           - Transform selection (weighted)
    │                           - Affine transformation
    │                           - 16 Variation functions (GPU)
    │                           - Color accumulation (3 modes)
    │                           - Write to temp texture
    │
    ├── accumulate.wgsl         🔥 COMPUTE: Temporal blending
    │                           - Read temp samples
    │                           - Blend with previous accumulation
    │                           - Exponential moving average
    │                           - Write to current accumulation
    │
    └── tonemap.wgsl            🎨 RENDER: Display
                                - Read accumulation texture
                                - Log-scale tone mapping
                                - Palette lookup (Speed mode)
                                - Gamma correction
                                - Background blending
                                - Output to screen
```

---

## 🔄 Data Flow

### Initialization Flow
```
main()
  → desktop_main() [lib.rs]
    → App::run() [app.rs]
      → GpuContext::new() [gpu/device.rs]
      → EguiLayer::new() [ui/mod.rs]
      → FlameRenderer::new() [renderer/compute_kernel.rs]
        → FlamePipelines::new() [gpu/pipelines.rs]
        → FlameBuffers::new() [gpu/buffers.rs]
          - Creates transform buffer
          - Creates palette texture
          - Creates accumulation textures (ping-pong)
          - Creates params UBO
      → Event loop starts
```

### Per-Frame Render Flow
```
WindowEvent::RedrawRequested
  → app.update()
    → metrics.update()  // FPS tracking

  → app.render()
    ┌─────────────────────────────────────────────────┐
    │ 1. COMPUTE PASS (trajectory.wgsl)               │
    │    - Generate random samples (128 workgroups)   │
    │    - Each thread: N iterations (e.g., 256)      │
    │    - Apply transforms + variations              │
    │    - Write color to temp texture                │
    └─────────────────────────────────────────────────┘
              ↓
    ┌─────────────────────────────────────────────────┐
    │ 2. ACCUMULATE PASS (accumulate.wgsl)            │
    │    - Read temp samples                          │
    │    - Blend with previous accumulation           │
    │    - blend_factor = 1.0 / sample_count          │
    │    - Write to current accumulation              │
    │    - Swap textures (ping-pong)                  │
    └─────────────────────────────────────────────────┘
              ↓
    ┌─────────────────────────────────────────────────┐
    │ 3. TONEMAP PASS (tonemap.wgsl)                  │
    │    - Read current accumulation                  │
    │    - Apply log tone mapping                     │
    │    - Palette lookup (if needed)                 │
    │    - Gamma correction                           │
    │    - Background blending                        │
    │    - Output to screen                           │
    └─────────────────────────────────────────────────┘
              ↓
    ┌─────────────────────────────────────────────────┐
    │ 4. UI PASS (egui)                               │
    │    - Render UI panels on top                    │
    │    - Return UiResponse with change flags        │
    └─────────────────────────────────────────────────┘
              ↓
    Handle UI responses
      - If flame_changed → update_flame()
      - If view_changed → update_iterations()
      - If palette_changed → update_palette()
      - If reset_requested → reset()
      - If undo/redo → import_config()
      - If config_export → save .flame file
      - If config_import → load .flame file
      - If palette_export → save .palette file
      - If palette_import → load .palette file
      - If export → capture_png()
```

### Event Handling Flow
```
User Input
  ↓
WindowEvent
  ↓
egui.handle_event() → consumed?
  ↓
If NOT consumed:
  - Mouse drag → pan_x/pan_y
  - Mouse wheel → zoom (toward cursor)
  - Arrow keys → pan_x/pan_y
  - +/- keys → zoom
  - Ctrl+Z → undo()
  - Ctrl+Y → redo()
  ↓
Set view_changed_by_keyboard flag
  ↓
Next frame: trigger reset() if changed
```

### State Change Flow
```
UI Change (e.g., edit transform)
  ↓
Set flame_changed = true
  ↓
Before applying change:
  app.capture_state()
    → undo_history.push(current_config)
  ↓
Apply change
  ↓
In render():
  if flame_changed:
    renderer.update_flame()
    renderer.reset() // Clear accumulation
```

---

## 💾 GPU Buffer Layout

### Bind Group 0 (Compute Pass)
```
@group(0) @binding(0) - transforms: array<GpuTransform>  (storage buffer, read)
@group(0) @binding(1) - params: GpuParams               (uniform buffer, read)
@group(0) @binding(2) - palette_texture: texture_1d     (texture, sample)
@group(0) @binding(3) - palette_sampler: sampler        (sampler)
@group(0) @binding(4) - temp_samples: texture_storage_2d (texture, write)
```

### Bind Group 0 (Accumulate Pass)
```
@group(0) @binding(0) - temp_samples: texture_2d         (texture, sample)
@group(0) @binding(1) - prev_accumulation: texture_2d    (texture, sample)
@group(0) @binding(2) - accumulation: texture_storage_2d (texture, write)
@group(0) @binding(3) - sampler_linear: sampler          (sampler)
@group(0) @binding(4) - params: AccumulateParams         (uniform buffer, read)
```

### Bind Group 0 (Tonemap Pass)
```
@group(0) @binding(0) - accumulation: texture_2d    (texture, sample)
@group(0) @binding(1) - palette: texture_1d         (texture, sample)
@group(0) @binding(2) - sampler_linear: sampler     (sampler)
@group(0) @binding(3) - params: TonemapParams       (uniform buffer, read)
```

---

## 🎨 GPU Data Structures

### GpuTransform (std140 layout)
```rust
struct GpuTransform {
    affine: mat2x2<f32>,    // 2x2 linear transform
    offset: vec2<f32>,      // Translation (e, f)
    weight: f32,            // Selection probability
    variations: [f32; 16],  // Variation weights
    color: vec3<f32>,       // RGB color
    color_speed: f32,       // Color blend factor
}
```

### GpuParams
```rust
struct GpuParams {
    num_transforms: u32,
    iterations_per_thread: u32,
    burn_in: u32,
    width: u32,
    height: u32,
    seed: u32,               // Random seed per frame
    color_mode: u32,         // 0=Transform, 1=Palette, 2=Speed
    splat_size: f32,
    zoom: f32,               // View transform
    pan_x: f32,
    pan_y: f32,
    rotation: f32,
    speed_factor: f32,       // Speed color blend
}
```

### TonemapParams
```rust
struct TonemapParams {
    exposure: f32,
    gamma: f32,
    density_scale: f32,      // Alpha multiplier
    background_color: vec3<f32>,
}
```

---

## 🔥 Flame Algorithm (GPU Implementation)

### Trajectory Shader Logic
```
1. Initialize RNG with unique seed:
   seed = params.seed + global_invocation_id

2. Generate random starting point:
   p = random_point_in_circle()

3. Burn-in iterations (discard):
   for i in 0..burn_in:
     p = iterate_flame(p)

4. Accumulation iterations:
   for i in 0..iterations_per_thread:
     // Select transform weighted by probability
     transform_idx = select_transform(rng)

     // Apply affine transformation
     p' = transform.affine * p + transform.offset

     // Apply variation functions
     p'' = sum(weight[i] * variation[i](p'))

     // Update color based on mode:
     if mode == Transform:
       color = blend(color, transform.color, transform.color_speed)
     elif mode == Palette:
       color = palette_lookup(color_index)
     elif mode == Speed:
       speed = length(p'' - p')
       color = palette_lookup(speed)

     // Project to screen space with view transform
     screen_pos = world_to_screen(p'', zoom, pan, rotation)

     // Write to texture (additive blend)
     if in_bounds(screen_pos):
       temp_samples[screen_pos] += vec4(color, 1.0)

     p = p''
```

### Accumulate Shader Logic
```
For each pixel:
  new_sample = temp_samples[pixel]
  prev_accum = prev_accumulation[pixel]

  // Exponential moving average
  blend_factor = 1.0 / samples_accumulated
  current = mix(prev_accum, new_sample, blend_factor)

  accumulation[pixel] = current
```

### Tonemap Shader Logic
```
For each pixel:
  accum = accumulation[pixel]

  // Log-scale tone mapping
  intensity = dot(accum.rgb, vec3(0.3, 0.59, 0.11))
  log_intensity = log(1.0 + intensity * exposure)
  scale = log_intensity / (intensity + 1e-6)

  color = accum.rgb * scale

  // Speed mode: lookup palette
  if color_mode == Speed:
    color = palette_lookup(color.r)

  // Gamma correction
  color = pow(color, vec3(1.0 / gamma))

  // Alpha blending with background
  alpha = accum.a * density_scale
  color = mix(background_color, color, alpha)

  return vec4(color, 1.0)
```

---

## 🔢 Key Constants

```rust
MAX_VARIATIONS = 16              // Number of variation functions
MAX_TRANSFORMS = 32              // Max transforms in a flame (buffer limit)
PALETTE_SIZE = 256               // 1D palette texture resolution
WORKGROUP_SIZE = 64              // 8x8 threads per workgroup
DEFAULT_ITERATIONS = 256         // Iterations per thread
DEFAULT_WORKGROUPS = 128         // Workgroups per dispatch
BURN_IN = 20                     // Initial iterations to discard
MAX_UNDO_HISTORY = 50            // Undo stack depth
```

---

## 🎯 Critical Code Paths

### Hot Path (Every Frame)
1. [app.rs:354-373](app.rs#L354-L373) - Compute + accumulate passes
2. [renderer/compute_kernel.rs:102-141](renderer/compute_kernel.rs#L102-L141) - Compute dispatch
3. [renderer/compute_kernel.rs:144-181](renderer/compute_kernel.rs#L144-L181) - Accumulate dispatch
4. [shaders/trajectory.wgsl:125-220](shaders/trajectory.wgsl#L125-L220) - Flame iteration loop
5. [shaders/tonemap.wgsl:50-100](shaders/tonemap.wgsl#L50-L100) - Tone mapping

### State Update Path
1. [ui/mod.rs:82-712](ui/mod.rs#L82-L712) - UI rendering + input
2. [app.rs:572-621](app.rs#L572-L621) - Handle UI responses
3. [renderer/compute_kernel.rs:207-232](renderer/compute_kernel.rs#L207-L232) - Update flame
4. [gpu/buffers.rs](gpu/buffers.rs) - Write GPU buffers

### Configuration Path
1. [config.rs:29-36](config.rs#L29-L36) - Serialize to JSON
2. [app.rs:630-644](app.rs#L630-L644) - Export config
3. [app.rs:647-680](app.rs#L647-680) - Import config
4. [undo.rs:19-31](undo.rs#L19-L31) - Push to undo stack

### PNG Export Path
**Transparent Export** (preserves alpha):
1. [compute_kernel.rs:351-453](src/renderer/compute_kernel.rs#L351-L453) - `capture_from_accumulation_buffer()`
2. Copy Rgba16Float accumulation buffer → CPU buffer
3. CPU reads f16 values, applies tone mapping (log + gamma)
4. Calculate alpha = density × density_scale
5. Convert to Rgba8 → PNG encoder

**Opaque Export** (background blended):
1. [compute_kernel.rs:455-543](src/renderer/compute_kernel.rs#L455-L543) - `capture_from_tonemap_render()`
2. Render via tonemap_pass() → temp Rgba8 texture
3. Copy texture → CPU buffer
4. PNG encoder

**Why dual paths?** The tonemap shader mixes RGB with background_color before outputting, so even though it outputs alpha, the RGB channels are already blended. For transparency, we need the raw accumulation buffer colors.

---

## 🐛 Common Modification Points

| Task | Files to Modify |
|------|-----------------|
| Add new variation | [transforms.rs](src/scene/transforms.rs), [trajectory.wgsl](shaders/trajectory.wgsl) |
| Change color algorithm | [trajectory.wgsl](shaders/trajectory.wgsl), [tonemap.wgsl](shaders/tonemap.wgsl) |
| Add UI panel | [ui/mod.rs](src/ui/mod.rs) |
| Add preset | [presets.rs](src/scene/presets.rs) |
| Modify accumulation | [accumulate.wgsl](shaders/accumulate.wgsl), [compute_kernel.rs](src/renderer/compute_kernel.rs) |
| Change tone mapping | [tonemap.wgsl](shaders/tonemap.wgsl) |
| Add export format | [compute_kernel.rs](src/renderer/compute_kernel.rs) |
| Modify GPU params | [buffers.rs](src/gpu/buffers.rs), corresponding shader |
| Add keyboard shortcut | [app.rs](src/app.rs) handle_keyboard() |
| Add built-in palette | [palette.rs](src/scene/palette.rs) |
| Import/export palette | Use Palette Editor UI (Added 2025-10-20) |

---

**Last Updated:** 2025-10-20
**Project:** fflame-rust
