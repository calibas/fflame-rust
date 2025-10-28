# Architecture Overview

Quick reference guide to understanding the codebase structure and data flow.

**Detailed Documentation:**
- [UI.md](main/UI.md) - Windows, panels, input handling, UiResponse system
- [BUFFERS.md](main/BUFFERS.md) - GPU layouts, bind groups, data structures
- [TRANSFORMS.md](main/TRANSFORMS.md) - Flame algorithm, affine math, IFS implementation
- [RENDERER.md](main/RENDERER.md) - 3-pass pipeline, FlameRenderer, PNG export

---

## 🏗️ Module Organization

```
fractal_flame_wgpu/
├── Core Entry Points
│   ├── main.rs                 Desktop entry → CLI parser + GUI/export mode
│   │                           - clap subcommand: export (headless batch PNG)
│   │                           - No args: launches GUI via lib::desktop_main()
│   │
│   └── lib.rs                  Library root + WASM entry
│                               - desktop_main() → GUI mode
│                               - export_mode() → headless batch export
│                               - export_async() → batch processing loop
│
├── Application Layer
│   ├── app/                    Main App module (refactored into 4 files)
│   │   ├── mod.rs              Core App struct (832 lines)
│   │   │                       - Event loop and window management
│   │   │                       - Render function + UI response handling
│   │   │                       - update() performance tracking
│   │   │
│   │   ├── input.rs            Input handlers (208 lines)
│   │   │                       - handle_keyboard() - Arrow keys, zoom, undo/redo
│   │   │                       - handle_mouse_button() - Drag state management
│   │   │                       - handle_mouse_move() - Rotation-aware panning
│   │   │                       - handle_mouse_wheel() - Zoom to cursor
│   │   │
│   │   ├── config.rs           Config management (122 lines)
│   │   │                       - export_config() → FractalConfig
│   │   │                       - import_config() + palette library sync
│   │   │                       - capture_state() for undo
│   │   │                       - undo/redo/can_undo/can_redo
│   │   │
│   │   └── export.rs           Headless export (126 lines)
│   │                           - export_headless() → CLI PNG export
│   │                           - Creates headless GPU instance
│   │                           - Renders to max_iterations
│   │                           - Embeds PNG metadata
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
│       │                       - Transform struct (affine + variations + variation_params + g field)
│       │                       - RenderMode (2D/3D) and ProjectionType
│       │                       - Point calculations (r, θ, φ)
│       │                       - Flame struct (transform collection + name + 3D settings)
│       │                       - CPU iteration reference (2D only)
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
│       ├── palette.rs          Color system
│       │                       - ColorMode enum (Transform/Palette/Speed)
│       │                       - ColorStop gradient system
│       │                       - Palette with interpolation
│       │                       - PaletteLibrary (auto-loads from assets/)
│       │
│       └── variations/         Variation system
│           ├── mod.rs          VariationRegistry (global singleton)
│           │                   - VariationInfo (name, display_name, category, parameters)
│           │                   - VariationParameter (name, type, default, min/max)
│           │                   - ParamType enum (Float, Integer, Angle)
│           │                   - Registration system for all 26 variations
│           │                   - ordered_names: Vec<String> (defines numerical IDs)
│           │                   - global_registry() function for singleton access
│           │
│           └── (future)        Plugin variations (wgsl + metadata)
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
│       │                       - Compute pipeline (trajectory 2D)
│       │                       - Compute pipeline (trajectory 3D)
│       │                       - Compute pipeline (accumulate)
│       │                       - Render pipeline (tonemap)
│       │                       - Bind group layouts
│       │                       - Runtime pipeline selection based on render mode
│       │
│       └── buffers.rs          FlameBuffers + GPU data structures
│                               - Transform buffer (storage, 32 slots)
│                               - Variation params buffer (storage, 400 floats: 50 variations × 8 params)
│                               - Histogram buffer (storage, 4× u32 per pixel for atomic color accumulation)
│                               - Palette texture (1D)
│                               - Params uniform buffers
│                               - Accumulation textures (ping-pong)
│                               - GpuTransform, GpuParams, TonemapParams, GpuVariationParams
│
├── Renderer Layer
│   └── renderer/
│       ├── mod.rs              Module exports
│       │
│       └── compute_kernel.rs  🎨 RENDERING CORE
│                               FlameRenderer
│                               - Manages rendering state
│                               - Orchestrates GPU passes:
│                                 1. compute_pass() - generate samples (2D or 3D shader)
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
├── Shader Generation
│   └── shader_builder_v2.rs   ShaderBuilder
│                               - Dynamic WGSL generation based on active variations
│                               - Detects variation needs (RNG, parameters)
│                               - Generates correct function signatures
│                               - Builds apply_variations() switch statement
│                               - Separate builders for 2D and 3D modes
│                               - Enables variation plugins (future)
│
├── State Management
│   ├── config.rs               FractalConfig (complete state)
│   │                           - Flame (transforms, variations, params)
│   │                           - View (zoom, pan, rotation, camera)
│   │                           - Rendering (density, speed, max_iterations)
│   │                           - Colors (mode, palette_index, palette data, background)
│   │                           - Tone mapping (mode, curve, use_curve, exposure, gamma)
│   │                           - Reproducibility (deterministic_rng)
│   │                           - JSON import/export (.fflame files)
│   │                           - File save/load with full metadata
│   │
│   ├── undo.rs                 UndoHistory
│   │                           - 50-state circular buffer
│   │                           - Undo/redo tracking
│   │
│   └── png_metadata.rs         PNG metadata embedding
│                               - PngMetadata struct (build, render, config)
│                               - encode_png_with_metadata() → tEXt chunks
│                               - read_png_metadata() → extract from PNG
│                               - SHA256 checksum of config JSON
│
├── Profiling & Version Tracking
│   ├── profiler.rs             GpuProfiler + CPU timing
│   │                           - GPU timestamp queries (TIMESTAMP_QUERY feature)
│   │                           - CPU scope timing with RAII
│   │                           - FrameProfile with version metadata
│   │                           - JSON export for performance data
│   │
│   └── version.rs              VersionInfo
│                               - Version from Cargo.toml
│                               - Auto-incrementing build numbers
│                               - Git hash, branch, build time
│                               - Platform and architecture info
│                               - Global singleton via get_version_info()
│
├── Testing & Benchmarking
│   ├── tests/regression.rs    12 regression tests
│   │                           - CPU iteration determinism
│   │                           - All 26 variation functions
│   │                           - Preset validation
│   │                           - Serialization round-trips
│   │
│   ├── benches/flame_bench.rs Criterion benchmarks
│   │                           - Statistical microbenchmarking
│   │                           - CPU iteration performance
│   │                           - Individual variation functions
│   │
│   └── bin/simple_benchmark.rs CLI benchmark tool
│                               - Human-readable performance testing
│                               - Tests all presets and variations
│                               - M ops/sec output
│
└── Shaders (WGSL) - Dynamic Compilation System
    ├── core/                   🔥 MODULAR COMPONENTS (assembled at runtime)
    │   ├── header.wgsl         Bindings and data structures (66 lines)
    │   │                       - Transform, DispatchParams, VariationParams
    │   │                       - All bind group layouts
    │   │
    │   ├── rng.wgsl            RNG functions (34 lines)
    │   │                       - PCG random number generator
    │   │
    │   ├── affine.wgsl         Affine transform (9 lines)
    │   │
    │   ├── variations_2d.wgsl  2D variation functions (152 lines)
    │   │                       - Core 2D variations (0-15)
    │   │                       - Parameterized 2D (JuliaN, Blob)
    │   │
    │   ├── variations_3d.wgsl  All variations including 3D (202 lines)
    │   │                       - All 2D variations PLUS
    │   │                       - 3D depth variations (Zcone, Flatten, ZScale)
    │   │                       - 3D rotation variations (PreRotate, PostRotate)
    │   │                       - 3D full variations (Hemisphere)
    │   │
    │   ├── utilities.wgsl      Helper functions (135 lines)
    │   │                       - get_param() - Parameter access
    │   │                       - world_to_pixel() - Camera and projection
    │   │                       - Point calculations (r, θ, φ)
    │   │
    │   ├── main_2d.wgsl        2D entry point (75 lines)
    │   │                       - @compute main function
    │   │                       - Calls apply_variations() [GENERATED]
    │   │
    │   └── main_3d.wgsl        3D entry point (76 lines)
    │                           - @compute main function
    │                           - Calls apply_variations() [GENERATED]
    │
    ├── [GENERATED at runtime]  Trajectory shaders built by ShaderBuilder
    │                           - 2D: header + rng + variations_2d + [generated apply_variations] + utilities + main_2d
    │                           - 3D: header + rng + variations_3d + [generated apply_variations] + utilities + main_3d
    │                           - Only active variations compiled
    │                           - Conditional RNG and parameter passing
    │                           - Supports plugin variation injection
    │
    ├── accumulate.wgsl         🔥 COMPUTE: Temporal blending (41 lines)
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

**See [RENDERER.md](main/RENDERER.md)** for detailed pipeline documentation.

**Quick overview:**
```
1. Compute Pass → Generate samples (write to histogram)
2. Accumulate Pass → Blend with history (ping-pong buffers)
3. Tonemap Pass → Display rendering (log/linear + gamma)
4. UI Pass → Render egui panels
5. Handle UI responses → Update state
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

**See [BUFFERS.md](main/BUFFERS.md)** for complete buffer documentation including:
- Bind group layouts (Compute, Accumulate, Tonemap)
- GPU data structures (GpuTransform, GpuParams, TonemapParams, AccumulateParams)
- Memory layout rules (std140 vs std430)
- Buffer update patterns
- Common modification tasks

**Quick reference - Bind Group 0 (Compute Pass):**
```
@group(0) @binding(0) - transforms: array<GpuTransform>      (storage buffer, read)
@group(0) @binding(1) - params: GpuParams                   (uniform buffer, read)
@group(0) @binding(2) - histogram: array<atomic<u32>>       (storage buffer, read_write)
@group(0) @binding(3) - palette_texture: texture_1d         (texture, sample)
@group(0) @binding(4) - palette_sampler: sampler            (sampler)
@group(0) @binding(5) - variation_params: array<VariationParams> (storage buffer, read)
```


---

## 🎨 Histogram Color Accumulation System (Added 2025-10-27)

### Overview
The renderer uses a **histogram-based atomic accumulation** system to safely collect color data from thousands of parallel GPU threads. This replaced the previous direct texture writes which couldn't safely handle concurrent access.

### Architecture

**3-Stage Pipeline:**
```
1. Compute Pass (main_2d.wgsl / main_3d.wgsl)
   - Each thread generates 256-1024 iterations
   - Converts final_color (RGB f32) to u32 with fixed scale
   - Atomically adds to histogram buffer
   - Atomically increments iteration_counts per pixel (for convergence tracking)

2. Accumulate Pass (accumulate.wgsl)
   - Reads histogram buffer (non-atomic)
   - Reads iteration_counts per pixel
   - Decodes u32 back to f32 RGB
   - Applies accumulation controls: low-density smoothing, density compression, iteration limiting
   - Blends with previous accumulation using adjusted blend factor
   - Clears histogram for next frame (iteration_counts persist)

3. Tonemap Pass (tonemap.wgsl)
   - Reads accumulation texture
   - Applies tone mapping, gamma, background blending
   - Outputs to screen
```

### Histogram Format (U32 Unpacked - Current)

**Layout:** 4× u32 per pixel (separate R, G, B, Density channels)
```
Pixel Index: i = y * width + x
Base Index:  base = i * 4

histogram[base + 0] = R (u32, 0 to 4,294,967,295)
histogram[base + 1] = G (u32, 0 to 4,294,967,295)
histogram[base + 2] = B (u32, 0 to 4,294,967,295)
histogram[base + 3] = Density (u32, count of hits)
```

**Memory Usage:** `width × height × 4 × 4 bytes`
- 1920×1080: ~31.5 MB
- 800×600: ~9.2 MB

**Encoding (Compute Shader):**
```wgsl
let color_scale = params.histogram_color_scale;  // Default: 100.0

let r_u32 = u32(clamp(final_color.r, 0.0, 1.0) * color_scale);
let g_u32 = u32(clamp(final_color.g, 0.0, 1.0) * color_scale);
let b_u32 = u32(clamp(final_color.b, 0.0, 1.0) * color_scale);
let density_u32 = u32(color_scale);

atomicAdd(&histogram[base_idx + 0u], r_u32);
atomicAdd(&histogram[base_idx + 1u], g_u32);
atomicAdd(&histogram[base_idx + 2u], b_u32);
atomicAdd(&histogram[base_idx + 3u], density_u32);
```

**Decoding (Accumulate Shader):**
```wgsl
let r_sum = f32(histogram[base_idx + 0u]);
let g_sum = f32(histogram[base_idx + 1u]);
let b_sum = f32(histogram[base_idx + 2u]);
let density = f32(histogram[base_idx + 3u]);

let color = vec3(r_sum, g_sum, b_sum) / (density + 1e-6);
```

### Evolution History

**Original (Removed):** Direct texture writes
- Used `textureStore()` to write colors directly
- **Problem:** Race conditions with concurrent writes (undefined behavior per WebGPU spec)
- **Symptom:** Visual artifacts, incorrect colors

**First Histogram (2025-10-26):** U16 packed RGB + U32 density
- Format: `[u32: (R16|G16), u32: B16, u32: density]` (3× u32 per pixel)
- **Problem:** RGB channels overflow after ~1,310 hits at scale=50
- **Symptom:** Bright areas wrap to dark colors (0xFFFF → 0x0000)
- Capacity: 65,535 max value per channel

**Second Histogram (2025-10-27):** U32 unpacked (current)
- Format: `[R32, G32, B32, density32]` (4× u32 per pixel)
- **Benefit:** Eliminates overflow - 4.2 billion max value per channel
- **Tradeoff:** 33% larger memory footprint (3→4 words)
- Capacity: 42.9M hits before overflow (at scale=100) = 91 minutes continuous rendering

### Performance Characteristics

**Benchmark Results (simple3 @ 1920×1080, 1024 iters/thread):**
```
Commit    Description                  Time (ms)  Throughput (Giter/s)
------    -----------                  ---------  --------------------
dd80003   textureStore (baseline)      ~6800      ~5.86  [had race conditions]
9ac278a   u16 packed histogram         1570       25.36  [overflow issues]
a8301de   u32 unpacked histogram       1607       24.76  [current, no overflow]
```

**Performance vs Baseline:**
- U32 histogram: **2.4% slower** than u16 packed (acceptable tradeoff)
- 76% of memory bandwidth: 4 words vs 3 words, but better cache locality

**Why Acceptable:**
- Eliminates visual artifacts (overflow wraparound)
- Proper HDR behavior (bright areas stay bright)
- Clean, maintainable codebase (no complex workarounds)
- Future-proof for high iteration counts

### Key Design Decisions

**Why U32 instead of F32?**
- Atomic operations on f32 are undefined in WGSL/WebGPU
- Integer atomics are guaranteed to be safe and correct
- Scale factor provides adequate precision for color accumulation

**Why Global Scale instead of Per-Pixel Adaptive?**
- Simpler implementation (single uniform constant)
- Faster access (uniform vs storage buffer read)
- Eliminated 1.9 MB scale_buffer overhead
- Avoids complex convergence detection logic

**Why Separate Density Channel?**
- Allows correct averaging: `color = sum / density`
- Preserves HDR information for tone mapping
- Matches traditional flame renderer architecture

### UI Control

**Location:** Settings window → Rendering section → "Histogram Color Scale" slider
- Range: 1.0 to 1000.0
- Default: 100.0
- Higher values: Better precision, faster overflow (not an issue with u32)
- Lower values: Less precision, more headroom (unnecessary with u32)

### Related Documentation
- [HISTOGRAM_OPTIMIZATION_ATTEMPTS.md](HISTOGRAM_OPTIMIZATION_ATTEMPTS.md) - Failed optimization attempts
- [PER_PIXEL_ADAPTIVE_SCALING_DEBUG.md](PER_PIXEL_ADAPTIVE_SCALING_DEBUG.md) - Why adaptive scaling was abandoned
- [U32_HISTOGRAM_CLEANUP.md](U32_HISTOGRAM_CLEANUP.md) - Cleanup plan after u32 implementation

---

## 🔥 Flame Algorithm

**See [TRANSFORMS.md](main/TRANSFORMS.md)** for complete flame algorithm documentation including:
- What is a fractal flame (IFS explanation)
- Transform structure (affine + variations + color)
- Flame algorithm (CPU reference + GPU implementation)
- Point calculations (r, θ, φ)
- Render modes (2D vs 3D)
- Transform selection (weighted random)
- Variation blending (additive)
- Color modes (Transform, Palette, Speed)
- View transformation (world to screen)

**Quick reference - Algorithm overview:**

```
1. Random starting point → Burn-in (settle) → Accumulation loop:
   - Select transform (weighted random)
   - Apply affine (2D matrix + translation, + Z offset in 3D)
   - Apply variations (additive blending)
   - Update color (Transform/Palette/Speed mode)
   - Project to screen (view transform + camera rotation in 3D)
   - Write to histogram (atomic u32 accumulation)
```

---

## 🔢 Key Constants

```rust
MAX_VARIATIONS = 24              // Number of variation slots (16 basic 2D + 8 3D)
MAX_PARAMS_PER_VARIATION = 8     // Parameter slots per variation
MAX_TRANSFORMS = 32              // Max transforms in a flame (buffer limit)
PALETTE_SIZE = 256               // 1D palette texture resolution
WORKGROUP_SIZE = 64              // 8x8 threads per workgroup
DEFAULT_ITERATIONS = 256         // Iterations per thread
DEFAULT_WORKGROUPS = 128         // Workgroups per dispatch
BURN_IN = 20                     // Initial iterations to discard
MAX_UNDO_HISTORY = 50            // Undo stack depth
```

---

## 🖼️ UI Organization

**See [UI.md](main/UI.md)** for complete UI documentation including:
- Window layout (5 windows + menu bar)
- All panels and controls
- Input handling (keyboard, mouse, wheel)
- UiResponse system
- Common UI modification tasks

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
| Add new 2D variation | [variations/mod.rs](src/variations/mod.rs), [variations_2d.wgsl](shaders/core/variations_2d.wgsl), [variations_3d.wgsl](shaders/core/variations_3d.wgsl) |
| Add new 3D variation | [variations/mod.rs](src/variations/mod.rs), [variations_3d.wgsl](shaders/core/variations_3d.wgsl) |
| Add variation parameters | [variations/mod.rs](src/variations/mod.rs) - use `registry.add_parameters()` |
| Change color algorithm | [main_2d.wgsl](shaders/core/main_2d.wgsl), [main_3d.wgsl](shaders/core/main_3d.wgsl), [tonemap.wgsl](shaders/tonemap.wgsl) |
| Add UI panel/window | [ui/mod.rs](src/ui/mod.rs) - add to menu bar and window rendering |
| Add preset | [presets.rs](src/scene/presets.rs) or create `.fflame` file in `assets/presets/` |
| Modify accumulation | [accumulate.wgsl](shaders/accumulate.wgsl), [compute_kernel.rs](src/renderer/compute_kernel.rs) |
| Change tone mapping | [tonemap.wgsl](shaders/tonemap.wgsl) |
| Add export format | [compute_kernel.rs](src/renderer/compute_kernel.rs) |
| Modify GPU params | [buffers.rs](src/gpu/buffers.rs), corresponding shader module |
| Add keyboard shortcut | [app.rs](src/app.rs) handle_keyboard() |
| Add built-in palette | [palette.rs](src/scene/palette.rs) or create `.palette` file in `assets/palettes/` |
| Import/export palette | Use Palette Editor UI (Added 2025-10-20) |
| Add/delete transforms | Use "➕ Add Transform" / "🗑 Delete Transform" buttons (Added 2025-10-20) |
| Edit transform visually | Use Triangle Editor window (Added 2025-10-21) |
| Toggle window visibility | Use "View" menu in menu bar (Added 2025-10-21) |
| Run unit tests | `cargo test` |
| Run regression tests | `cargo test --test regression` |
| Run benchmarks | `cargo bench` |
| Run CLI benchmark | `cargo run --bin simple_benchmark --release` |
| Display version info | `cargo run --example show_version` |
| Export presets | `cargo run --example export_presets` |

---

## ⚡ Speed Multiplier System (Added 2025-10-25)

### Overview
The speed multiplier system decouples **rendering throughput** from **accumulation quality**, solving a fundamental quality degradation issue at high `iterations_per_thread` settings.

### The Problem
- **Observation**: High `iterations_per_thread` (4096) causes 60-70% visual quality degradation vs baseline (256)
- **Root Cause**: Fewer accumulation passes → large density jumps → sqrt() tone mapping artifacts
- **Impact**: Progressive rendering looks "chunky" instead of smooth
- **Critical Issue**: Would cause temporal flickering in animations with adaptive iteration counts

### The Solution: Two Approaches

#### 1. Interactive App (Frame Rate Control)
**Location**: [src/app/mod.rs](../src/app/mod.rs#L228-L256)

**Mechanism**:
```rust
// Speed multiplier controls target FPS
let target_fps = 60.0 * speed_multiplier as f64;

// Frame rate limiter using ControlFlow::WaitUntil
if elapsed >= target_frame_time {
    window.request_redraw();  // Time for next frame
} else {
    set_control_flow(WaitUntil(next_frame_time));  // Sleep until deadline
}
```

**How it works**:
- Each frame does ONE full compute+accumulate at full `iterations_per_thread`
- Speed multiplier increases frame rate: 1x=60fps, 2x=120fps, ... 16x=960fps
- More frames per second → more accumulation passes per second
- When idle (paused/finished): Falls back to 60 FPS to save CPU
- PresentMode: `Mailbox` for smooth uncapped rendering

**UI Controls**: Settings window → Speed selector (1x/2x/4x/8x/16x buttons)

#### 2. CLI Export (Explicit Chunking)
**Location**: [src/app/export.rs](../src/app/export.rs#L72-L107)

**Mechanism**:
```rust
// Calculate iterations per chunk
let iterations_per_frame = iterations_per_thread / speed_multiplier;

// Render multiple chunks per "batch"
for _ in 0..speed_multiplier {
    compute_pass(iterations_per_frame, ...);  // Smaller chunk
    accumulate_pass(samples_this_chunk);      // Accumulate immediately
    // Ping-pong buffers swap - chunk is a complete frame
}
```

**How it works**:
- No frame rate concept in headless export
- Speed multiplier chunks iterations into smaller batches
- Each chunk: compute → accumulate → swap (complete frame cycle)
- Example: 4096 iters with 16x speed = 16 chunks of 256 iterations each

**CLI Usage**: `--speed-multiplier` parameter
```bash
fractal_flame_wgpu export -i config.fflame -o output.png \
  --iterations-per-thread 4096 --speed-multiplier 16
```

### Verification
**Test Results**: Pixel-perfect identical (PSNR = inf, SSIM = 1.0)
- 256 iters (1x speed) vs 4096 iters (16x speed): 0.00% pixel difference
- Formula for equivalent quality: `speed_multiplier = iterations_per_thread / 256`

### Architecture Impact

**Data Flow Changes**:
```
Before:
  iterations_per_thread=4096 → 1 compute(4096) → 1 accumulate → chunky quality

After (Interactive):
  speed_multiplier=16x → 16 frames/sec at 960fps
  Each frame: compute(4096) → accumulate → smooth quality

After (Export):
  speed_multiplier=16 → 16 chunks of 256 iters
  Each chunk: compute(256) → accumulate → smooth quality
```

**Key Design Principles**:
1. **Orthogonal Concerns**: Throughput (`iterations_per_thread`) vs Quality (`speed_multiplier`)
2. **Context-Appropriate**: Frame rate for interactive, chunking for export
3. **No Shader Changes**: All logic is CPU-side
4. **Animation-Ready**: Foundation for future adaptive rendering with consistent quality

### Performance Characteristics

**Interactive App**:
- Higher frame rate = more frequent screen updates
- Each frame has full `iterations_per_thread` work
- Total throughput unchanged, distributed over more frames
- GPU utilization: Spiky (brief compute bursts at high FPS)

**Export**:
- More chunks per batch = more accumulate passes
- Each chunk has smaller `iterations_per_frame` work
- Total throughput unchanged, same time to completion
- GPU utilization: Sustained (continuous compute+accumulate)

### Future: Animation System
The speed multiplier enables **adaptive rendering** with **consistent quality**:
```rust
for frame in animation {
    let iterations = calculate_adaptive_iterations(frame.motion);
    let speed = iterations / base_iterations;  // Maintain constant accumulation frequency
    render_frame(iterations_per_thread: iterations, speed_multiplier: speed);
}
```
Result: Variable throughput based on motion, but constant visual quality.

### References
- Complete analysis: [ITERATIONS_PER_THREAD_QUALITY.md](ITERATIONS_PER_THREAD_QUALITY.md)
- Root cause investigation: 4 failed attempts documented
- Implementation details: Frame rate control vs iteration chunking

---

**Last Updated:** 2025-10-27
**Project:** fflame-rust

**Major Recent Changes:**
- **U32 histogram color accumulation** for overflow-free rendering (2025-10-27)
  - Atomic u32 accumulation eliminates RGB overflow artifacts
  - 4× u32 per pixel: separate R, G, B, Density channels
  - 2.4% performance cost vs u16 packed, but eliminates visual artifacts
  - Cleaned up all failed optimization attempts (per-pixel adaptive scaling, convergence masking)
- **Speed multiplier system** for quality-independent throughput control (2025-10-25)
  - Frame rate control (interactive app) and iteration chunking (export)
  - Pixel-perfect quality at any iterations_per_thread setting
  - Foundation for future animation system
- Variation registry system with parameters (2025-10-22)
- Dynamic shader generation via ShaderBuilder v2 (2025-10-21)
- Triangle editor with visual transform editing (2025-10-21)
- UI refactoring into 5 windows with menu bar (2025-10-21)
- 26 total variations (16 basic 2D + 8 3D + 2 parameterized)
