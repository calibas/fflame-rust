# Fractal Flame Renderer - Project Context

## Overview
See @STATUS.md for implementation status vs original design
See @ARCHITECTURE.md for code organization and data flow
See @CHANGELOG.md for recent changes and release notes
See @outline.md for original design goals

## Quick Reference

### Project Structure
- **Shaders**: All WGSL shaders in `shaders/` directory
  - `trajectory.wgsl` - Flame iteration compute shader
  - `accumulate.wgsl` - Temporal blending
  - `tonemap.wgsl` - Display rendering

- **Core Modules**:
  - `src/app.rs` - Main application loop and event handling
  - `src/renderer/compute_kernel.rs` - GPU rendering orchestration
  - `src/scene/transforms.rs` - Flame algorithm (CPU + GPU)
  - `src/ui/mod.rs` - All UI panels and controls

### Key Concepts
- **Fractal Flames**: IFS (Iterated Function System) with variations
- **16 Variations**: Linear, Sinusoidal, Spherical, Swirl, Horseshoe, Polar, Handkerchief, Heart, Disc, Spiral, Hyperbolic, Diamond, Ex, Julia, Bent, Waves
- **3-Pass Rendering**: Compute samples → Accumulate temporally → Tonemap for display
- **Ping-Pong Accumulation**: Two textures swapped each frame for progressive refinement
- **Color Modes**: Transform colors, Palette lookup, Speed-based coloring

### Important Implementation Details
- Using **ping-pong accumulation** (not atomic) for better performance
- Using **JSON** for serialization (not RON as in outline)
- **Undo/redo** system with 50-state history
- **Full WASM support** for web builds (100% complete including PNG export)
- All GPU params use **std140 layout** for cross-platform compatibility

### Current Limitations
- PNG export only at current viewport resolution (no tiled high-res export)
- No UI for adding/removing transforms (can only edit existing)
- No randomize button

### Build Commands
```bash
# Desktop
cargo run --release

# WASM
wasm-pack build --target web --release
```

### Testing
```bash
cargo test
```

## Coding Guidelines

### GPU Code
- All shaders use **WGSL** (WebGPU Shading Language)
- Use `@group(0) @binding(N)` for bind groups
- Follow std140/std430 layout rules for buffers
- Use `texture_storage_2d<rgba32float, write>` for output textures

### Rust Code
- Use `bytemuck::Pod` and `bytemuck::Zeroable` for GPU data structures
- All GPU params should be aligned to vec4 boundaries
- Prefer `&Queue::write_buffer()` over buffer mapping for updates
- Use `CommandEncoder` for GPU operations, submit once per frame

### State Management
- Call `app.capture_state()` before making changes (for undo)
- Reset accumulation when view/flame/palette changes
- Use `view_changed_by_keyboard` flag pattern for deferred updates

### Performance
- Target 60+ FPS at 1080p
- Default: 128 workgroups × 64 threads × 256 iterations per frame
- Progressive refinement: each frame adds more samples
- Track total iterations for quality measurement

## Common Tasks

### Adding a New Variation
1. Add to `VariationType` enum in `src/scene/transforms.rs`
2. Implement CPU version in `VariationType::apply()`
3. Add GPU version in `shaders/trajectory.wgsl` `apply_variation()` function
4. Update `MAX_VARIATIONS` if needed
5. Add to UI variation list in `src/ui/mod.rs`

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
1. Create a `.flame` file in `assets/presets/` directory (FractalConfig JSON)
2. File is auto-loaded on desktop builds (see `PresetLibrary::new()`)
3. WASM builds use built-in presets only
4. Use `cargo run --example export_presets` to generate preset files from code

**Option 3: Export current state as preset**
1. Use Config Import/Export → Save Config
2. Save as `.flame` file in `assets/presets/`
3. Restart app to see it in preset dropdown (desktop only)

### Modifying Tone Mapping
1. Edit `shaders/tonemap.wgsl`
2. Update `TonemapParams` in `src/gpu/buffers.rs` if adding parameters
3. Update UI in `src/ui/mod.rs` if exposing new controls

### Adding UI Controls
- All UI is in `src/ui/mod.rs` `render_ui()` function
- Return changes via `UiResponse` struct
- Handle responses in `src/app.rs` `render()` function

## Dependencies
See @Cargo.toml for full dependency list

Key dependencies:
- **wgpu 23.0** - WebGPU API
- **winit 0.30** - Window management
- **egui 0.30** - Immediate mode UI
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

### Config Files (.flame)
JSON format containing full fractal state (see [src/config.rs](src/config.rs))

## Important Implementation Notes

### Preset System (Added 2025-10-20)
The preset system stores **complete FractalConfig** (not just Flame):
- Includes flame definition (transforms, variations, colors)
- Includes view state (zoom, pan, rotation)
- Includes rendering settings (density_scale, speed_factor)
- Includes color settings (color_mode, palette_index, background_color)

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
- `assets/presets/*.flame` → PresetLibrary
WASM builds use built-in assets only (no filesystem access)

## Known Issues
- Julia variation uses CPU `rand::random()` which doesn't work on GPU (needs RNG passed in)
- WASM PNG export uses `unsafe` lifetime extension (safe in practice, GPU resources live for program lifetime)
- No error handling for invalid .flame or .palette file imports
- Background color changes don't trigger undo capture in all cases
- Transparent PNG export reads from accumulation buffer (Rgba16Float) and applies tone mapping on CPU
  - This is necessary because tonemap shader blends RGB with background before alpha is applied
  - Accumulation buffer stores raw fractal colors with separate density channel
