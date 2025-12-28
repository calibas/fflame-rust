# Tiled Interactive Rendering

## Goal
Use TiledRenderer in the interactive editor for:
1. Large screen sizes (4K+) that exceed single-buffer limits
2. Supersampling (2×, 4×) for anti-aliased display

## Current State
- `FlameRenderer` handles interactive rendering with single histogram buffer
- `TiledRenderer` handles batch export with tiled histogram buffers
- Both duplicate significant GPU setup code

## Proposed Architecture

### Option A: Unified Renderer (Recommended)
Create a single renderer that automatically tiles when needed:

```rust
pub struct UnifiedFlameRenderer {
    // Common resources
    device: Device,
    queue: Queue,

    // Rendering mode
    mode: RenderingMode,

    // Shared components
    pipelines: FlamePipelines,
    // ...
}

enum RenderingMode {
    SingleBuffer {
        buffers: FlameBuffers,
        // Single-tile resources
    },
    Tiled {
        tile_buffers: Vec<TileBuffers>,
        tile_grid: (u32, u32, u32), // tiles_x, tiles_y, tile_size
        // Multi-tile resources
    },
}
```

### Option B: Wrapper (Simpler, Less Code Sharing)
Keep both renderers, add a wrapper that chooses:

```rust
pub enum FlameRendererBackend {
    Standard(FlameRenderer),
    Tiled(TiledRenderer),
}

impl FlameRendererBackend {
    pub fn new(device, queue, width, height, flame, supersample: SupersampleLevel) -> Self {
        let render_width = width * supersample.multiplier();
        let render_height = height * supersample.multiplier();

        if needs_tiling(render_width, render_height) {
            Self::Tiled(TiledRenderer::new(...))
        } else {
            Self::Standard(FlameRenderer::new(...))
        }
    }
}
```

## Key Challenges

### 1. Interactive Tiled Rendering
Current TiledRenderer runs all iterations at once. For interactive use:
- Run N iterations per frame (like FlameRenderer)
- Accumulate progressively in per-tile buffers
- Stitch tiles for display each frame

### 2. Tile Stitching Performance
Each frame, need to combine tiles into final display texture:
- Option A: GPU compute shader (fast)
- Option B: Copy commands (moderate)
- Option C: Render each tile to final texture region (simple)

### 3. Supersampling Downscale
When supersampling:
- Render at 2× or 4× resolution (internal tiles)
- Downsample to display resolution before showing
- Box filter (average NxN pixels) is sufficient

## Implementation Plan

### Phase 1: Make TiledRenderer Progressive
1. Add `compute_pass_frame()` method for per-frame iterations
2. Add `accumulate_pass_frame()` for progressive blending
3. Add `get_display_texture()` for current frame output

### Phase 2: Add Tile Stitching
1. Create stitched output texture at full render resolution
2. Add GPU pass to combine tiles into single texture
3. Add downsampling pass for supersampling

### Phase 3: Integrate with App
1. Add SupersampleLevel to FractalConfig/SystemSettings
2. Add UI controls in Settings panel
3. Switch renderer based on resolution/supersampling

## Memory Analysis

| Resolution | Histogram Size | Needs Tiling? |
|------------|---------------|---------------|
| 1920×1080  | 33 MB         | No            |
| 2560×1440  | 59 MB         | No            |
| 3840×2160  | 133 MB        | Maybe*        |
| 1920×1080 @2× | 133 MB     | Maybe*        |
| 1920×1080 @4× | 531 MB     | Yes           |
| 3840×2160 @2× | 531 MB     | Yes           |

*Depends on adapter's max_storage_buffer_binding_size (typically 256MB-2GB on modern GPUs)

## Implementation Status

### Completed (2025-11-27)

#### Phase 1: Progressive TiledRenderer ✅
- Added `init_interactive()` - initializes for per-frame rendering
- Added `reset_interactive()` - clears histogram and iteration counters
- Added `compute_pass_frame()` - runs N iterations per frame
- Added iteration tracking (`samples_accumulated`, `total_iterations`)

#### Phase 2: Tile Stitching ✅
- Added `accumulate_and_stitch()` - processes all tiles and stitches to display texture
- Created `stitched_texture` at full render resolution
- Processes each tile: accumulate → tonemap → copy to stitched texture
- Added `clear_accumulation()` for reset

#### Phase 2.5: Supersampling Downsample ✅
- Created `downsample.wgsl` shader (box filter averaging)
- Added `init_supersampling()` - sets up downsample pipeline
- Added `downsample()` - reduces stitched texture to display size
- Added `get_final_display_view()` - returns appropriate texture for display

#### Phase 3: App Integration ✅
1. Added SupersampleLevel to SystemSettings (device-specific setting)
2. Added UI controls in Settings panel (dropdown: Off/2×/4×)
3. Added `new_with_device()` constructor for shared GPU context
4. Wired up App render loop to use TiledRenderer when supersampling enabled
5. Registered TiledRenderer's texture with egui for display

### Remaining Work

- Test with 2× and 4× supersampling in practice
- Handle viewport resize when supersampling is active
- Add WASM support (currently desktop only)

### API Summary

```rust
// Construction
let mut renderer = TiledRenderer::new(&config, render_width, render_height).await?;

// Optional: Enable supersampling
renderer.init_supersampling(SupersampleLevel::X2, display_width, display_height);

// Per-frame render loop:
renderer.init_interactive(&config);

// Each frame:
renderer.compute_pass_frame(&config, workgroups, iterations_per_thread, clear);
renderer.accumulate_and_stitch(&config, blend_factor);
renderer.downsample();  // Only runs if supersampling enabled

// Get texture for display
let view = renderer.get_final_display_view();
```

## Next Steps
1. Add SupersampleLevel to SystemSettings
2. Add UI controls in Settings → Performance section
3. Wire up App render loop to use TiledRenderer when needed
4. Test with 2× and 4× supersampling
