# Changelog

All notable changes to this project will be documented in this file.

## [Unreleased]

### Fixed - 2025-10-21

#### Event Loop Memory Leak and Performance (CRITICAL)
**Root Cause:** Event loop `set_control_flow()` was being called on **every single event** (mouse moves, keyboard presses, window events) instead of once per frame.

**Symptoms:**
- Memory leak in WASM (100MB → 1000MB over time, even when idle/paused)
- Awful performance (10-15 fps despite trying to run "as fast as possible")
- High CPU usage when idle
- Browser garbage collector couldn't keep up with event loop churn

**The Fix** ([src/app.rs:172-175](src/app.rs#L172-L175)):
```rust
// Before (WRONG - called hundreds of times per second):
event_loop.run(move |event, elwt| {
    elwt.set_control_flow(ControlFlow::Poll);  // ❌ On every event!

// After (CORRECT - called once per frame):
Event::AboutToWait => {
    elwt.set_control_flow(ControlFlow::Wait);  // ✅ Event-driven
    window.request_redraw();
}
```

**Impact:**
- ✅ Memory leak eliminated (memory stays under 8MB with proper GC)
- ✅ Performance fixed (smooth 60fps in all modes)
- ✅ CPU usage reduced when idle
- ✅ Browser GC can work properly between frames

**Key Insight:** Using `ControlFlow::Wait` instead of `ControlFlow::Poll` makes the event loop **event-driven** rather than busy-looping. The browser's `requestAnimationFrame` naturally paces frames at 60fps, giving the garbage collector time to clean up GPU resources.

#### WASM Frame Smearing and Performance
Critical bug fixes for WebAssembly builds:

- **Fixed frame accumulation bug in WASM**
  - Root cause: Tonemap pipeline used `BlendState::ALPHA_BLENDING`, causing frames to blend instead of replace
  - Solution: Changed to `blend: None` in tonemap pipeline ([src/gpu/pipelines.rs:257](src/gpu/pipelines.rs#L257))
  - Shader already handles all color mixing internally, GPU blending was unnecessary
  - **Side effect: Performance improvement** on all platforms due to eliminated blend operations

- **Fixed "Encoder is invalid" WebGPU error in WASM**
  - Root cause: Accumulation textures lacked `RENDER_ATTACHMENT` usage, preventing render pass clearing
  - Solution: Platform-specific texture usage flags ([src/gpu/buffers.rs:212-216](src/gpu/buffers.rs#L212-L216))
    - Desktop: `STORAGE_BINDING | TEXTURE_BINDING | COPY_DST | COPY_SRC`
    - WASM: Added `RENDER_ATTACHMENT` for render pass clearing
  - Created `clear_texture_wasm()` using render passes instead of `clear_texture()` feature
  - Desktop builds unaffected (all changes use `#[cfg(target_arch = "wasm32")]`)

- **Fixed camera rotation not loading from presets**
  - Changed hardcoded `0.0` to `config.camera_rotation_x/y` ([src/renderer/compute_kernel.rs:259-260](src/renderer/compute_kernel.rs#L259-L260))
  - Camera rotation now properly loads when switching presets

**Platform-Specific Implementation:**
- Desktop: Uses `encoder.clear_texture()` with `CLEAR_TEXTURE` feature (unchanged)
- WASM: Uses render pass with `LoadOp::Clear` for compatibility
- Temp samples texture clearing skipped in WASM (compute shader overwrites all pixels anyway)
- All changes isolated with conditional compilation - zero impact on desktop builds

**Files modified:**
- [src/gpu/pipelines.rs](src/gpu/pipelines.rs) - Disabled alpha blending
- [src/gpu/buffers.rs](src/gpu/buffers.rs) - WASM texture clearing
- [src/gpu/device.rs](src/gpu/device.rs) - Explicit `CompositeAlphaMode::Opaque`
- [src/renderer/compute_kernel.rs](src/renderer/compute_kernel.rs) - Camera rotation from config

### Added - 2025-10-21

#### 3D Rendering System
Complete pseudo-3D rendering implementation inspired by Apophysis 7X:

- **Dual Shader Architecture**
  - `shaders/trajectory_3d.wgsl` - New 3D compute shader tracking vec3 throughout iteration
  - `shaders/trajectory.wgsl` - Existing 2D shader (vec2 only)
  - Runtime selection based on `flame.render_mode` (TwoD or ThreeD)
  - No performance difference between 2D and 3D modes

- **3D Variations (8 new variations)**
  - **Zcone** (16): Z = distance from origin in XY plane (creates cone shape)
  - **Flatten** (17): Compress Z toward zero (depth control)
  - **Hemisphere** (18): Project onto sphere surface (full 3D structure)
  - **PreRotateX** (19): Rotate around X-axis before variations
  - **PreRotateY** (20): Rotate around Y-axis before variations
  - **PostRotateX** (21): Rotate around X-axis after variations
  - **PostRotateY** (22): Rotate around Y-axis after variations
  - **ZScale** (23): Scale Z coordinate up or down
  - Z-only variations (Zcone, Flatten, ZScale) modify `result.z` directly to avoid affecting XY
  - Full 3D variations (rotations, Hemisphere) use standard `result += weight * variation(p)`

- **Camera System**
  - Camera Pitch (X-axis rotation): Up/down orbit around fractal
  - Camera Yaw (Y-axis rotation): Left/right orbit around fractal
  - UI sliders in Performance window for real-time camera control
  - Applied before projection in `world_to_pixel()` function

- **Projection Types**
  - **Orthographic**: Flat projection (no depth perspective)
  - **Perspective**: Depth-aware projection with configurable strength (1.0-10.0)
  - Formula: `screen_pos = p.xy / (1.0 + p.z * strength)`
  - UI toggle and slider in Performance window

- **3D Transform Enhancements**
  - Added `g` field to Transform struct (Z offset for 3D mode)
  - Affine applies to XY, Z gets offset: `p'.z = p.z + transform.g`
  - CPU reference returns point unchanged for 3D variations (CPU is 2D only)

- **3D Preset**
  - "3D Spiral Tower" preset demonstrating clear 3D structure
  - Uses Zcone, Spherical, PostRotateY, and Hemisphere variations
  - Different Z offsets per transform to create depth layers
  - Perspective projection with strength 3.0

- **Flower of Life Preset**
  - Sacred geometry pattern with 6-fold rotational symmetry
  - Central Spiral variation with 6 Linear transforms arranged hexagonally
  - Rainbow color scheme (hue-shifted per transform)
  - Created based on user request for Hindu sacred geometry

- **Backward Compatibility**
  - Custom deserializer accepts both 16 and 24-element variation arrays
  - Old preset files (16 variations) auto-padded with zeros for new variations
  - 2D shader updated to use 24-element variation array (ignores last 8)
  - `Transform.g` field defaults to 0.0 for old presets

- **UI Enhancements**
  - Render Mode selector (2D/3D) in Performance window
  - Projection Type selector with Perspective strength slider
  - Camera Pitch and Yaw sliders (-180° to 180°)
  - 3D Variations section in Transforms window (only visible in 3D mode)
  - Separated "2D Variations" and "3D Variations" UI sections
  - Z Offset slider in each transform (3D mode only)

### Changed - 2025-10-21

#### Variation System Expansion
- **MAX_VARIATIONS** increased from 16 to 24 (16 2D + 8 3D)
- **GpuTransform.variations** changed from `[f32; 16]` to `[f32; 24]`
- Both `trajectory.wgsl` and `trajectory_3d.wgsl` use 24-element variation arrays
- 2D shader only uses first 16 variations, ignoring the last 8

#### Transform Structure
- Added `g: f32` field for Z offset in 3D mode (defaults to 0.0)
- Added `render_mode: RenderMode` to Flame (TwoD or ThreeD)
- Added `projection: ProjectionType` to Flame (Orthographic or Perspective)
- Added `camera_pitch: f32` and `camera_yaw: f32` to Flame
- Manual `Default` implementation for Flame (needed custom default name)

#### GPU Parameters
- **GpuParams** expanded with 3D fields:
  - `camera_pitch: f32` - Camera X-axis rotation
  - `camera_yaw: f32` - Camera Y-axis rotation
  - `projection_type: u32` - 0=Ortho, 1=Perspective
  - `perspective_strength: f32` - Perspective intensity

#### Pipeline Management
- **FlamePipelines** now creates two trajectory compute pipelines:
  - `trajectory_pipeline` - 2D mode (trajectory.wgsl)
  - `trajectory_pipeline_3d` - 3D mode (trajectory_3d.wgsl)
- Runtime selection in `compute_pass()` based on `flame.render_mode`

### Fixed - 2025-10-21

#### 3D Variation Application
- **Fixed Z-only variations affecting XY coordinates**
  - Root cause: Using `result += weight * variation(p)` added p.x and p.y to result
  - Solution: Z-only variations (Zcone, Flatten, ZScale) now modify `result.z` directly
  - Example: `result.z *= (1.0 - xform.variations[17] * 0.5)` for Flatten

#### Backward Compatibility
- **Fixed old preset loading errors**
  - Root cause: Old presets had 16-element variation arrays, code expected 24
  - Solution: Custom `Deserialize` implementation for Transform
  - Visitor pattern accepts both 16 and 24-element arrays
  - 16-element arrays auto-padded with 8 zeros
  - Also fixed missing `g` field with `#[serde(default)]`

#### UI and Rendering
- **Fixed missing 3D variation controls in UI**
  - Added "3D Variations" section (only visible in 3D mode)
  - Split variation list into 2D (0-15) and 3D (16-23) sections
- **Fixed struct layout mismatch**
  - Updated 2D shader to use 24-element variation array
  - Ensures GPU struct matches CPU layout exactly

### Implementation Details - 2025-10-21

#### 3D Rendering System Files
- **New files:**
  - `shaders/trajectory_3d.wgsl` - 3D compute shader
  - `src/scene/presets.rs::create_3d_flame()` - 3D preset creation

- **Modified files:**
  - `src/scene/transforms.rs` - RenderMode, ProjectionType, Transform.g, 8 new VariationType enums, custom Deserialize
  - `src/gpu/buffers.rs` - GpuTransform.g, GpuTransform.variations[24], GpuParams 3D fields
  - `src/gpu/pipelines.rs` - trajectory_pipeline_3d creation, runtime selection
  - `src/renderer/compute_kernel.rs` - Pipeline selection logic, 3D param updates
  - `src/ui/mod.rs` - Render mode selector, projection controls, camera sliders, 3D variation UI, Z offset slider
  - `src/app.rs` - Default transform with 24 variations, 3D field handlers
  - `shaders/trajectory.wgsl` - Updated to 24-element variation array

- **Variation implementation pattern:**
  - **2D variations (0-15):** Implemented in both trajectory.wgsl and trajectory_3d.wgsl (pass Z through)
  - **3D variations (16-23):** Only implemented in trajectory_3d.wgsl
  - **CPU reference:** All 3D variations return `p` unchanged (CPU is 2D only)

### Added - 2025-10-20

#### Transform Add/Delete Functionality
- **"➕ Add Transform" Button**
  - Located at top of Transforms window
  - Creates new transform with sensible defaults:
    - 0.5 scale affine matrix (a=0.5, d=0.5, others=0)
    - Weight 1.0
    - 0.5 Linear variation, all others 0
    - Gray color (0.5, 0.5, 0.5)
    - Color speed 0.5
  - Full undo support via `capture_state()`
  - Automatic renderer update and accumulation reset

- **"🗑 Delete Transform" Button**
  - Located inside each transform's collapsible header
  - Only visible when more than 1 transform exists (prevents deleting last transform)
  - Full undo support via `capture_state()`
  - Automatic renderer update and accumulation reset
  - Bounds checking to prevent out-of-range deletions

- **UI Improvements**
  - Transform count displayed in window header: "Transforms (N)"
  - Transform count also shown next to Add button
  - Cached `num_transforms` variable to avoid borrow conflicts during iteration

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

#### Transform Add/Delete
- Files modified:
  - `src/ui/mod.rs`:
    - Added `add_transform: bool` and `delete_transform: Option<usize>` to UiResponse
    - Added "➕ Add Transform" button with `flame_changed = true`
    - Added "🗑 Delete Transform" button with `flame_changed = true`
    - Cached `num_transforms` to avoid borrow conflicts
  - `src/app.rs`:
    - Handler for `ui_response.add_transform` - creates default Transform, pushes to flame
    - Handler for `ui_response.delete_transform` - removes transform by index
    - Both handlers call `capture_state()` for undo support
    - Both trigger `ui_response.flame_changed` which updates renderer

#### Preset and Palette Systems
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
