# Fractal Flame Renderer - Project Context

## Overview
See @STATUS.md for implementation status vs original design
See @ARCHITECTURE.md for code organization and data flow
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
- No preset browser (presets are code-based)
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
1. Add function in `src/scene/palette.rs` (follow `Palette::fire()` pattern)
2. Add to `PaletteLibrary::new()` constructor
3. Palette auto-appears in UI dropdown

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

## Known Issues
- Julia variation uses CPU `rand::random()` which doesn't work on GPU (needs RNG passed in)
- WASM PNG export uses `unsafe` lifetime extension (safe in practice, GPU resources live for program lifetime)
- No error handling for invalid .flame file imports
- Background color changes don't trigger undo capture in all cases
