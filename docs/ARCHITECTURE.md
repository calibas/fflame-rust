# Architecture Overview

Quick reference guide to understanding the codebase structure and data flow.

**Detailed Documentation:**
- [UI.md](main/UI.md) - Windows, panels, input handling, UiResponse system
- [BUFFERS.md](main/BUFFERS.md) - GPU layouts, bind groups, data structures
- [TRANSFORMS.md](main/TRANSFORMS.md) - Flame algorithm, affine math, IFS implementation
- [RENDERER.md](main/RENDERER.md) - 3-pass pipeline, FlameRenderer, PNG export
- [SHADERS.md](main/SHADERS.md) - WGSL modular system, ShaderBuilder, dynamic compilation
- [VARIATIONS.md](main/VARIATIONS.md) - Variation registry, all 26 core variations, parameters
- [COLOR.md](main/COLOR.md) - Color modes, palette system, histogram accumulation
- [CONFIG.md](main/CONFIG.md) - FractalConfig, presets, undo/redo, serialization
- [EXPORT.md](main/EXPORT.md) - PNG export (transparent/opaque), metadata, CLI batch mode
- [PRESET-BROWSER.md](main/PRESET-BROWSER.md) - Gallery UI system for browsing fractals
- [TESTING-GUIDE.md](TESTING-GUIDE.md) - Unit tests, regression tests, benchmarks, profiling

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
├── UI Layer (egui + egui_dock - Migrated 2025-11-13)
│   ├── ui/
│   │   ├── mod.rs              EguiLayer + main UI coordinator
│   │   │                       - DockArea integration
│   │   │                       - Fractal texture management
│   │   │                       - Panel rendering dispatcher
│   │   │
│   │   ├── workspace.rs        Docking layout management
│   │   │                       - Workspace struct with DockState
│   │   │                       - Panel tabs (Settings, Transforms, View, etc.)
│   │   │                       - Default layout configuration
│   │   │                       - Future: Save/restore layouts
│   │   │
│   │   ├── settings.rs         Settings panel
│   │   │                       - File & Project (presets, undo/redo)
│   │   │                       - Rendering controls
│   │   │                       - Export options
│   │   │                       - Preferences (language selector)
│   │   │
│   │   ├── transforms.rs       Transform editor panel
│   │   │                       - Transform list with controls
│   │   │                       - Add/delete transforms
│   │   │                       - Affine parameters
│   │   │
│   │   ├── triangle_editor.rs  Visual triangle editor panel
│   │   │                       - Interactive affine editing
│   │   │                       - Drag handles for pre/post triangles
│   │   │                       - Batch updates via ConfigManager
│   │   │
│   │   ├── view.rs             View controls panel
│   │   │                       - Zoom, pan, rotation
│   │   │                       - 3D camera controls
│   │   │                       - Projection settings
│   │   │
│   │   ├── tone_mapping.rs     Tone mapping & color panel
│   │   │                       - Color mode selection
│   │   │                       - Palette controls
│   │   │                       - Tone curve settings
│   │   │                       - Background color
│   │   │
│   │   ├── palette_editor.rs   Palette editor panel
│   │   │                       - Color stop editing
│   │   │                       - Palette import/export
│   │   │                       - Gradient preview
│   │   │
│   │   ├── undo_history.rs     Undo history browser panel
│   │   │                       - Visual state preview
│   │   │                       - Jump to any config state
│   │   │                       - ConfigManager integration
│   │   │
│   │   ├── random_generator.rs Random Generator panel (Added PR #40)
│   │   │                       - Configurable flame generation settings
│   │   │                       - Symmetry options (bilateral, rotational, dihedral)
│   │   │                       - Batch generation with File Browser integration
│   │   │
│   │   ├── menu_bar.rs         Top menu bar
│   │   │                       - File, Edit, View, Fractal, Rendering, Window, Help
│   │   │                       - Keyboard shortcuts documented
│   │   │                       - Future: Full menu action implementation
│   │   │
│   │   ├── variation_controls.rs  Variation weight UI
│   │   ├── variation_params.rs    Variation parameter UI
│   │   ├── performance.rs         Performance metrics display
│   │   ├── config_dialog.rs       Config import/export dialog
│   │   ├── help.rs                Help/about dialogs
│   │   ├── helpers.rs             UI utility functions
│   │   ├── formatting.rs          Number formatting helpers
│   │   └── response.rs            UiResponse struct (legacy)
│   │
│   └── i18n.rs                 Internationalization (Added 2025-11-13)
│                               - rust-i18n integration
│                               - Locale management (current_locale, set_locale)
│                               - LocaleInfo struct for UI display
│                               - Translation macro re-exports
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
│       ├── randomize.rs        Random flame generation (Extended PR #40)
│       │                       - RandomGeneratorSettings struct
│       │                       - SymmetryType enum (None/Bilateral/Rotational/Dihedral)
│       │                       - generate_random_flame_with_settings()
│       │                       - generate_batch() for exploration
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
├── State Management - **See [CONFIG.md](main/CONFIG.md)** for complete documentation
│   ├── config/
│   │   ├── mod.rs              Module exports and re-exports
│   │   ├── defaults.rs         Default value constants (single source of truth)
│   │   ├── fractal_config.rs   FractalConfig (per-fractal artistic state), JSON serialization
│   │   ├── delta.rs            ConfigPath, ConfigValue, ConfigDelta enums
│   │   ├── manager.rs          ConfigManager (unified state, undo/redo, coalescing)
│   │   │                       - Manages FractalConfig (undo/redo enabled)
│   │   │                       - Manages SystemSettings (no undo, persistent)
│   │   │                       - update_param() for fractal changes
│   │   │                       - update_system_setting() for device settings
│   │   │                       - Returns UpdateType for GPU synchronization
│   │   └── slider.rs           Slider/DragValue UI helpers
│   ├── storage/                Local storage system (Added 2025-11-23, PR #27)
│   │   ├── mod.rs              Module exports
│   │   ├── settings.rs         SystemSettings struct (device-specific settings)
│   │   │                       - VSync, target FPS, iterations per thread
│   │   │                       - Language preference, export defaults
│   │   │                       - Recent files (desktop only)
│   │   └── backend.rs          Cross-platform storage implementation
│   │                           - Desktop: JSON files in user data directory
│   │                           - WASM: browser localStorage
│   │                           - StorageBackend trait for platform abstraction
│   │
│   ├── resources/              HTTP resource fetching (Added PR #39)
│   │   ├── mod.rs              Core types (LoadState, PalettePackInfo, ResourceManifest)
│   │   ├── fetch.rs            Platform-specific fetch
│   │   │                       - Desktop: filesystem read
│   │   │                       - WASM: fetch API with async/await
│   │   ├── palettes.rs         Palette pack loading with manifest
│   │   │                       - Lazy loading of large packs (701 Apophysis palettes)
│   │   │                       - Auto-load enabled packs on startup
│   │   └── error.rs            FetchError type for cross-platform errors
│   │
│   └── png_metadata.rs         PNG metadata embedding, tEXt chunks
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
├── Testing & Benchmarking - **See [TESTING-GUIDE.md](TESTING-GUIDE.md)** for complete guide
│   ├── tests/regression.rs         12 regression tests (CPU determinism, variations, presets)
│   ├── tests/visual/               Visual regression testing (desktop + WASM)
│   │   ├── run_all_tests.py        Unified test runner (desktop + WASM + performance tracking)
│   │   ├── run_tests.py            Desktop CLI visual tests (pixel-perfect comparison)
│   │   ├── wasm/test_wasm.py       WASM browser tests (Playwright automation)
│   │   ├── configs/                Test configurations (.fflame files)
│   │   ├── baseline/               Reference images (desktop + wasm/)
│   │   ├── current/                Test outputs (desktop + wasm/)
│   │   └── performance_history.csv Performance tracking over time
│   ├── benches/flame_bench.rs      Criterion benchmarks (statistical microbenchmarking)
│   └── bin/simple_benchmark.rs     CLI benchmark tool (human-readable performance)
│
└── Shaders (WGSL) - **See [SHADERS.md](main/SHADERS.md)** for complete shader documentation
    ├── core/                       Modular components (assembled by ShaderBuilder)
    │   ├── header.wgsl             Bind groups and structs (interactive)
    │   ├── header_export.wgsl      Header for headless export
    │   ├── header_tiled.wgsl       Header for high-res tiled rendering
    │   ├── rng.wgsl                PCG random number generator
    │   ├── utilities.wgsl          Helper functions (r/θ/φ, projection)
    │   ├── utilities_tiled.wgsl    Utilities for tiled rendering
    │   ├── affine.wgsl             2D affine transform
    │   ├── affine_3d.wgsl          3D affine transform with Z
    │   ├── main_template.wgsl      Main compute shader ({{VARIATIONS_CODE}} placeholder)
    │   ├── main_2d_export.wgsl     2D export entry point
    │   ├── main_3d_export.wgsl     3D export entry point
    │   ├── main_2d_tiled.wgsl      2D high-res tiled entry point
    │   ├── main_3d_tiled.wgsl      3D high-res tiled entry point
    │   └── path_filter.wgsl        Path filtering for density estimation
    │   (Note: Variation functions generated dynamically, not stored as files)
    │
    ├── ShaderBuilder               Dynamic compilation (only active variations)
    ├── accumulate.wgsl             Ping-pong progressive refinement
    └── tonemap.wgsl                Display rendering pass
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

### State Change Flow (Delta-Based System)

**See [CONFIG.md](main/CONFIG.md)** for complete ConfigManager documentation.

**Modern Flow (Delta-Based):**
```
UI Change (e.g., edit transform parameter)
  ↓
Slider/control binds to ConfigManager via helper:
  config_slider(ui, config_manager, ConfigPath::TransformAffine { index, param })
  ↓
On value change:
  1. ConfigManager computes delta (old value vs new value)
  2. LazyUndoHelper throttles undo captures (500ms minimum between captures)
  3. Applies change to active_config
  4. Returns UpdateType (View/Color/Flame/etc.)
  ↓
UI returns UpdateType to App
  ↓
App handles UpdateType:
  - View: renderer.update_view() → reset()
  - Color: renderer.update_palette() → reset()
  - Flame: renderer.update_flame() → reset()
  - ToneMap: renderer.update_tonemap() (no reset)
  ↓
ConfigManager maintains undo/redo stack:
  - Undo: ConfigManager.undo() → App applies deltas
  - Redo: ConfigManager.redo() → App applies deltas
```

**Legacy Flow (Flag-Based, deprecated):**
```
UI Change → Set flags → capture_state() → Apply changes
(Still used for some operations like preset loading, being phased out)
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
1. Compute Pass (main_template.wgsl - dynamically compiled)
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
- [archive/histogram/](archive/histogram/) - Complete histogram evolution and optimization attempts (15 historical docs)
  - HISTOGRAM_FINAL.md - Complete evolution timeline
  - HISTOGRAM_OPTIMIZATION_ATTEMPTS.md - Failed optimization attempts
  - U32_HISTOGRAM_CLEANUP.md - Cleanup plan (completed)

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

## 🖼️ UI Organization (egui_dock - Migrated 2025-11-13)

**Docking System:**
- Migrated from fixed side panel to flexible docking layout using egui_dock
- All windows converted to dockable panels (1:1 mapping)
- Users can rearrange, detach, and dock panels anywhere
- Future: Save/restore workspace layouts

**7 Main Panels:**
1. **Fractal Viewport** - Main rendering display (center, always visible)
2. **Settings** - File operations, rendering controls, preferences (with language selector)
3. **Transforms** - Transform list, add/delete, affine parameters
4. **Triangle Editor** - Visual affine editing with interactive triangles
5. **View** - Camera controls, zoom, pan, rotation
6. **Tone Mapping & Colors** - Color mode, palette, tone mapping settings
7. **History** - Visual undo/redo browser with state preview

**Menu Bar:**
- Top-level menus: File, Edit, View, Fractal, Rendering, Window, Help
- Professional menu structure for feature discoverability
- Keyboard shortcuts documented in menus
- Future: Implement all menu actions

**Internationalization (Added 2025-11-13):**
- rust-i18n v3.1 with YAML translation files
- Language selector in Settings → Preferences
- English (en) complete with 200+ strings
- Ready for community translations (Spanish, French, German, Japanese, Chinese)
- See [I18N.md](main/I18N.md) for translation guide

**See [UI.md](main/UI.md)** for complete UI documentation including:
- Panel descriptions and controls
- Input handling (keyboard, mouse, wheel)
- UiResponse system (legacy)
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

**See [EXPORT.md](main/EXPORT.md)** for complete PNG export documentation.

**Quick overview:** Dual export paths for different use cases:
- **Transparent:** Read from accumulation buffer, apply CPU tone mapping, preserve alpha
- **Opaque:** Render via tonemap shader, background pre-blended, faster
- **Metadata:** All PNGs include build info, config JSON, render stats in tEXt chunks

---

## 🐛 Common Modification Points

| Task | Files to Modify |
|------|-----------------|
| Add new 2D variation | [variations/mod.rs](src/variations/mod.rs), [shader_builder.rs](src/renderer/shader_builder.rs) (generates WGSL) |
| Add new 3D variation | [variations/mod.rs](src/variations/mod.rs), [shader_builder.rs](src/renderer/shader_builder.rs) (generates WGSL) |
| Add variation parameters | [variations/mod.rs](src/variations/mod.rs) - use `registry.add_parameters()` |
| Change color algorithm | [main_template.wgsl](shaders/core/main_template.wgsl), [tonemap.wgsl](shaders/tonemap.wgsl) |
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

## 🔄 Delta-Based State Management System (Completed 2025-11-17)

### Overview
The application uses a **simplified delta-based state management system** (ConfigManager) that has **completely replaced** the previous flag-based approach. All configuration changes flow through a single centralized gateway that tracks deltas, manages undo/redo with automatic coalescing, and determines selective GPU updates.

**Status**: ✅ **COMPLETE** - All UI controls simplified, preview mode removed, real-time rendering with 100ms overwrite window

**See [CONFIG.md](main/CONFIG.md)** for complete ConfigManager documentation.

### Architecture

**Core Components:**
- **ConfigManager** ([src/config/manager.rs](../src/config/manager.rs)) - Central state manager
  - Stores current (current state)
  - Maintains undo_history (50 ConfigChange entries)
  - Automatic coalescing (2 second window for same parameter)
  - Computes deltas on every change
  - Returns UpdateType for selective GPU updates

- **ConfigPath** ([src/config/delta.rs](../src/config/delta.rs)) - Type-safe parameter identifiers
  - Examples: `Zoom`, `Exposure`, `TransformAffine { index, param }`
  - 100+ paths covering all editable parameters
  - Display trait for human-readable undo descriptions

- **ConfigValue** ([src/config/delta.rs](../src/config/delta.rs)) - Type-safe value container
  - Wraps all value types (Float, Int, Bool, ColorRgb, enums, etc.)
  - Enables generic parameter updates

- **ConfigDelta** ([src/config/delta.rs](../src/config/delta.rs)) - Change record
  - Records (path, old_value, new_value)
  - Used for undo/redo operations

- **UpdateType** - Selective update classification
  - `None` - No GPU update needed
  - `ViewOnly` - Camera/zoom changed, use overwrite mode
  - `ColorOnly` - Palette changed, use overwrite mode
  - `IterationReset` - Transform/variation changed, use overwrite + reset iteration counter after 100ms
  - `ToneMappingOnly` - Tone mapping changed, no accumulation buffer changes

- **100ms Overwrite Window** ([src/app/mod.rs](../src/app/mod.rs)) - Real-time rendering without blank frames
  - Triggered by ViewOnly, ColorOnly, or IterationReset updates
  - Sets `blend_factor=1.0` (replace buffer instead of blend)
  - Sets `batch_size=1` (accumulate every frame, not every 4th)
  - Keeps overwrite ON for 100ms after last change (~6 frames at 60fps)
  - After window expires: return to normal accumulation and optionally reset iteration counter

### UI Integration

**Standard Pattern** (simple immediate updates with automatic coalescing):
```rust
// Read current value
let mut value = config_manager.current().exposure;

// Show UI control
let response = ui.add(egui::Slider::new(&mut value, 0.1..=5.0).text("Exposure"));

// Update if changed
if response.changed() {
    config_manager.update_param(ConfigPath::Exposure, value.into())?;
}
```

**That's it!** No lazy parameters, no force_commit calls, no preview mode logic.

**Batch Updates** (multiple related parameters):
```rust
let changes = vec![
    (ConfigPath::TransformAffine { index, param: A }, a.into()),
    (ConfigPath::TransformAffine { index, param: B }, b.into()),
    // ... more params
];
config_manager.update_batch(changes, "Transform affine update")?;
```

**How It Works**:
- Every `update_param()` call creates a ConfigChange
- Coalescing automatically merges changes to **same ConfigPath** within 2 seconds
- Changes to **different ConfigPath** always create separate undo entries
- Result: Slider drags create 1 undo entry (not 100+)

### Key Benefits

**Compared to old flag-based system:**
1. **Single Source of Truth** - All changes go through ConfigManager
2. **Automatic Undo/Redo** - No manual capture_state() calls needed
3. **Selective Updates** - UpdateType determines minimal GPU work
4. **Human-Readable History** - ConfigPath::Display shows "Transform 2 → Affine a"
5. **Type Safety** - Compile-time verification of parameter types
6. **Automatic Coalescing** - Merges rapid changes (2s window) prevents undo stack bloat
7. **Real-Time Rendering** - 100ms overwrite window eliminates blank frames

### Migration Status: ✅ COMPLETE

**All UI Controls Simplified (2025-11-17):**
- ✅ View controls (zoom, pan, rotation, camera) - Real-time updates
- ✅ Settings sliders (iterations, blend, compression, etc.) - Coalescing
- ✅ Tone mapping controls (exposure, gamma, curves) - No accumulation reset
- ✅ Variation weights and parameters - Real-time updates
- ✅ Color controls (palette, background, color mode) - Real-time updates
- ✅ **Triangle Editor** - Batch updates for multi-param changes
- ✅ Palette editor - Direct updates (no separate preview mode)

**Simplification (PR #23 - 2025-11-17):**
- Removed preview mode system and lazy parameter complexity
- All updates now immediate with automatic coalescing
- 100ms overwrite window provides smooth real-time updates
- No blank frames during any parameter changes
- UI code dramatically simplified (no lazy/force_commit logic)

**Non-Config Actions** (intentionally separate):
- Transform add/delete (structural changes)
- Config import/export (file I/O)
- Preset loading (bulk replacement)

### Project Documentation

**COMPLETED - All documentation archived to [docs/archive/delta-migration/](archive/delta-migration/)**

**Historical Reference (Archived):**
- [delta-based-state-management.md](archive/delta-migration/delta-based-state-management.md) - Original 2,600-line plan
- [delta-system-completed.md](archive/delta-migration/delta-system-completed.md) - Completed work summary (Phases 1-10)
- [complete-delta-migration.md](archive/delta-migration/complete-delta-migration.md) - Final migration phases (11-16)
- [MIGRATION-STATUS.md](archive/delta-migration/MIGRATION-STATUS.md) - Detailed migration tracking
- [remove-preview-mode.md](archive/remove-preview-mode.md) - Preview mode removal plan (PR #23)

**Current Documentation:**
- [CONFIG.md](main/CONFIG.md) - Complete ConfigManager reference
- [palette-system-redesign.md](projects/palette-system-redesign.md) - Palette architecture

### Performance Characteristics

**Memory:**
- Undo stack: 50 × sizeof(FractalConfig) ≈ 50 × 10KB = 500KB
- Delta computation: O(1) per change (direct field access)
- No heap allocations for simple value changes

**CPU:**
- ConfigPath matching: Single match statement (< 100ns)
- Value extraction: Direct field access (< 50ns)
- Undo/redo: Clone FractalConfig (< 10μs)

**Overhead vs Flag-Based:**
- Negligible - delta computation is trivial compared to GPU work
- Benefits far outweigh costs (cleaner code, better UX)

---

## 🌐 WebAssembly (WASM) Support

### Overview
The project has **full WASM support** including interactive rendering and headless PNG export in browsers.

**See [WASM.md](WASM.md)** for complete WASM build documentation.

### Architecture

**Entry Points:**
- **Interactive App** ([src/lib.rs](../src/lib.rs):run()) - Full GUI in browser via egui
- **Headless Export** ([src/app/export.rs](../src/app/export.rs):export_headless_wasm()) - PNG generation without window
- **WASM Bindings** ([src/wasm_api.rs](../src/wasm_api.rs)) - JavaScript API via wasm-bindgen

**JavaScript API:**
```javascript
// Exposed function: export_headless_wasm(config, width, height, ipt, speed)
const pngData = await export_headless_wasm(config, 800, 600, 256, 4);
```

**Browser Compatibility:**
- ✅ Chrome/Chromium 113+ (fully tested)
- ✅ Firefox 121+ (fully tested)
- ⚠️ Safari (experimental WebGPU support)
- ❌ Mobile (limited WebGPU support)

**Key Differences from Desktop:**

1. **GPU Limits** - Uses `downlevel_webgl2_defaults()` instead of desktop defaults
   - Reduced buffer sizes and texture dimensions
   - Ensures compatibility with browser WebGPU implementations

2. **Texture Format** - 1D textures → 2D with height=1
   - Browser WebGPU doesn't support `textureSampleLevel` on 1D textures
   - Palette and curve LUTs use 2D textures with `vec2(x, 0.5)` sampling

3. **Surface Creation** - Direct canvas approach on macOS
   - Uses `SurfaceTarget::Canvas(canvas)` instead of window-based surface
   - Fixes compatibility issues with macOS Safari/Chrome

4. **Timing** - Uses `web_time` crate instead of `std::time`
   - Provides accurate timestamps in WASM environment

### Visual Regression Testing

**Test Infrastructure:**
- **Desktop Tests** ([tests/visual/run_tests.py](../tests/visual/run_tests.py)) - CLI headless export
- **WASM Tests** ([tests/visual/wasm/test_wasm.py](../tests/visual/wasm/test_wasm.py)) - Browser automation via Playwright
- **Unified Runner** ([tests/visual/run_all_tests.py](../tests/visual/run_all_tests.py)) - Runs both + performance comparison

**Test Process:**
1. Playwright launches headless Chrome/Firefox
2. Loads WASM module and test configs
3. Calls `export_headless_wasm()` for each config
4. Downloads PNG via blob URL
5. Compares pixel data (SHA256 hash) against baseline
6. Extracts performance metrics from PNG metadata
7. Saves results to `performance_history.csv`

**Coverage:**
- 8 desktop visual tests (800x600, pixel-perfect comparison)
- 7 WASM visual tests (800x600, pixel-perfect comparison)
- Performance tracking: render time, throughput (M iter/sec)
- Baseline comparison: time delta, throughput delta

**Performance:**
- Desktop: ~500-3500ms total export time (device + render + encode)
- WASM: ~500-3500ms total export time (same as desktop)
- PNG metadata: Stores total export time (user-facing performance)

---

**Last Updated:** 2025-11-17
**Project:** fflame-rust

**Major Recent Changes:**
- **Simplified state management system** (2025-11-17, PR #23)
  - Removed preview mode and lazy parameter complexity
  - 100ms overwrite window for real-time rendering without blank frames
  - Automatic coalescing (2 second window) for undo history
  - All UI controls simplified to immediate updates
  - ConfigManager with automatic undo/redo and delta tracking (2025-10-31, PR #22)
  - Selective GPU updates via UpdateType classification
  - 100+ type-safe ConfigPath variants for all parameters
  - Visual undo history window with human-readable descriptions
  - Replaces flag-based system (flame_changed, etc.)
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
