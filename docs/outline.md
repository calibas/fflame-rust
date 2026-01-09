# Rust + wgpu Fractal Flame Renderer — Project Architecture

**Purpose:**
A project-scoped architecture document describing the design, file layout, data formats, pipeline, and development milestones for a GPU-accelerated fractal-flame renderer implemented in **Rust** using **wgpu** (WebGPU). The target is an interactive desktop application with optional WebAssembly (browser) demo.

**Status:** ~96% complete, fully functional. See [STATUS.md](STATUS.md) for detailed comparison.

**Key Implementation Decisions:**
- ✅ Using **ping-pong accumulation** (not atomics) for better performance and compatibility
- ✅ Using **JSON** (not RON) for config serialization
- ✅ Added **undo/redo system** with 50-state history
- ✅ Added **three color modes**: Transform, Palette, Speed-based
- ✅ Added **24 variations** (16 2D + 8 3D) with full 3D rendering support
- ✅ Added **comprehensive testing** infrastructure (unit, regression, benchmarks)
- ✅ Added **version tracking** with auto-incrementing build numbers
- ✅ Added **GPU and CPU profiling** tools
- ✅ **Asset auto-loading** from filesystem (desktop only)

**Documentation:**
- [STATUS.md](STATUS.md) - Implementation status vs this outline
- [ARCHITECTURE.md](ARCHITECTURE.md) - Code organization and data flow
- [../CLAUDE.md](../CLAUDE.md) - Project context for Claude Code
- [TESTING-GUIDE.md](TESTING-GUIDE.md) - Complete testing reference
- [PROFILING.md](PROFILING.md) - Performance profiling guide
- [VERSION-TRACKING.md](VERSION-TRACKING.md) - Build and version system

---

## 1 — High-level goals

- ✅ Real-time, interactive fractal flame exploration at 720p–1440p with progressive refinement.
- ✅ Deterministic seeds and reproducible scenes (via config import/export).
- ✅ Cross-platform: Windows, Linux, macOS (including Apple Silicon), Web via WASM.
- ✅ Modular code structure with separable GPU, scene, renderer, and UI modules.
- ✅ Compatible with all GPU hardware (no special features required beyond `CLEAR_TEXTURE`).
- ➕ Added: Undo/redo for interactive exploration, multiple color modes, customizable palettes.

---

## 2 — Tech stack

- Language: **Rust** (stable)
- GPU API: **wgpu** (latest stable compatible with target platforms)
- Windowing / Input: **winit**
- Immediate-mode UI: **egui** (via egui-winit + egui-wgpu integration)
- Serialization & config: **serde** + **JSON** (serde_json)
- State management: **undo/redo** history with FractalConfig snapshots
- CLI / app boot: **clap** (for command-line render/export options)
- Optional: **wasm-bindgen** and **wasm-pack** for browser build
- Testing: **Criterion** for benchmarks, built-in test framework for unit/regression tests
- Profiling: **GpuProfiler** with timestamp queries, **web-time** for CPU timing
- Version tracking: **build.rs** with auto-incrementing build numbers, **chrono** for timestamps
- Tooling: **cargo**, **rustfmt**, **clippy**, CI (GitHub Actions)

---

## 3 — Top-level repo layout

```
fractal-flame-wgpu/
├── Cargo.toml
├── build.rs             # build script (version capture, build number increment)
├── build_number.txt     # auto-incrementing build counter (currently at #9)
├── README.md
├── CLAUDE.md            # project context for Claude Code
├── STATUS.md            # implementation status vs this outline
├── ARCHITECTURE.md      # code organization and data flow guide
├── TESTING-GUIDE.md     # complete testing reference
├── PROFILING.md         # performance profiling guide
├── VERSION-TRACKING.md  # build and version system documentation
├── WASM-STATUS.md       # WebAssembly build status
├── WASM-PROFILING.md    # WASM-specific profiling
├── MILESTONE-7-COMPLETE.md # milestone 7 summary
├── src/
│   ├── main.rs          # entrypoint (app init, event loop)
│   ├── lib.rs           # library root + WASM entry
│   ├── app.rs           # App struct, state machine
│   ├── ui/
│   │   ├── mod.rs
│   │   └── panels.rs    # UI controls, presets, palette editor
│   ├── gpu/
│   │   ├── mod.rs
│   │   ├── device.rs    # wgpu device and queue setup, feature checks
│   │   ├── pipelines.rs # pipeline creation: compute + render pipelines
│   │   └── buffers.rs   # buffer / texture layout & helpers
│   ├── scene/
│   │   ├── mod.rs
│   │   ├── transforms.rs# transform definitions, 24 variations (16 2D + 8 3D)
│   │   ├── presets.rs   # built-in flame presets
│   │   ├── assets.rs    # asset loading from filesystem
│   │   └── palette.rs   # color modes, palettes, gradient system
│   ├── renderer/
│   │   ├── mod.rs
│   │   └── compute_kernel.rs # GPU orchestration, PNG export
│   ├── config.rs        # FractalConfig serialization (.fflame files)
│   ├── undo.rs          # undo/redo history system
│   ├── util.rs          # performance metrics
│   ├── profiler.rs      # GPU/CPU profiling tools
│   ├── version.rs       # version tracking and build info
│   └── bin/
│       └── simple_benchmark.rs # CLI benchmark tool
├── tests/
│   └── regression.rs    # 12 regression tests (all passing)
├── benches/
│   └── flame_bench.rs   # Criterion benchmarks
├── examples/
│   ├── show_version.rs  # display version info
│   └── export_presets.rs # export built-in presets to files
├── assets/
│   ├── palettes/        # .palette files (auto-loaded on desktop)
│   │   ├── fire.palette
│   │   ├── cool.palette
│   │   └── rainbow.palette
│   └── presets/         # .fflame config files (auto-loaded on desktop)
│       ├── simple.fflame
│       ├── complex.fflame
│       ├── spherical.fflame
│       ├── spiral.fflame
│       └── julia.fflame
└── shaders/             # WGSL shader files (compute + fragment)
    ├── trajectory.wgsl  # flame iteration compute shader (2D mode)
    ├── trajectory_3d.wgsl # flame iteration compute shader (3D mode)
    ├── accumulate.wgsl  # temporal ping-pong accumulation
    └── tonemap.wgsl     # tone mapping + palette lookup
```

Notes:
- Keep `gpu/` focused on wgpu resource management; `renderer/` orchestrates compute + render flow.
- `scene/transforms.rs` implements 24 variation functions (16 2D + 8 3D) in pure Rust for CPU-side testing.
- Palettes and presets auto-load from `assets/` on desktop (WASM uses built-in only).
- Config system provides JSON import/export for sharing fractal configurations.
- Testing infrastructure includes unit tests, regression tests, and Criterion benchmarks.
- Version tracking with auto-incrementing build numbers via build.rs script.
- Full 3D rendering with dual shader system (trajectory.wgsl for 2D, trajectory_3d.wgsl for 3D).

---

## 4 — Core data structures and formats

### 4.1 Transform (per-IFS transform)

Rust struct (conceptual):

```text
Transform {
    // 2x2 linear matrix
    a: f32, b: f32,
    c: f32, d: f32,
    // translation
    e: f32, f: f32,
    // weight (probability)
    weight: f32,
    // variation weights (vector of 24 variations: 16 2D + 8 3D)
    variations: [f32; 24],
    // Z offset for 3D mode
    g: f32,
    // color contribution (RGB)
    color: [f32; 3],
    // color blending speed (0.0 = parent color, 1.0 = transform color)
    color_speed: f32,
}
```

- Serialized to JSON for config import/export (.fflame files).
- Packed into a `std430`-style structure for GPU buffers (align to vec4 boundaries).

### 4.2 Palette / LUT

- 1D texture (256×1 Rgba8Unorm) uploaded to GPU for palette lookup.
- CPU-side representation with ColorStop gradient system for editing.
- Built-in palettes: Grayscale, Fire, Cool, Rainbow, Purple Pink.
- Custom palette editor in UI with add/remove color stops.

### 4.2.1 Color Modes

Three color assignment strategies:
- **Transform**: Use per-transform colors, blended with color_speed
- **Palette**: Look up 1D palette texture based on accumulated color index
- **Speed**: Color based on iteration velocity, mapped through palette

### 4.3 Accumulation buffer

- **Ping-pong double buffering**: Two RGBA32Float textures swapped each frame
- **Temp samples texture**: RGBA32Float for current frame's samples
- Layout: same size as output resolution; each pixel stores R,G,B color and sample count in alpha.
- Accumulation strategy: Exponential moving average with blend_factor = 1.0 / samples_accumulated
- No atomic operations needed - each pixel written by single thread in accumulate pass.

### 4.4 Work dispatch parameters

Packed into uniform buffers:
- **GpuParams**: num_transforms, iterations_per_thread, burn_in, width, height, seed, color_mode, splat_size, zoom, pan_x, pan_y, rotation, speed_factor, camera_pitch, camera_yaw, projection_type, perspective_strength
- **TonemapParams**: exposure, gamma, density_scale, background_color
- **AccumulateParams**: width, height, blend_factor

---

## 5 — GPU pipeline & shaders

### 5.1 Pipelines (wgpu)

Three-pass progressive rendering:

- **Compute pipeline: `trajectory` (2D mode)** (trajectory.wgsl)
  - Input: transforms storage buffer, palette texture, GpuParams UBO
  - Output: temp samples texture (additive write, no atomics)
  - Work: 128 workgroups × 64 threads, each runs 256 iterations (configurable)
  - Per-thread: PCG RNG, weighted transform selection, affine + 16 2D variations, color accumulation
  - Supports 3 color modes and view transforms (zoom, pan, rotation)

- **Compute pipeline: `trajectory` (3D mode)** (trajectory_3d.wgsl)
  - Same as 2D plus:
  - Affine transformation with Z offset (g field)
  - 24 variation functions (16 2D + 8 3D)
  - Camera rotation (pitch/yaw)
  - Projection (orthographic or perspective)
  - Z tracking through iteration

- **Compute pipeline: `accumulate`** (accumulate.wgsl)
  - Input: temp samples texture, previous accumulation texture, AccumulateParams
  - Output: current accumulation texture
  - Work: One thread per 8×8 pixel tile
  - Blends new samples with history using exponential moving average
  - Swaps ping-pong textures after each frame

- **Render pipeline: `tonemap`** (tonemap.wgsl)
  - Input: current accumulation texture, palette texture (for Speed mode), TonemapParams
  - Output: swapchain (screen)
  - Fullscreen triangle: log-scale tone mapping, palette lookup, gamma correction, background blending

### 5.2 Shader design notes

- ✅ WGSL for all shaders (wgpu portable)
- ✅ Single-precision float math throughout
- ✅ PCG random number generator per thread (seeded with global_invocation_id + frame seed)
- ✅ 24 variation functions implemented:
  - **2D (0-15)**: Linear, Sinusoidal, Spherical, Swirl, Horseshoe, Polar, Handkerchief, Heart, Disc, Spiral, Hyperbolic, Diamond, Ex, Julia, Bent, Waves
  - **3D (16-23)**: Zcone, Flatten, Hemisphere, PreRotateX, PreRotateY, PostRotateX, PostRotateY, ZScale
- ✅ CPU reference implementations in Rust for validation (2D only)
- ✅ No atomics used - ping-pong accumulation is faster and more compatible
- ✅ Dual shader system for 2D and 3D rendering modes

---

## 6 — CPU-side render orchestration

`renderer::compute_kernel.rs` (FlameRenderer) responsibilities:

- ✅ Progressive rendering: Each frame runs compute → accumulate → tonemap passes
- ✅ Ping-pong texture management: Swaps current/previous accumulation textures
- ✅ Sample tracking: Counts frames and total iterations (workgroups × threads × iterations_per_thread)
- ✅ Parameter updates: Flame, iterations, view transform, color mode, palette, density, background
- ✅ Accumulation reset: Triggered on any parameter change
- ✅ Optional pause/resume and max iterations limit
- ✅ PNG export: Captures current accumulation buffer via GPU readback

---

## 7 — UI and UX

Implemented in `ui/mod.rs` using egui (floating windows):

**Performance Window:**
- ✅ FPS, frame time, resolution display
- ✅ Frames accumulated, total iterations with K/M/B/T formatting
- ✅ Pause/Resume button
- ✅ Reset accumulation button
- ✅ Max iterations slider (logarithmic 1K - 1T)
- ✅ Iterations per thread slider (64-4096)
- ✅ Density scale slider
- ✅ Background color picker
- ✅ Color mode selector (Transform/Palette/Speed)
- ✅ Palette dropdown (when in Palette or Speed mode)
- ✅ Speed blend factor slider (Speed mode)
- ✅ Palette editor button
- ✅ View controls: zoom, pan X/Y, rotation sliders
- ✅ Arrow button grid for pan navigation
- ✅ Reset view button
- ✅ Undo/Redo buttons (Ctrl+Z, Ctrl+Y)
- ✅ Config import/export dialog button
- ✅ PNG export buttons (with/without background)

**Transforms Window:**
- ✅ Scrollable list of all transforms
- ✅ Collapsible headers per transform
- ✅ Affine matrix editors (a, b, c, d, e, f)
- ✅ Weight slider (0-2)
- ✅ RGB color sliders
- ✅ Color speed slider (0-1)
- ✅ Z offset slider (3D mode only)
- ✅ 24 variation weight sliders with names (split into 2D and 3D sections)
- ✅ Add/delete transforms buttons
- ❌ Clone/duplicate transform
- ❌ Final transform controls

**Palette Editor Window:**
- ✅ Gradient preview bar (samples palette at every pixel)
- ✅ Color stop list with position sliders (0-255)
- ✅ Color pickers per stop
- ✅ Add color stop button
- ✅ Remove stop button (minimum 2 stops)
- ✅ Apply button (creates custom palette)
- ❌ Save palette to library permanently

**Config Import/Export Window:**
- ✅ Export to clipboard (JSON)
- ✅ Save as .fflame file (desktop file dialog)
- ✅ Import from JSON text area
- ✅ Load .fflame file (desktop file dialog)
- ✅ WASM: file dialogs copy to/from clipboard

**Viewport Interaction:**
- ✅ Left mouse drag → pan
- ✅ Mouse wheel → zoom toward cursor
- ✅ Arrow keys → pan
- ✅ +/- or numpad +/- → zoom in/out
- ❌ Right click context menu

**Missing from outline:**
- ❌ Preset browser UI
- ❌ Randomize button with seeded generation
- ❌ High-res export UI with tiling options

---

## 8 — Export & high-resolution rendering

**Current Implementation:**
- ✅ PNG export at current viewport resolution (implemented in `compute_kernel.rs`)
- ✅ GPU readback via buffer mapping with async await
- ✅ Transparent background option with proper alpha preservation
  - **Transparent export**: Reads from Rgba16Float accumulation buffer, applies CPU tone mapping
  - **Opaque export**: Renders with tonemap shader, reads from Rgba8 render target
  - Necessary because tonemap shader blends RGB with background before outputting
- ✅ Automatic BGRA ↔ RGBA conversion for format compatibility
- ✅ Vertical flip (GPU textures are upside down)
- ✅ Desktop: blocking export with file dialog
- ✅ WASM: async export with `spawn_local` lifetime extension

**Not Implemented (from outline):**
- ❌ Tiled high-resolution rendering (would allow 4K+ exports)
- ❌ Separate sample budget for export quality
- ❌ EXR/HDR format support
- ❌ Post-processing (sharpening, additional AA)
- ❌ Async export progress UI

---

## 9 — Fallbacks & compatibility

**Current Implementation:**
- ✅ Minimal feature requirements: only requires `CLEAR_TEXTURE` feature
- ✅ No float atomics needed (ping-pong accumulation works everywhere)
- ✅ Standard texture formats (Rgba8Unorm, RGBA32Float)
- ✅ Fixed workgroup size (8×8 = 64 threads)
- ✅ Works on all wgpu backends (Vulkan, Metal, DX12, WebGL2, WebGPU)

**Not Implemented (from outline):**
- ❌ GPU feature detection and fallback strategies
- ❌ Workgroup shared memory histograms
- ❌ Integer fixed-point accumulation
- ❌ Dynamic shader variant selection

**Platform Status:**
- ✅ Windows (DX12/Vulkan) - fully working
- ✅ macOS (Metal) - fully working
- ✅ Linux (Vulkan) - fully working
- ✅ WASM (WebGPU/WebGL2) - 100% working (including PNG export)

---

## 10 — Testing & validation

**Current Implementation:**
- ✅ **Unit tests** in `scene/transforms.rs`:
  - Point calculations (r, r², θ, φ)
  - 24 variation functions (16 2D + 8 3D)
  - Affine transformations
  - Flame iteration logic
- ✅ **Unit tests** in `scene/palette.rs`:
  - Color interpolation
  - Texture data generation
- ✅ **Regression tests** in `tests/regression.rs` (12 tests, all passing):
  - CPU iteration determinism
  - All 24 variation functions
  - Preset validation
  - Serialization round-trips
- ✅ **Criterion benchmarks** in `benches/flame_bench.rs`:
  - CPU iteration performance
  - Individual variation functions
  - Affine transformations
  - Point calculations
- ✅ **CLI benchmark tool** in `src/bin/simple_benchmark.rs`:
  - Tests all presets
  - Tests all variations
  - Human-readable M ops/sec output

**Not Implemented:**
- ❌ Visual regression tests with image checksums
- ❌ GPU vs CPU reference comparison tests
- ⚠️ GPU profiling (infrastructure exists but not used in CI)

---

## 11 — Profiling & optimization

**Current Performance:**
- Target: 60+ FPS at 1080p ✅
- Default config: 128 workgroups × 64 threads × 256 iterations = ~2.1M iterations/frame
- Achieves 200-400 FPS on modern GPUs (RTX 3060+, M1+)
- Progressive refinement: visible structure in 1-2 frames, high quality in 100+ frames

**Optimization Strategies (in priority order):**
1. ✅ **Ping-pong accumulation** - Eliminated atomic contention completely
2. ✅ **Configurable iterations-per-thread** - UI slider 64-4096 (reduces dispatch overhead)
3. ❌ Fixed workgroup size (8×8) - could tune for GPU occupancy
4. ❌ Multi-resolution progressive rendering
5. ❌ Adaptive sampling based on density

**Profiling Tools Implemented:**
- ✅ **GpuProfiler** (src/profiler.rs) - GPU timestamp queries for measuring pass durations
- ✅ **CPU Scopes** - RAII-based timing for CPU code sections
- ✅ **PerformanceMetrics** - FPS, frame time, component timing, JSON export
- ✅ **Version Tracking** - Build numbers, git hash, platform info
- ✅ **Criterion Benchmarks** - Statistical microbenchmarking for CPU code
- ✅ **CLI Benchmark** - Simple_benchmark.rs for human-readable performance testing

**External Profiling Tools:**
- RenderDoc for frame capture
- NVIDIA Nsight for GPU profiling
- Xcode GPU frame capture (macOS)
- Chrome DevTools (WASM)

---

## 12 — Milestones (project-level)

1. ✅ **Repo skeleton:** wgpu + winit + egui setup, window + swapchain, basic UI panel
2. ✅ **CPU reference:** Full CPU flame implementation with 24 variations (`scene/transforms.rs`)
3. ✅ **Minimal GPU point plot:** Compute shader generates samples, writes to temp texture
4. ✅ **Accumulation pass:** Ping-pong accumulation with progressive refinement
5. ✅ **UI integration:** Full transforms UI, palette editor, color modes, view controls
6. ✅ **Export:** PNG export at viewport resolution (desktop + WASM, tiled high-res pending)
7. ✅ **Performance tuning:** Comprehensive profiling tools, version tracking, benchmarks
8. ✅ **Cross-platform:** Works on Windows, macOS, Linux, WASM (100%)

**Added Beyond Original Milestones:**
- ✅ Undo/redo system with 50-state history
- ✅ Config import/export (.fflame JSON files)
- ✅ Three color modes (Transform, Palette, Speed)
- ✅ 3D rendering with 8 3D variations (total 24 variations)
- ✅ Camera rotation and projection controls (orthographic/perspective)
- ✅ Asset auto-loading from filesystem (desktop)
- ✅ Background color customization
- ✅ Pause/resume and max iterations limit
- ✅ Interactive viewport (mouse/keyboard navigation)
- ✅ Performance metrics with JSON export
- ✅ Version tracking with auto-incrementing build numbers
- ✅ GPU profiler with timestamp queries
- ✅ Comprehensive testing (unit, regression, benchmarks)
- ✅ Transform add/delete UI controls

---

## 13 — Security & privacy

- Sandbox WASM builds carefully: avoid allowing arbitrary file write in browser context.
- Be cautious when loading presets from untrusted sources — parse with safe deserializers and validate numeric ranges.

---

## 14 — Future expansions

- Add CUDA backend for NVIDIA for much faster accumulation if targeting desktops only.
- Integrate compute-graph scheduler for dynamic load balancing on hybrid CPU/GPU.
- Add animation tools: morphing between transforms and keyframe timelines.
- Implement layered compositing and vector field-guided splatting for artist workflows.

---

## 15 — Appendix: quick buffer & bind layout (conceptual)

**Bind group 0 — Scene / transforms**
- `0` : transforms storage buffer (array of packed Transform)
- `1` : palette texture (1D) + sampler
- `2` : params uniform buffer (seed, resolution, view transform, iteration counts)

**Bind group 1 — Accumulation target**
- `0` : accumulation storage texture (RGBA32Float) — image/texture2D write access

**Bind group N — reduce / staging**
- per-workgroup staging buffer (only bound during reduce pass)


---