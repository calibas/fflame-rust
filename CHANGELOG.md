# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Added - 2025-10-20

#### Preset System with Asset Loading
- **Preset Selector UI**
  - Dropdown selector in Performance window
  - Instant loading when selecting different presets
  - Five built-in presets: Simple, Complex, Spherical, Spiral, Julia

- **Asset Folder Auto-Loading** (Desktop only)
  - Auto-load presets from `assets/presets/*.flame` files
  - Auto-load palettes from `assets/palettes/*.palette` files
  - New `src/scene/assets.rs` module for filesystem-based asset discovery
  - WASM builds use built-in assets only (no filesystem access)

- **Preset File Generation**
  - `examples/export_presets.rs` to export built-in presets to files
  - Run with `cargo run --example export_presets`
  - Generates `.flame` files in `assets/presets/`

- **Enhanced Renderer API**
  - `FlameRenderer::load_config()` - Comprehensive atomic FractalConfig loading
  - Ensures all GPU state (transforms, params, palette, color mode) is synchronized
  - Pre-allocated transform buffer for MAX_TRANSFORMS (32)

#### Palette Import/Export System
- **Palette Import/Export UI**
  - Export palette to clipboard as JSON
  - Save palette as `.palette` file (desktop) or download (WASM)
  - Import palette from JSON text
  - Load palette from `.palette` file (desktop) or clipboard (WASM)
  - Palettes include a `name` field in the JSON format
  - Imported palettes automatically added to palette library
  - Palette editor automatically updated with imported palette
  - Full cross-platform support (desktop + WASM)

### Changed - 2025-10-20

#### Preset System Architecture
- **PresetLibrary** changed from `Vec<Flame>` to `Vec<FractalConfig>`
  - Presets now store complete fractal state (flame + view + color + rendering settings)
  - Preset files are `.flame` files containing full FractalConfig JSON
  - Breaking change: Old preset files (Flame-only) are incompatible

- **Flame Structure**
  - Added `name: String` field to Flame struct
  - Changed from `derive(Default)` to manual Default impl with default name "Untitled"

#### Transform Buffer Management
- **Transform buffer** now pre-allocated for MAX_TRANSFORMS (32) instead of dynamic sizing
  - Prevents buffer overrun when loading presets with more transforms
  - Added `MAX_TRANSFORMS = 32` constant in `src/gpu/buffers.rs`
  - `update_transforms()` now zero-fills unused transform slots
  - Eliminates residual transform data when switching between presets

#### Renderer Reset Behavior
- **`FlameRenderer::reset()`** now only clears accumulation buffers
  - No longer updates GPU params (prevents overwriting num_transforms)
  - Parameter updates are responsibility of specific update functions
  - `update_flame()` and `load_config()` handle params correctly

#### Import/Export Flow
- **`import_config()`** simplified to use comprehensive `load_config()`
  - Single atomic operation instead of multiple sequential updates
  - Ensures correct order: transforms → color mode → palette → params → clear
  - Eliminates race conditions and state inconsistencies

### Fixed - 2025-10-20

#### Critical Preset System Bugs
- **Fixed buffer overrun crash** when loading Complex preset (4 transforms)
  - Root cause: Transform buffer sized for initial flame's transform count
  - Solution: Pre-allocate buffer for MAX_TRANSFORMS (32)

- **Fixed residual transforms** appearing when switching from larger to smaller preset
  - Root cause: Old transform data remained in GPU buffer beyond written range
  - Example: Complex (4 transforms) → Simple (2 transforms) left transforms 3-4 in memory
  - Solution: Zero-fill all 32 transform slots when updating, not just N transforms

- **Fixed num_transforms corruption** after loading preset
  - Root cause: `reset()` was overwriting GPU params after `update_flame()` set them correctly
  - Solution: `reset()` now only clears buffers, never touches params

- **Fixed frame timeout** when switching presets multiple times
  - Root cause: Early `return Ok()` prevented `frame.present()` from being called
  - Solution: Use flag-based approach that skips updates but still presents frame

#### Other Fixes
- Removed debug "🔄 Load Selected Preset" button (no longer needed)
- Removed debug console logging for preset changes

### Implementation Details
- Files modified:
  - `src/scene/transforms.rs` - Added name field to Flame
  - `src/scene/presets.rs` - Changed to FractalConfig storage
  - `src/scene/assets.rs` - New file for asset loading
  - `src/scene/palette.rs` - Auto-load from assets/palettes/
  - `src/ui/mod.rs` - Added preset selector and palette import/export UI
  - `src/app.rs` - Simplified import_config, added preset/palette handlers
  - `src/renderer/compute_kernel.rs` - Added load_config(), fixed reset()
  - `src/gpu/buffers.rs` - Pre-allocated buffer, zero-fill transforms
  - `examples/export_presets.rs` - New file for exporting presets
- JSON format matches existing `Palette` and `FractalConfig` struct serialization
- Reuses existing file dialog patterns from config import/export

#### JSON Format Example
```json
{
  "name": "My Custom Palette",
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

---

## [0.1.0] - Initial Release

### Features
- GPU-accelerated fractal flame rendering using wgpu
- 16 variation functions (Linear, Sinusoidal, Spherical, etc.)
- Three color modes: Transform, Palette, Speed
- Interactive palette editor with gradient stops
- Real-time progressive refinement
- Full undo/redo system (50 states)
- Config import/export (.flame files)
- PNG export with transparency support
- Full WASM support for web builds
- Mouse and keyboard viewport navigation
- Performance metrics display

See [STATUS.md](STATUS.md) for detailed implementation status.
