# Optimized Tiled Rendering

**Status:** Planning
**Priority:** High
**Created:** 2025-11-27
**Depends on:** high-res-export.md (current implementation)

## Problem

The current tiled export implementation re-renders the entire fractal for each tile. For a 4-tile export, we do 4× the iteration work. This is inefficient and doesn't scale.

**Current approach:**
```
For each tile:
    Create renderer at tile viewport
    Run N iterations (samples go to this tile's histogram)
    Read back tile pixels
    Stitch into final image
```

**Problem:** Iteration work is repeated for every tile. A 10000×10000 export with 4×4 tiles runs 16× more iterations than necessary.

## Solution

Single-pass tiled rendering: generate samples once, distribute to appropriate tile buffers based on screen coordinates.

**Optimized approach:**
```
Create histogram buffers for all tiles
Run N iterations once:
    For each sample:
        Calculate full-resolution pixel coordinates
        Determine which tile it belongs to
        Write to that tile's histogram buffer
For each tile:
    Run accumulate + tonemap passes
    Read back tile pixels
Stitch into final image
```

## Architecture

### Tile Histogram Array

Instead of one histogram buffer, create an array of tile histogram buffers:

```rust
// Rust side
struct TiledRenderer {
    tile_histograms: Vec<wgpu::Buffer>,  // One per tile
    tiles_x: u32,
    tiles_y: u32,
    tile_size: u32,
    full_width: u32,
    full_height: u32,
}
```

### Shader Changes

The compute shader needs to:
1. Calculate pixel coordinates in full-resolution space
2. Determine which tile the pixel belongs to
3. Write to the correct tile's histogram buffer

```wgsl
// Uniforms for tiled rendering
struct TileParams {
    full_width: u32,
    full_height: u32,
    tile_size: u32,
    tiles_x: u32,
    tiles_y: u32,
    _padding: vec3<u32>,
}

@group(0) @binding(X) var<uniform> tile_params: TileParams;

// Option A: Array of storage buffers (if supported)
@group(1) @binding(0) var<storage, read_write> tile_histograms: array<array<atomic<u32>, TILE_PIXELS * 4>, MAX_TILES>;

// Option B: Single large buffer with offsets
@group(1) @binding(0) var<storage, read_write> all_histograms: array<atomic<u32>>;

fn write_to_tile(world_point: vec3<f32>, color_idx: f32) {
    // Calculate full-resolution pixel coordinates
    let full_pixel = world_to_pixel(world_point, tile_params.full_width, tile_params.full_height);

    // Check bounds
    if (full_pixel.x < 0.0 || full_pixel.x >= f32(tile_params.full_width) ||
        full_pixel.y < 0.0 || full_pixel.y >= f32(tile_params.full_height)) {
        return;  // Out of bounds
    }

    let px = u32(full_pixel.x);
    let py = u32(full_pixel.y);

    // Determine tile
    let tile_x = px / tile_params.tile_size;
    let tile_y = py / tile_params.tile_size;
    let tile_idx = tile_y * tile_params.tiles_x + tile_x;

    // Local coordinates within tile
    let local_x = px % tile_params.tile_size;
    let local_y = py % tile_params.tile_size;
    let local_idx = local_y * tile_params.tile_size + local_x;

    // Write to tile histogram (Option B: single buffer)
    let tile_offset = tile_idx * tile_params.tile_size * tile_params.tile_size * 4u;
    let pixel_offset = tile_offset + local_idx * 4u;

    // Atomic writes for R, G, B, density
    atomicAdd(&all_histograms[pixel_offset + 0u], r_scaled);
    atomicAdd(&all_histograms[pixel_offset + 1u], g_scaled);
    atomicAdd(&all_histograms[pixel_offset + 2u], b_scaled);
    atomicAdd(&all_histograms[pixel_offset + 3u], density_scaled);
}
```

### Buffer Layout Options

**Option A: Binding Array (Preferred)**
- Each tile has its own storage buffer
- Use WGSL binding arrays: `@group(1) @binding(0) var<storage> tiles: binding_array<TileBuffer>`
- Requires `BUFFER_BINDING_ARRAY` feature
- Clean separation, easy to manage memory per-tile

**Option B: Single Large Buffer with Offsets**
- All tiles in one contiguous buffer
- Calculate offset: `tile_idx * tile_pixels * channels`
- Simpler binding, but must fit in one buffer
- For 16 tiles at 2000×2000×16 bytes = 1GB total

**Option C: Chunked Processing**
- If too many tiles to bind at once, process in chunks
- E.g., for 16 tiles, process 4 at a time
- Iterations run once per chunk (still better than current)

**Recommendation:** Start with Option B (single buffer) for simplicity. Fall back to Option C if buffer size exceeds limits.

## Memory Analysis

Per-tile histogram buffer:
- 2000×2000 pixels × 4 channels × 4 bytes = **64 MB**

For different tile counts:
| Tiles | Total Histogram Memory |
|-------|------------------------|
| 2×2   | 256 MB |
| 3×3   | 576 MB |
| 4×4   | 1024 MB |
| 5×5   | 1600 MB |

**GPU buffer limits:**
- Most GPUs: 256 MB per buffer (some allow up to 2GB)
- For Option B, 4×4 tiles may exceed single buffer limit
- Solution: Use Option C chunked processing for large tile counts

## Implementation Plan

### Phase 1: Core Infrastructure
- [ ] Add `TileParams` uniform buffer
- [ ] Create single large histogram buffer for all tiles
- [ ] Modify compute shader to calculate tile index and write to correct offset
- [ ] Add `world_to_pixel_full_res()` function that uses full output dimensions

### Phase 2: Tiled Compute Pass
- [ ] Create `TiledFlameRenderer` or extend `FlameRenderer`
- [ ] Implement `compute_pass_tiled()` that writes to multi-tile buffer
- [ ] Handle edge tiles (may be smaller than tile_size)

### Phase 3: Per-Tile Post-Processing
- [ ] Run accumulate pass per-tile (or modify for tiled operation)
- [ ] Run tonemap pass per-tile
- [ ] Read back each tile and stitch

### Phase 4: Chunked Processing (if needed)
- [ ] Detect when total histogram size exceeds buffer limits
- [ ] Split tiles into chunks that fit
- [ ] Run iterations once per chunk
- [ ] Still N× better than current (where N = tiles / chunks)

### Phase 5: Integration
- [ ] Replace current `render_tiled()` with optimized version
- [ ] Update progress reporting (iterations once, then tile post-processing)
- [ ] Benchmark vs current implementation

## Performance Comparison

For 8000×8000 export (4×4 = 16 tiles) with 100M iterations:

**Current implementation:**
- 16 tiles × 100M iterations = 1.6B total iterations
- Time: ~16× single render

**Optimized implementation:**
- 100M iterations (once) + 16× post-processing
- Post-processing is fast (accumulate + tonemap)
- Time: ~1.1× single render

**Speedup:** ~14× faster for 4×4 tiling

## Edge Cases

1. **Samples on tile boundaries**: Each sample belongs to exactly one tile based on integer pixel coordinates. No special handling needed.

2. **Edge tiles smaller than tile_size**: The rightmost column and bottom row of tiles may be smaller. Handle in local coordinate calculation.

3. **Very large exports (many tiles)**: Use chunked processing. Each chunk processes a subset of tiles, iterations run once per chunk.

4. **Aspect ratio mismatch**: Tiles may not divide evenly. Handled by variable edge tile sizes.

## Success Criteria

- [ ] 8000×8000 export runs in ~same time as 2000×2000
- [ ] Memory usage stays within GPU limits
- [ ] No visual artifacts at tile boundaries
- [ ] Correct output matches current (re-render) implementation

## References

- Current implementation: [high-res-export.md](high-res-export.md)
- WGSL binding arrays: https://www.w3.org/TR/WGSL/#binding-array
- wgpu buffer limits: https://docs.rs/wgpu/latest/wgpu/struct.Limits.html
