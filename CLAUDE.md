# Fractal Flame Renderer - Project Context

## Overview

**Quick Start:** This file provides a concise quick reference for AI assistants. For detailed documentation, see the docs below.

**Core Documentation:**
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) - System overview and navigation hub
- [docs/main/](docs/main/) - Detailed topic documentation:
  - [UI.md](docs/main/UI.md) - Windows, panels, input handling
  - [BUFFERS.md](docs/main/BUFFERS.md) - GPU layouts, data structures
  - [TRANSFORMS.md](docs/main/TRANSFORMS.md) - Flame algorithm, IFS, thread isolation
  - [RENDERER.md](docs/main/RENDERER.md) - 3-pass pipeline, FlameRenderer
  - [SHADERS.md](docs/main/SHADERS.md) - WGSL modular system
  - [VARIATIONS.md](docs/main/VARIATIONS.md) - All 26 variations, registry
  - [COLOR.md](docs/main/COLOR.md) - Color modes, palette, histogram
  - [CONFIG.md](docs/main/CONFIG.md) - FractalConfig, presets, undo/redo
  - [EXPORT.md](docs/main/EXPORT.md) - PNG export, metadata, CLI batch mode
- [docs/TESTING-GUIDE.md](docs/TESTING-GUIDE.md) - Unit tests, regression, benchmarks
- [docs/WASM.md](docs/WASM.md) - WebAssembly build guide
- [docs/STATUS.md](docs/STATUS.md) - Implementation status vs original design
- [docs/outline.md](docs/outline.md) - Original design goals

**Note:** Project history is tracked via git commits. Use `git log --oneline` to see recent changes.

## Quick Reference

### Project Structure
- **Shaders**: Dynamic shader compilation from modular components
  - `shaders/core/` - Modular shader components (dynamically assembled)
    - `header.wgsl` - Structs and bind groups
    - `rng.wgsl` - Random number generation
    - `variations_2d.wgsl` - 2D variation functions
    - `variations_3d.wgsl` - 3D variation functions (includes all 2D + 3D-specific)
    - `utilities.wgsl` - Helper functions (r, θ, φ calculations)
    - `main_2d.wgsl` - 2D compute shader entry point
    - `main_3d.wgsl` - 3D compute shader entry point
  - `shaders/accumulate.wgsl` - Temporal blending pass
  - `shaders/tonemap.wgsl` - Display tone mapping pass
  - **Note**: Shaders are built dynamically by `ShaderBuilder` with only active variations

- **Core Modules**:
  - `src/app/` - Application state and event handling (modular)
    - `mod.rs` - Core App struct, event loop, render function
    - `input.rs` - Keyboard, mouse, wheel input handlers
    - `config.rs` - Config export/import (legacy, being phased out)
    - `export.rs` - Headless PNG export for CLI
  - `src/config/` - **Delta-based state management system** (Added 2025-10-31)
    - `manager.rs` - ConfigManager with undo/redo + system settings integration
    - `delta.rs` - ConfigPath, ConfigValue, ConfigDelta enums (568 lines)
    - `slider.rs` - UI helpers: config_slider (299 lines)
    - `fractal_config.rs` - FractalConfig struct (per-fractal state)
    - `defaults.rs` - Default value constants (single source of truth)
  - `src/storage/` - **Local storage system** (Added 2025-11-23, PR #27)
    - `settings.rs` - SystemSettings struct (device-specific settings)
    - `backend.rs` - Cross-platform storage (filesystem + localStorage)
    - `thumbnail_cache.rs` - **Thumbnail cache for preset gallery** (Added 2025-11-24)
    - Persists VSync, target FPS, iterations per thread, export defaults
    - Desktop: User data directory, WASM: browser localStorage
  - `src/renderer/compute_kernel.rs` - GPU rendering orchestration
  - `src/renderer/render.rs` - **Unified render API** (Added 2025-12-24) - Single entry point for all headless rendering
  - `src/renderer/thumbnail.rs` - **Thumbnail rendering** (Added 2025-11-24)
  - `src/export/` - **High-resolution export system** (Added 2025-12-18)
    - `high_res.rs` - CPU histogram + GPU tonemap for any resolution
    - `mod.rs` - `needs_cpu_export()` threshold check
  - `src/scene/transforms.rs` - Flame algorithm (CPU + GPU)
  - `src/ui/` - **Dockable panel UI system** (Migrated to egui_dock 2025-11-13)
    - `mod.rs` - Main UI coordinator, docking integration
    - `workspace.rs` - Docking layout management and panel organization
    - `settings.rs` - Settings panel (File, Rendering, Preferences)
    - `transforms.rs` - Transform list and editing panel
    - `triangle_editor.rs` - Visual triangle editor panel
    - `view.rs` - Camera and navigation controls panel
    - `tone_mapping.rs` - Color and tone mapping panel
    - `palette_editor.rs` - Palette editing panel
    - `palette_library.rs` - **Palette Library panel** (Added 2025-11-18)
    - `preset_library.rs` - **Preset Library panel** (Added 2025-11-24)
    - `fractal_gallery.rs` - **Reusable gallery widget** (Added 2025-11-24)
    - `undo_history.rs` - Visual undo history browser panel
    - `menu_bar.rs` - Top menu bar (File, Edit, View, etc.)
  - `src/i18n.rs` - **Internationalization support** (Added 2025-11-13)
    - Uses rust-i18n for multi-language support
    - Translation files in `locales/*.yml`
    - Language switcher in Settings → Preferences

## Environment
- Shell within VSCode: Git Bash (MSYS2 MinGW64)
- Use forward slashes for paths
- Avoid `/dev/null` (creates literal files on Windows)
- NO `sed` EVER! It's never worked out well.

### Key Concepts
- **Fractal Flames**: IFS (Iterated Function System) with variations
- **Render Modes**: 2D (classic) and 3D (pseudo-3D with depth)
- **26 Core Variations** (registry can hold hundreds/thousands total, max 50 active per flame):
  - **Basic 2D (0-4)**: Linear, Sinusoidal, Spherical, Swirl, Horseshoe
  - **Advanced 2D (5-15)**: Polar, Handkerchief, Heart, Disc, Spiral, Hyperbolic, Diamond, Ex, Julia (RNG), Bent, Waves
  - **3D Depth (16-17, 23)**: Zcone, Flatten, ZScale
  - **3D Full (18)**: Hemisphere
  - **3D Rotation (19-22)**: PreRotateX, PreRotateY, PostRotateX, PostRotateY
  - **Parameterized (24-25)**: JuliaN (power, dist), Blob (high, low, waves)
  - **Plugin Variations**: Registry can hold unlimited variations; dynamically assigned to shader indices 26-49 based on active usage
- **Variation Index Mapping**: Two-tier system for core stability + plugin flexibility
  - **Core variations (0-25)**: Fixed indices, never change (backward compatibility)
  - **Plugin variations (26-49)**: Dynamic indices assigned per-flame based on which plugins are active
  - Shader is dynamically compiled with only the active variations (up to 50 total)
  - Registration order for core variations is fixed in code to maintain consistent IDs
  - Global singleton registry ensures all code paths use same mapping
  - UI ordering: by category first, then by registration order within category
- **Variation Parameters**: Some variations have configurable parameters (power, distance, waves, etc.)
  - Stored per-transform in HashMap
  - Uploaded to GPU via dedicated storage buffer (400 floats: 50 variations × 8 params)
  - Accessible in shaders via `get_param(xform_id, variation_id, param_slot)`
  - UI sliders appear below active variations (Float, Integer, Angle types)
- **3-Pass GPU Rendering Pipeline** (each frame at ~60 FPS):
  1. **Compute Pass** - Generate samples (128 workgroups × 256 iterations = 32,768 iterations/frame)
     - Each iteration: random transform → affine → variations → color → write to temp texture
     - Alpha channel = density (0.01 per hit)
  2. **Accumulate Pass** - Progressive refinement (blend new samples with history)
     - Ping-pong buffers swapped each frame
     - Blend control: Exponential (dynamic) or fixed rate
     - Low-density smoothing to reduce noise in sparse areas
     - Density compression to slow accumulation in bright areas
     - Per-pixel iteration limiting to prevent over-sampling dense areas (optional)
  3. **Tonemap Pass** - Display rendering
     - Log/linear tone mapping based on density
     - Optional S-curve adjustment
     - Exposure and gamma correction
     - Background color blending
- **Total Speed**: ~2 million iterations/second at default settings (128 × 256 × 60 FPS)
- **Speed Multiplier**: Quality control independent of render speed
  - **Problem**: High `iterations_per_thread` causes quality degradation (60-70% difference)
  - **Root Cause**: Fewer accumulation passes → chunky density growth → sqrt() artifacts
  - **Solution**: Speed multiplier normalizes accumulation frequency
  - **Interactive App**: Frame rate control (60 × multiplier FPS, up to 16× = 960 FPS)
  - **CLI Export**: Iteration chunking (`--speed-multiplier` parameter)
  - **Result**: Pixel-perfect quality at any `iterations_per_thread` setting
  - **Critical for animation**: Ensures consistent quality across frames with varying iteration counts
  - See [docs/ITERATIONS_PER_THREAD_QUALITY.md](docs/ITERATIONS_PER_THREAD_QUALITY.md) for complete analysis
- **Accumulation Controls**: Fine-grained control over convergence behavior
  - **Blend Rate**: 0.01 (slow/smooth) to 1.0 (fast/flickery), default 0.1 (10%)
  - **Dynamic Blend Mode**: Exponential convergence (old default) vs fixed rate (new option)
  - **Low-Density Smoothing** (0.0-1.0): Reduces blend rate in sparse areas to reduce noise, default 0.5
  - **Density Compression** (0.0-100.0): Slows accumulation in bright areas to reveal detail, default 0.0 (disabled)
    - Formula: `compression_factor = 1 / (1 + density × strength × 0.01)`
    - 25 = gentle (20% rate in bright areas), 50 = moderate (2% rate), 100 = strong (1% rate)
  - **Per-Pixel Iteration Limit** (0-1M): Stop accumulating pixel after N hits, default 0 (disabled)
    - Prevents over-sampling dense areas while sparse areas catch up
    - Tracked via atomic counters in compute shader (~5% performance overhead)
    - Gated after pixel accumulates initial density to avoid empty spots
    - Low limits (5-100) for quick previews, high limits (100K-1M) for quality
- **Color Modes**: Transform colors, Palette lookup, Speed-based coloring
- **Projection Types**: Orthographic (flat) and Perspective (depth-aware)
- **Camera Control**: Full 3D camera rotation (pitch and yaw) for viewing from any angle

### UI Architecture (egui_dock - Migrated 2025-11-13)
- **Docking System**: Flexible panel-based UI using egui_dock
  - All windows converted to dockable panels (1:1 mapping)
  - Panels can be rearranged, detached, and docked anywhere
  - Future: Save/restore workspace layouts
- **7 Main Panels**:
  1. **Fractal Viewport** - Main rendering display (always visible)
  2. **Settings** - File operations, rendering controls, preferences
  3. **Transforms** - Transform list, add/delete, affine parameters
  4. **Triangle Editor** - Visual affine editing with interactive triangles
  5. **View** - Camera controls, zoom, pan, rotation
  6. **Tone Mapping & Colors** - Color mode, palette, tone mapping settings
  7. **History** - Visual undo/redo browser with state preview
- **Menu Bar**: Top-level menus (File, Edit, View, Fractal, Rendering, Window, Help)
  - Professional menu structure for discoverability
  - Keyboard shortcuts documented in menus
  - Future: Implement all menu actions
- **Benefits**:
  - More flexible than fixed side panel layout
  - Users can organize UI to match workflow
  - Foundation for future workspace presets (Beginner/Standard/Advanced layouts)

### Palette Library System (Added 2025-11-18)
- **713 Total Palettes**: 12 curated + 701 Apophysis classics
- **Pack-Based Organization**:
  - **Starter Pack** (12 palettes) - Enabled by default
  - **Apophysis Pack** (701 palettes) - Disabled by default (enable via UI)
  - JSON format: `assets/palettes/packs/*.json`
- **Palette Library Panel**:
  - Visual browsing with gradient previews (200px × 20px)
  - Grid layout: Name on left, preview on right
  - Expand/collapse packs independently of enable/disable
  - Click palette to select (creates editable custom copy)
  - Hover feedback: Row highlight + pointer cursor
- **Loading System**:
  - Desktop: Loads from filesystem at runtime
  - WASM: Embeds Starter Pack at compile time (~2KB)
  - All routes use `add_or_update()` with case-insensitive duplicate checking
  - First palette loaded with a name wins (duplicates logged/skipped)
- **Custom Copy Behavior**:
  - Selecting from library creates copy: `"Name (Custom)"` or `"Name (Custom N)"`
  - Copy is editable (`built_in = false`), original unchanged
  - Same behavior as Colors panel dropdown
- **See**: [docs/main/COLOR.md](docs/main/COLOR.md) for palette system details

### Internationalization (i18n - Added 2025-11-13)
- **Framework**: rust-i18n v3.1 with YAML translation files
- **Architecture**:
  - Translation files in `locales/*.yml` (compile-time embedding)
  - `src/i18n.rs` module for locale management
  - Language switcher in Settings → Preferences panel
- **Current Support**:
  - English (en) - Complete with 200+ strings
  - Ready for community translations (Spanish, French, German, Japanese, Chinese)
- **Coverage**:
  - All menu items and panel titles
  - Transform and variation controls
  - Color and rendering settings
  - Tooltips and help text
  - Error messages and notifications
- **Font Support** (egui default):
  - ✅ Full: Latin scripts, Cyrillic, Greek
  - ⚠️ Limited: CJK (Chinese, Japanese, Korean) - basic characters only
  - ❌ No support: Arabic/Hebrew (RTL languages)
  - For full CJK: Add Noto Sans CJK font via egui FontDefinitions
- **See**: [docs/main/I18N.md](docs/main/I18N.md) for translation guide

### System Settings & Local Storage (Added 2025-11-23, PR #27)
- **Architecture**: Unified state management through ConfigManager
  - **FractalConfig**: Per-fractal artistic parameters (undo/redo enabled)
  - **SystemSettings**: Device-specific settings (no undo, persistent across sessions)
  - Both managed by ConfigManager for consistent GPU update propagation
- **System Settings** (device-specific, persisted to disk):
  - **Performance**: VSync, target FPS, iterations per thread
  - **UI/UX**: Language preference (saved for next session)
  - **Export Defaults**: Width, height, use custom size flag
  - **File Paths** (desktop only): Recent files list
- **VSync Configuration**:
  - **Desktop**: Toggle VSync on/off, set custom target FPS (10-1000 Hz)
  - **WASM**: VSync always enabled (WebGPU Fifo mode required), controls hidden
  - Settings persist across app restarts via local storage
- **Storage Backend**:
  - **Desktop**: JSON files in platform-specific user data directory
    - Windows: `%APPDATA%\FractalFlame\system_settings.json`
    - macOS: `~/Library/Application Support/FractalFlame/system_settings.json`
    - Linux: `~/.config/FractalFlame/system_settings.json`
  - **WASM**: Browser localStorage (5-10 MB quota)
  - Cross-platform API via `src/storage/backend.rs`
- **ConfigManager Integration**:
  - All system settings changes flow through `config_manager.update_system_setting()`
  - Returns `UpdateType` for GPU synchronization (e.g., iterations per thread → reset accumulation)
  - Automatic disk persistence (no manual save() calls needed)
  - System settings excluded from undo/redo history
- **See**: [docs/projects/local-storage-system.md](docs/projects/local-storage-system.md) for complete design

### Important Implementation Details
- Using **ping-pong accumulation** (not atomic) for better performance
- Using **JSON** for serialization (not RON as in outline)
- **Precision Limitation (f32 vs Apophysis double):**
  - Apophysis uses 64-bit `double` for variation weights and parameters (±1E308, ~15-16 digit precision)
  - We use 32-bit `f32` (±3.4E38, ~7 digit precision) - **WGSL has no f64 support**
  - Impact: Minimal for typical flames, may cause slight differences at extreme values
  - See [docs/projects/apophysis-full-compatibility.md](docs/projects/apophysis-full-compatibility.md) Phase 2.0 for details
- **Undo/redo** system with 50-state history
- **Full WASM support** for web builds (100% complete including PNG export)
- GPU buffers use **std430 layout** (storage buffers) and **std140 layout** (uniform buffers) for cross-platform compatibility
- **WASM shader compatibility:** Use `textureLoad()` instead of `textureSample()` inside non-uniform control flow (browser WebGPU strictly enforces WGSL spec, desktop drivers are lenient)

### Current Limitations
- No transform clone/duplicate button
- No randomize button

### Build Commands
```bash
# Desktop GUI (Windows/macOS/Linux)
cargo run --release

# Headless CLI Export (batch PNG generation)
cargo run --release -- export --input tests/visual/configs --output tests/visual/current
# See CLI Export section below for details

# WASM (Web)
wasm-pack build --target web --release

# iOS (experimental - requires dependency fixes)
cargo build --target aarch64-apple-ios
# Known issues: 'rfd' crate not compatible with iOS

# Android (experimental - requires dependency configuration)
cargo build --target aarch64-linux-android
# Known issues: 'android-activity' needs specific features enabled

# Note: Mobile builds are not fully functional yet but may be possible
# with additional work on platform-specific dependencies
```

### Testing & Profiling

See [docs/TESTING-GUIDE.md](docs/TESTING-GUIDE.md) for complete guide.

```bash
# Unit tests (embedded in source files)
cargo test

# Unified benchmark suite (CPU + GPU + visual regression)
python scripts/run_benchmarks.py          # Full suite
python scripts/run_benchmarks.py --quick  # Quick mode (skip WASM)

# Main app
cargo run --release
```

**Unified Benchmark Suite** (`scripts/run_benchmarks.py`):
- **CPU Microbenchmarks**: Criterion benchmarks with statistical analysis (5 runs, warmup)
- **GPU Desktop Rendering**: Headless PNG export tests (800×600, multiple runs)
- **GPU WASM Rendering**: Browser-based WebGPU tests via Selenium (800×600)
- **Visual Regression**: Pixel-perfect hash comparison (baseline vs current, desktop + WASM)
- **Performance Tracking**: CSV history with previous 2 runs for regression detection
- **Color-coded output**: Green (>2% faster), yellow (>5% slower), red (>10% slower)

**What's Tested:**
- Unit tests: Transform math, variations, palette interpolation, version info
- CPU benchmarks: Affine transforms, point helpers (r/θ/φ), all 26 core variations
- GPU benchmarks: 8 visual test configs (variations, presets, 3D, tone mapping)
- Visual regression: SHA256 hash comparison of pixel data (baseline vs current)

**All tests passing:** ✅ 15+ unit tests, 24 CPU benchmarks, 16 GPU benchmarks (8 desktop + 8 WASM), visual regression checks

### CLI Export Mode

The main app supports headless batch PNG export for testing and automation:

```bash
# Export single file
fractal_flame_wgpu export -i config.fflame -o output.png --width 1920 --height 1080

# High-resolution export (automatically uses CPU histogram for large sizes)
fractal_flame_wgpu export -i config.fflame -o output.png --width 4000 --height 4000

# Batch export directory
fractal_flame_wgpu export -i tests/visual/configs -o tests/visual/current

# With test category metadata
fractal_flame_wgpu export -i tests/visual/configs/variations -o tests/visual/current --category variations
```

**Features:**
- **High-resolution support**: Exports at any resolution (4K, 8K, or larger)
- Automatic GPU/CPU path selection based on resolution
- Renders exact `max_iterations` from config for reproducibility
- Progress indicator shows iteration count and percentage
- Full PNG metadata embedding (build info, config, render settings)
- Batch processes entire directories
- Headless GPU rendering (no window required)

**High-Resolution Export Architecture:**
- **GPU path** (≤128MB histogram): Fast GPU-only rendering
- **CPU path** (>128MB histogram): GPU compute + CPU histogram + GPU tonemap
- Threshold: 4K (3840×2160) uses GPU, larger uses CPU path
- ~24 seconds for 4000×4000 @ 10M iterations

**Performance:**
- 128 workgroups × 256 iterations = 32,768 iterations/dispatch
- Automatically calculates dispatch count from `config.max_iterations`
- Example: 10M iterations @ 800x600 renders in ~0.5 seconds
- Example: 10M iterations @ 4000x4000 renders in ~24 seconds (CPU path)

**Output:**
- PNG files with embedded metadata (see PNG Metadata section below)
- Named after flame config name (lowercase, underscores)
- Includes test category if provided

### PNG Metadata

All exported PNGs include comprehensive metadata in tEXt chunks:

**Build Information:**
- Version, git hash, branch, timestamp
- Platform, rustc version, build profile

**Render Settings:**
- Resolution, total iterations, render time
- Frame count, workgroups, iterations per dispatch

**Flame Configuration:**
- Complete FractalConfig serialized as JSON
- SHA256 checksum of config for verification
- All transforms, variations, parameters

**Display Settings:**
- Background color, exposure, gamma
- Tone curve usage, tonemap mode

**Test Support:**
- Optional test_name and test_category fields
- For visual regression testing

**Reading Metadata:**
```rust
use fractal_flame_wgpu::png_metadata::read_png_metadata;

let metadata = read_png_metadata("output.png")?;
println!("Rendered {} iterations in {:.2}ms",
    metadata.total_iterations, metadata.render_time_ms);
```

### WASM API

The WASM build exposes a JavaScript API for headless PNG export in browsers:

```javascript
import init, { WasmApi } from './pkg/fractal_flame_wgpu.js';

// Initialize WASM module
await init();

// Create API instance
const api = new WasmApi();

// Load config JSON
const config = await fetch('config.fflame').then(r => r.json());
api.load_config_json(JSON.stringify(config));

// Export to PNG (returns Uint8Array)
const pngData = await api.export_png(
    800,    // width
    600,    // height
    256,    // iterations_per_thread
    false   // transparent (true for transparent PNG, false for opaque with background)
);

// Download PNG
const blob = new Blob([pngData], { type: 'image/png' });
const url = URL.createObjectURL(blob);
const a = document.createElement('a');
a.href = url;
a.download = 'fractal.png';
a.click();
```

**Browser Compatibility:**
- ✅ **Chrome/Chromium 113+** - Fully tested, all features working
- ✅ **Firefox 121+** - Fully tested, all features working
- ⚠️ **Safari** - WebGPU support experimental, may require flags
- ❌ **Mobile browsers** - WebGPU support limited/experimental

**Limitations:**
- WebGL fallback not possible (compute shaders required)
- Uses `downlevel_webgl2_defaults()` limits for broader compatibility
- 1D textures converted to 2D (height=1) for browser WebGPU compatibility

**Performance:**
- Same rendering speed as desktop (~800-1000 M iterations/sec)
- Headless export completes in ~0.5-2 seconds for typical configs
- No performance difference between interactive and headless modes

## Coding Guidelines

### GPU Code
- All shaders use **WGSL** (WebGPU Shading Language)
- Use `@group(0) @binding(N)` for bind groups
- Follow std140/std430 layout rules for buffers
- Use `texture_storage_2d<rgba32float, write>` for output textures
- **WASM Compatibility**: Use `textureLoad()` instead of `textureSample()` inside conditionals
  - Browser WebGPU strictly enforces WGSL uniform control flow requirements
  - `textureSample()` must only be called from uniform control flow
  - Desktop GPU drivers are lenient but WASM will fail silently with black output
- **IMPORTANT**: Trust the shader compiler for optimization
  - Modern GPU compilers (SPIR-V, DXC, Metal) perform aggressive CSE (Common Subexpression Elimination)
  - Write clear, straightforward code - compiler will optimize redundant calculations
  - Manual "optimizations" often hurt performance (register pressure, function call overhead)
  - See [docs/SHADER_COMPILER_CSE_ANALYSIS.md](docs/SHADER_COMPILER_CSE_ANALYSIS.md) for detailed analysis

### Rust Code
- Use `bytemuck::Pod` and `bytemuck::Zeroable` for GPU data structures
- GPU struct alignment rules:
  - **std140 (uniform buffers)**: vec3/vec4 require 16-byte alignment
  - **std430 (storage buffers)**: vec3 requires 16-byte alignment, arrays more packed
  - **Critical**: Add explicit padding for vec3 fields after large arrays (see GpuTransform)
- Prefer `&Queue::write_buffer()` over buffer mapping for updates
- Use `CommandEncoder` for GPU operations, submit once per frame

### State Management (Simplified System - Completed 2025-11-17)
**All configuration changes now flow through ConfigManager** - see [docs/main/CONFIG.md](docs/main/CONFIG.md) for complete documentation.

**Core Principles:**
- ConfigManager automatically handles undo/redo with delta tracking
- Type-safe `ConfigPath` enum identifies all parameters (100+ variants)
- `UpdateType` return value determines selective GPU updates (View/Color/Flame/ToneMap)
- **All updates are immediate** - no lazy/preview distinction needed
- **Coalescing** automatically merges rapid changes within 2-second window
- **100ms overwrite window** provides smooth real-time updates for all parameter types
- Batch updates group multiple parameter changes into single undo point

**UI Patterns:**

**1. Single Parameter Change** (slider, drag, button, checkbox):
```rust
let response = ui.add(egui::Slider::new(&mut value, 0.0..=1.0).text("Parameter"));
if response.changed() {
    config_manager.update_param(path, value.into())?;
}
// That's it! Coalescing and overwrite mode handle the rest automatically.
```

**2. Batch Update** (multiple related parameters):
```rust
let changes = vec![
    (ConfigPath::TransformAffine { index, param: A }, a.into()),
    (ConfigPath::TransformAffine { index, param: B }, b.into()),
    // ... more params
];
config_manager.update_batch(changes, "Description")?;
```

**How It Works:**
- **Any change** → GPU updates immediately, coalescing tracks for undo/redo
- **Rapid changes** (within 2s) → Merged into single undo point
- **Overwrite mode** → Enabled for 100ms after parameter changes for smooth transitions
- **Iteration reset** → Triggered when overwrite window expires for clean rebuild

**Architecture Evolution:**
- **2025-10**: Created ConfigManager with delta-based undo/redo (PR #22)
- **2025-11**: Removed preview mode system, simplified to direct updates (PR #23)
- **Result**: 800+ lines removed, real-time updates for all controls, no blank frames

**Key Files:**
- `src/config/manager.rs` - ConfigManager with simplified update system (~1,100 lines)
- `src/config/delta.rs` - ConfigPath, ConfigValue, ConfigDelta enums (568 lines)
- `src/app/mod.rs` - 100ms overwrite window + iteration counter reset logic
- `src/ui/triangle_editor.rs` - Batch updates for grouped parameter changes

### Variation Registry Architecture
- **Global Singleton**: `global_registry()` returns `&'static VariationRegistry` (initialized once via `once_cell::Lazy`)
- **Two-Tier ID System**: Core variations have fixed IDs, plugins get dynamic IDs
  - **Core variations (0-25)**: Fixed position in `ordered_names`, never changes
    - `ordered_names[0]` = "linear" → always shader index 0
    - `ordered_names[1]` = "sinusoidal" → always shader index 1
    - Ensures backward compatibility with existing presets
  - **Plugin variations (26-49)**: Dynamically assigned per-flame based on active usage
    - Registry can hold unlimited plugins (hundreds/thousands)
    - Only active plugins get assigned shader indices 26-49
    - Shader is dynamically compiled with the specific active set
- **Registration Order**: Fixed for core (0-25), append-only for plugins
  - Indices 0-23: Original 24 variations (NEVER REORDER - breaks presets!)
  - Indices 24-25: JuliaN, Blob (added later, placed at end for backward compatibility)
  - Indices 26+: Plugin variations (stored in registry but ID assigned dynamically)
- **Data Structures**:
  - `variations: HashMap<String, VariationInfo>` - Name → metadata lookup (unlimited size)
  - `ordered_names: Vec<String>` - Core variations list (defines indices 0-25)
  - Transform stores `HashMap<String, f32>` for active variations (any from registry)
  - GPU upload: Core variations use fixed indices, active plugins fill 26-49
  - Shader compilation: Generate code for exactly the active set (up to 50 total)
- **UI Ordering Rule**: Sort by category first, then by registration order within category
  - WRONG: `variations.values().filter(category)` ❌ (HashMap iteration is random!)
  - RIGHT: `ordered_names.iter().filter_map().filter(category)` ✅ (preserves order)

### Performance
- Target 60+ FPS at 1080p
- Default: 128 workgroups × 64 threads × 256 iterations per frame
- Progressive refinement: each frame adds more samples
- Track total iterations for quality measurement

## Common Tasks

### Adding a New Variation

#### 2D Variation (affects XY only)
1. Register in `VariationRegistry::new()` in `src/variations/mod.rs`:
   ```rust
   registry.register_core("myvar", "My Variation", VariationCategory::Advanced2D, false);
   ```
2. Add WGSL implementation to both shaders:
   - `shaders/core/variations_2d.wgsl` (2D shader)
   - `shaders/core/variations_3d.wgsl` (3D shader - pass Z through: `vec3(new_x, new_y, p.z)`)
3. Function signature depends on needs:
   - Basic: `fn variation_myvar(p: vec2<f32>) -> vec2<f32>`
   - Needs RNG: `fn variation_myvar(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32>`
   - Has parameters: `fn variation_myvar(p: vec2<f32>, xform_id: u32) -> vec2<f32>`
   - Both: `fn variation_myvar(p: vec2<f32>, xform_id: u32, rng: ptr<function, RngState>) -> vec2<f32>`
4. Shader builder automatically detects signature based on `needs_rng` and `!parameters.is_empty()`
5. Variation automatically appears in UI under its category

#### 3D Variation (affects Z or rotates)
1. Register in `VariationRegistry::new()` (indices 16-23 reserved for 3D):
   ```rust
   registry.register_core("myvar", "My Variation", VariationCategory::Depth3D, false);
   ```
2. Add WGSL implementation to `shaders/core/variations_3d.wgsl`:
   - **Z-only variations**: Modify `result.z` directly (e.g., `result.z *= scale`)
   - **Rotation variations**: Apply rotation matrix to full `result` vector
   - **Full 3D variations**: Use `result += weight * variation(p)`
3. Only visible in 3D mode UI
4. CPU reference can return `p` unchanged (CPU is 2D only)

#### Parameterized Variation (with custom parameters)
1. Register variation (as above)
2. Add parameters using `registry.add_parameters()`:
   ```rust
   registry.add_parameters("myvar", vec![
       VariationParameter {
           name: "power".to_string(),
           display_name: "Power".to_string(),
           param_type: ParamType::Integer,
           default_value: 2.0,
           min_value: Some(-10.0),
           max_value: Some(10.0),
       },
   ]);
   ```
3. In shader, access parameters via `get_param()`:
   ```wgsl
   fn variation_myvar(p: vec2<f32>, xform_id: u32) -> vec2<f32> {
       let power = get_param(xform_id, VARIATION_INDEX, 0u);
       // Use power in calculation...
   }
   ```
4. Parameter sliders automatically appear in UI below variation
5. Supports Float, Integer, and Angle (0-360°) parameter types

### Adding a New Palette
**Option 1: Code-based (built-in)**
1. Add function in `src/scene/palette.rs` (follow `Palette::fire()` pattern)
2. Add to `PaletteLibrary::new()` constructor
3. Palette auto-appears in UI dropdown

**Option 2: File-based (auto-loaded from assets/)**
1. Create a `.palette` file in `assets/palettes/` directory
2. File is auto-loaded on desktop builds (see `PaletteLibrary::new()`)
3. WASM builds use built-in palettes only

**Option 3: Import/Export (user palettes)**
1. Use Palette Editor → Import/Export Palette section
2. Export to clipboard or save as `.palette` file
3. Import from JSON text or load `.palette` file
4. Imported palettes automatically added to library

### Adding a New Preset
**Option 1: Code-based (built-in)**
1. Add function in `src/scene/presets.rs` (follow existing patterns)
2. Add to `PresetLibrary::new()` constructor (wrap in `flame_to_config()`)
3. Preset auto-appears in UI dropdown

**Option 2: File-based (auto-loaded from assets/)**
1. Create a `.fflame` file in `assets/presets/` directory (FractalConfig JSON)
2. File is auto-loaded on desktop builds (see `PresetLibrary::new()`)
3. WASM builds use built-in presets only
4. Use `cargo run --example export_presets` to generate preset files from code

**Option 3: Export current state as preset**
1. Use Config Import/Export → Save Config
2. Save as `.fflame` file in `assets/presets/`
3. Restart app to see it in preset dropdown (desktop only)

### Modifying Tone Mapping
1. Edit `shaders/tonemap.wgsl`
2. Update `TonemapParams` in `src/gpu/buffers.rs` if adding parameters
3. Update UI in `src/ui/mod.rs` if exposing new controls

### Adding UI Controls
- All UI is in `src/ui/mod.rs` `render_ui()` function
- Return changes via `UiResponse` struct
- Handle responses in `src/app.rs` `render()` function

### Creating 3D Presets
1. Set `flame.render_mode = RenderMode::ThreeD`
2. Set projection: `flame.projection = ProjectionType::Perspective { strength: 2.0-5.0 }`
3. Use 3D variations (indices 16-23) for Z manipulation:
   - **Zcone**: Creates cone shape in Z (Z = distance from origin)
   - **Flatten**: Compresses Z toward zero (good for controlling depth)
   - **Hemisphere**: Projects onto sphere surface (full 3D structure)
   - **PreRotateY/PostRotateY**: Add spiral/twist in 3D space
   - **ZScale**: Scale Z depth up or down
4. Set different `g` (Z offset) values per transform to create layers
5. Test with camera rotation (Camera Pitch/Yaw sliders) to verify 3D structure
6. Save as `.fflame` file with 24-element variation arrays

**Example 3D Transform:**
```rust
let mut xform = Transform::new();
xform.a = 0.7; xform.d = 0.7;  // Affine (affects XY)
xform.g = 0.3;                  // Z offset
xform.variations[0] = 0.5;      // Linear (2D base)
xform.variations[16] = 0.5;     // Zcone (3D depth)
```

## Dependencies
See @Cargo.toml for full dependency list

Key dependencies:
- **wgpu 23.0** - WebGPU API
- **winit 0.30** - Window management
- **egui 0.33** - Immediate mode UI
- **egui_dock 0.18** - Docking panel system (added 2025-11-13)
- **rust-i18n 3.1** - Internationalization support (added 2025-11-13)
- **serde + serde_json** - Serialization
- **image** - PNG export
- **bytemuck** - GPU data layout

## File Formats

### Palette Files (.palette)
JSON format with name and color stops:
```json
{
  "name": "My Palette",
  "stops": [
    {
      "position": 0.0,
      "color": [1.0, 0.0, 0.0]
    },
    {
      "position": 0.5,
      "color": [0.0, 1.0, 0.0]
    },
    {
      "position": 1.0,
      "color": [0.0, 0.0, 1.0]
    }
  ]
}
```
- `position`: 0.0 to 1.0 (gradient stop position)
- `color`: RGB array with values 0.0 to 1.0

### Config Files (.fflame)
JSON format containing full fractal state (see [src/config.rs](src/config.rs))

**FractalConfig includes ALL settings for exact reproduction:**
- Flame definition (transforms, variations, variation parameters, colors)
- View state (zoom, pan, rotation, camera rotation)
- Rendering settings (density_scale, speed_factor, **max_iterations**)
- Color settings (color_mode, palette_index, background_color, **actual palette data**)
- Tone mapping (**tonemap_mode, tonemap_curve, use_curve**, exposure, gamma)
- Reproducibility (**deterministic_rng** flag)

**Added 2025-10-24 for full reproducibility:**
- `palette: Option<Palette>` - Embeds actual palette data (not just library index)
- `use_curve: bool` - Whether to apply tone curve (default: true)
- `max_iterations: u64` - Exact iteration count for tests (default: 1 billion)
- `deterministic_rng: bool` - Enable reproducible RNG (default: false)

This ensures configs can **exactly recreate** fractals via JSON import/export.

## Important Implementation Notes

### Preset System (Added 2025-10-20)
The preset system stores **complete FractalConfig** (not just Flame):
- Includes flame definition (transforms, variations, colors)
- Includes view state (zoom, pan, rotation)
- Includes rendering settings (density_scale, speed_factor, max_iterations)
- Includes color settings (color_mode, palette_index, background_color, palette data)
- Includes tone mapping (tonemap_mode, tonemap_curve, use_curve, exposure, gamma)

**Key Implementation Details:**
1. **Transform Buffer Sizing** - Pre-allocated for `MAX_TRANSFORMS` (32) to support any preset
2. **Zero Padding** - When writing N transforms, remaining slots are zeroed to prevent residual data
3. **Atomic Loading** - `FlameRenderer::load_config()` ensures all GPU state is synchronized atomically
4. **Reset Behavior** - `reset()` only clears accumulation buffers, never overwrites GPU params

**Critical Bug Fixes (2025-10-20):**
- Fixed buffer overrun when loading presets with more transforms than initial flame
- Fixed residual transforms appearing when switching from larger to smaller preset
- Fixed `reset()` overwriting `num_transforms` after it was correctly set
- Fixed frame presentation timeout when switching presets multiple times

### Asset Loading System (Added 2025-10-20)
Desktop builds auto-load from filesystem:
- `assets/palettes/*.palette` → PaletteLibrary
- `assets/presets/*.fflame` → PresetLibrary
WASM builds use built-in assets only (no filesystem access)

### Rotation-Aware Panning (Added 2025-10-24)
All panning inputs now respect view rotation for intuitive navigation:

**Input Methods:**
- **Mouse drag**: Left-click and drag to pan
- **Keyboard**: Arrow keys (↑↓←→)
- **UI buttons**: View window arrow controls

**Behavior:**
- When rotation = 0°: Standard axis-aligned panning
- When rotation ≠ 0°: Pan direction rotates with view
- Example at 90° rotation: Right arrow pans in original "up" direction

**Implementation:**
- Applies inverse rotation matrix to screen-space movements
- Converts screen deltas to fractal-space coordinates
- Formula: `fractal_delta = rotate(screen_delta, -rotation)`
- All three input methods use identical rotation logic

This ensures panning always moves in the direction you see on screen.

### 3D Rendering System (Added 2025-10-21)
Full pseudo-3D rendering inspired by Apophysis 7X:

**Architecture:**
- **Dual Shaders**: `trajectory.wgsl` (2D) and `trajectory_3d.wgsl` (3D) - selected at runtime
- **Variation System**: 24 total variations (16 2D + 8 3D)
- **Z Tracking**: 3D shader tracks `vec3<f32>` throughout iteration, 2D uses `vec2<f32>`
- **Camera System**: Full 3D camera rotation (pitch/yaw) applied before projection
- **Projection**: Orthographic (flat) or Perspective (depth-aware with configurable strength)

**Key Implementation:**
1. **Affine Transform**: 2D affine (a,b,c,d,e,f) + Z offset (g)
2. **Variation Blending**:
   - 2D variations (0-15): Pass Z through unchanged `vec3(new_x, new_y, p.z)`
   - Z-only variations (16,17,23): Modify `result.z` directly to avoid affecting XY
   - Full 3D variations (18-22): Use standard `result += weight * variation(p)`
3. **Camera Rotation**: Applied in `world_to_pixel()` before projection
   - Yaw (Y-axis): Left/right orbit
   - Pitch (X-axis): Up/down orbit
4. **Projection**: Applied after camera rotation to convert vec3 → vec2 for display

**Backward Compatibility:**
- Old preset files (16 variations) auto-padded with zeros for 3D variations (17-24)
- 2D shader updated to 24-element arrays (ignores indices 16-23)
- Custom deserializer handles both 16 and 24-element variation arrays

**Performance:**
- No measurable difference between 2D and 3D modes
- Pipeline selected at runtime based on `flame.render_mode`
- Same accumulation/tonemap passes for both modes

### Histogram Color Accumulation System (Added 2025-10-27)

Thread-safe atomic color accumulation using u32 histogram buffer:
- Format: 4× u32 per pixel (R, G, B, Density)
- Eliminates overflow (4.2B max), proper HDR
- UI: "Histogram Color Scale" slider (default 100.0)

**See [docs/main/COLOR.md](docs/main/COLOR.md)** for complete documentation. Historical investigation in [docs/archive/histogram/](docs/archive/histogram/).

## Known Issues
- Julia variation uses CPU `rand::random()` which doesn't work on GPU (needs RNG passed in)
- No error handling for invalid .fflame or .palette file imports
- Transparent PNG export reads from accumulation buffer (Rgba16Float) and applies tone mapping on CPU
  - This is necessary because tonemap shader blends RGB with background before alpha is applied
  - Accumulation buffer stores raw fractal colors with separate density channel

## State Management System Complete ✅

The simplified state management system is **complete** as of 2025-11-17:
- ✅ All UI controls use ConfigManager with immediate updates
- ✅ Real-time rendering with 100ms overwrite window (no blank frames)
- ✅ Automatic coalescing merges rapid changes (2 second window)
- ✅ Triangle Editor with batch updates for multi-param changes
- ✅ Automatic change tracking with 50-state undo/redo history
- ✅ Selective GPU updates via UpdateType enum

**Remaining Non-Config Actions** (by design):
- Transform add/delete buttons (structural changes, not parameter edits)
- Config import/export (file I/O operations)
- Preset loading (bulk config replacement)

These operations are intentionally separate from ConfigManager as they represent discrete actions, not incremental parameter changes. See [docs/archive/delta-migration/](docs/archive/delta-migration/) and [docs/archive/remove-preview-mode.md](docs/archive/remove-preview-mode.md) for evolution history.

## Mobile Platform Support (Experimental)

**Status:** Cross-compilation works, but runtime execution requires dependency fixes.

### iOS (aarch64-apple-ios)
```bash
cargo build --target aarch64-apple-ios
```

**Blockers:**
- **rfd** (file dialogs) - Not compatible with iOS
  - Solution: Conditional compilation to disable file dialogs on iOS, or use platform-specific alternatives
  - Impact: Config import/export, palette import/export, PNG export would need iOS-native file pickers

**Potential Solutions:**
- Use `#[cfg(not(target_os = "ios"))]` to exclude rfd on iOS
- Implement iOS-native file picker using `objc` or Swift interop
- Share via iOS share sheet instead of file dialogs

### Android (aarch64-linux-android)
```bash
cargo build --target aarch64-linux-android
```

**Blockers:**
- **android-activity** - Requires specific cargo features to be enabled
  - Needs proper Android app manifest and activity configuration
  - winit may need Android-specific initialization

**Potential Solutions:**
- Add `android-activity` with correct features to Cargo.toml
- Create Android-specific build configuration
- Use `cargo-apk` or `xbuild` for easier Android packaging

### General Mobile Considerations
- **Touch controls** - Current UI is mouse/keyboard focused
- **Performance** - Mobile GPUs may need lower default iteration counts
- **Screen sizes** - UI scaling for smaller displays and portrait mode
- **File access** - Platform-specific storage APIs (iOS sandbox, Android storage permissions)
- **App packaging** - Need proper mobile app bundles (.ipa for iOS, .apk/.aab for Android)

**Feasibility:** Medium to High - The core rendering engine should work on mobile GPUs (wgpu/WebGPU supports mobile), but the surrounding infrastructure (file I/O, UI, windowing) needs platform-specific adaptations.

## Optional/Future Features

Features that could be added in future development (see [docs/STATUS.md](docs/STATUS.md) for detailed priority breakdown):

### High Priority
- **Randomize button** - Generate random flames with seeded generation for exploration
- **Async export progress UI** - Currently export blocks the UI during rendering
- **Depth effects for 3D mode** - Optional visual enhancements:
  - Depth-based coloring (Z → color heat map)
  - Depth of field blur (focus plane + bokeh)
  - Z-fog/atmospheric depth
  - Depth buffer visualization

### Medium Priority
- **Final transform support** - Code exists but no UI controls (post-processing transform applied after all iterations)
- **Transform clone/duplicate** - UI button to duplicate existing transforms
- **EXR/HDR export** - High dynamic range output formats for compositing
- **Visual regression tests** - Automated testing with image checksums
- **Performance profiling/optimization** - Systematic GPU profiling and tuning
- **More 3D variations** - Additional depth-manipulating variations (curl_3d, splits_3d, etc.)

### Low Priority / Future Expansions
- **CLI interface** - Headless rendering from command line (clap already in deps)
- **Headless export example** - Render without window for batch processing
- **Animation system** - Keyframe timeline, transform morphing, parameter interpolation
- **CUDA backend** - NVIDIA-specific acceleration (desktop only)
- **Layered compositing** - Multiple flames blended together
- **Adaptive sampling** - Focus iterations on high-detail areas
- **Denoising** - AI or traditional denoising for faster convergence

### Nice to Have
- ~~**Preset browser UI**~~ ✅ Implemented 2025-11-24 - See [PRESET-BROWSER.md](docs/main/PRESET-BROWSER.md)
- **Palette library management** - Save/organize custom palettes permanently
- **Transform presets** - Save/load individual transform configurations
- **Batch export** - Render multiple configurations automatically
- **Video export** - Animate parameters over time and render to video

See [docs/outline.md](docs/outline.md) Section 14 for more ambitious future expansion ideas.
