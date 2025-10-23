# Fractal Flame Renderer - Project Context

## Overview
See [docs/STATUS.md](docs/STATUS.md) for implementation status vs original design
See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for code organization and data flow
See [CHANGELOG.md](CHANGELOG.md) for recent changes and release notes
See [docs/WASM-STATUS.md](docs/WASM-STATUS.md) for WebAssembly build status and platform-specific details
See [docs/outline.md](docs/outline.md) for original design goals

## Quick Reference

### Project Structure
- **Shaders**: All WGSL shaders in `shaders/` directory
  - `trajectory.wgsl` - 2D flame iteration compute shader
  - `trajectory_3d.wgsl` - 3D flame iteration compute shader (with camera rotation)
  - `accumulate.wgsl` - Temporal blending
  - `tonemap.wgsl` - Display rendering

- **Core Modules**:
  - `src/app.rs` - Main application loop and event handling
  - `src/renderer/compute_kernel.rs` - GPU rendering orchestration
  - `src/scene/transforms.rs` - Flame algorithm (CPU + GPU)
  - `src/ui/mod.rs` - All UI panels and controls

### Key Concepts
- **Fractal Flames**: IFS (Iterated Function System) with variations
- **Render Modes**: 2D (classic) and 3D (pseudo-3D with depth)
- **26 Variations**: 16 2D + 8 3D + 2 parameterized
  - **2D (0-15)**: Linear, Sinusoidal, Spherical, Swirl, Horseshoe, Polar, Handkerchief, Heart, Disc, Spiral, Hyperbolic, Diamond, Ex, Julia, Bent, Waves
  - **Parameterized 2D (14, 17)**: Julian (power, dist), Blob (high, low, waves)
  - **3D (16, 18-23)**: Zcone, Flatten, Hemisphere, PreRotateX, PreRotateY, PostRotateX, PostRotateY, ZScale
- **Variation Parameters**: Some variations have configurable parameters (power, distance, waves, etc.)
  - Stored per-transform in HashMap
  - Uploaded to GPU via dedicated storage buffer (192 floats: 24 variations × 8 params)
  - Accessible in shaders via `get_param(xform_id, variation_id, param_slot)`
  - UI sliders appear below active variations (Float, Integer, Angle types)
- **3-Pass Rendering**: Compute samples → Accumulate temporally → Tonemap for display
- **Ping-Pong Accumulation**: Two textures swapped each frame for progressive refinement
- **Color Modes**: Transform colors, Palette lookup, Speed-based coloring
- **Projection Types**: Orthographic (flat) and Perspective (depth-aware)
- **Camera Control**: Full 3D camera rotation (pitch and yaw) for viewing from any angle

### Important Implementation Details
- Using **ping-pong accumulation** (not atomic) for better performance
- Using **JSON** for serialization (not RON as in outline)
- **Undo/redo** system with 50-state history
- **Full WASM support** for web builds (100% complete including PNG export)
- All GPU params use **std140 layout** for cross-platform compatibility

### Current Limitations
- PNG export only at current viewport resolution (no tiled high-res export)
- No transform clone/duplicate button
- No randomize button

### Build Commands
```bash
# Desktop (Windows/macOS/Linux)
cargo run --release

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

# Regression tests (integration tests)
cargo test --test regression

# CPU benchmarks (Criterion - precise microbenchmarks)
cargo bench

# Simple benchmark (CLI - human-readable)
cargo run --release --bin simple_benchmark

# Show version info
cargo run --example show_version

# Main app
cargo run --release
```

**What's Tested:**
- Unit tests: Transform math, variations, palette interpolation, version info
- Regression: 12 tests (CPU determinism, all variations, presets, serialization)
- Benchmarks: CPU iteration, all 24 variations, affine, point calculations

**All tests passing:** ✅ 15+ unit tests, 12 regression tests

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
6. Save as `.flame` file with 24-element variation arrays

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

## Known Issues
- Julia variation uses CPU `rand::random()` which doesn't work on GPU (needs RNG passed in)
- WASM PNG export uses `unsafe` lifetime extension (safe in practice, GPU resources live for program lifetime)
- No error handling for invalid .flame or .palette file imports
- Background color changes don't trigger undo capture in all cases
- Transparent PNG export reads from accumulation buffer (Rgba16Float) and applies tone mapping on CPU
  - This is necessary because tonemap shader blends RGB with background before alpha is applied
  - Accumulation buffer stores raw fractal colors with separate density channel

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
- **Tiled high-resolution export** - Currently only exports at viewport resolution; tiled rendering would enable 4K+ exports
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
- **Preset browser UI** - Visual grid of preset thumbnails instead of dropdown
- **Palette library management** - Save/organize custom palettes permanently
- **Transform presets** - Save/load individual transform configurations
- **Batch export** - Render multiple configurations automatically
- **Video export** - Animate parameters over time and render to video

See [docs/outline.md](docs/outline.md) Section 14 for more ambitious future expansion ideas.
