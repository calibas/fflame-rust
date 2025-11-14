# Tiled High-Resolution Export

**Status:** Planning
**Branch:** `feature/tiled-export`
**Priority:** High (needed for 4K+ exports and supersampling)
**Created:** 2025-11-13

## Problem

Current implementation has GPU buffer size limits:
- **Histogram buffer:** `width × height × 16 bytes` (4 channels × u32)
- **Accumulation textures:** `width × height × 8 bytes` (Rgba16Float)
- **Example:** 5000×5000 = 400MB histogram buffer (exceeds typical 256MB limit)
- **User reported:** Cannot export at 5000×5000 resolution

## Solution: Tiled Rendering

Render large images in tiles, each fitting within GPU limits:

```
Final Image (5000×5000)
┌─────────┬─────────┬─────────┐
│ Tile 0  │ Tile 1  │ Tile 2  │  Each tile: 2048×2048
│ (2048²) │ (2048²) │ (904²)  │  Histogram: 64MB
├─────────┼─────────┼─────────┤
│ Tile 3  │ Tile 4  │ Tile 5  │  Well under 256MB limit
│ (2048²) │ (2048²) │ (904²)  │
├─────────┼─────────┼─────────┤
│ Tile 6  │ Tile 7  │ Tile 8  │
│ (904²)  │ (904²)  │ (904²)  │
└─────────┴─────────┴─────────┘
```

**Key Insight:** Fractal flames are resolution-independent - we can render at any resolution by adjusting the viewport transform.

## Architecture

### Tile Coordinate Transform

For each tile `(tx, ty)` in grid:

```rust
// Tile viewport in world space
let tile_world_width = world_width / tiles_x;
let tile_world_height = world_height / tiles_y;

// Tile center offset from image center
let tile_offset_x = (tx - tiles_x/2 + 0.5) * tile_world_width;
let tile_offset_y = (ty - tiles_y/2 + 0.5) * tile_world_height;

// Adjust pan for this tile
let tile_pan_x = config.pan_x + tile_offset_x;
let tile_pan_y = config.pan_y + tile_offset_y;

// Zoom stays the same (world_width is constant across all tiles)
```

### Rendering Process

```rust
pub async fn export_tiled_png(
    config: &FractalConfig,
    output_width: u32,
    output_height: u32,
    tile_size: u32, // Max tile dimension (e.g., 2048)
) -> Result<Vec<u8>, String> {
    // 1. Calculate tile grid
    let tiles_x = (output_width + tile_size - 1) / tile_size;
    let tiles_y = (output_height + tile_size - 1) / tile_size;

    // 2. Allocate final image buffer
    let mut final_image = vec![0u8; (output_width * output_height * 4) as usize];

    // 3. Render each tile
    for ty in 0..tiles_y {
        for tx in 0..tiles_x {
            // Calculate tile dimensions (last tiles may be smaller)
            let tile_w = (output_width - tx * tile_size).min(tile_size);
            let tile_h = (output_height - ty * tile_size).min(tile_size);

            // Calculate tile viewport transform
            let (tile_pan_x, tile_pan_y) = calculate_tile_offset(
                config, tx, ty, tiles_x, tiles_y, tile_w, tile_h,
            );

            // Render tile at tile_w × tile_h
            let tile_config = config.clone_with_pan(tile_pan_x, tile_pan_y);
            let tile_pixels = render_tile(
                &device, &queue, tile_w, tile_h, &tile_config
            ).await?;

            // Copy tile into final image
            blit_tile(&mut final_image, output_width, &tile_pixels,
                      tx * tile_size, ty * tile_size, tile_w, tile_h);

            // Progress feedback
            println!("Rendered tile {}/{}", ty * tiles_x + tx + 1, tiles_x * tiles_y);
        }
    }

    // 4. Encode final image as PNG
    encode_png_from_rgba(output_width, output_height, final_image, metadata)
}
```

### Key Functions

**1. Tile Offset Calculation**

```rust
fn calculate_tile_offset(
    config: &FractalConfig,
    tile_x: u32,
    tile_y: u32,
    tiles_x: u32,
    tiles_y: u32,
    tile_width: u32,
    tile_height: u32,
) -> (f64, f64) {
    // World space dimensions (what the full image sees)
    let aspect = config.output_width as f64 / config.output_height as f64;
    let world_height = 4.0 / config.zoom;
    let world_width = world_height * aspect;

    // Tile world dimensions
    let tile_world_width = world_width / tiles_x as f64;
    let tile_world_height = world_height / tiles_y as f64;

    // Tile center offset from full image center
    let center_offset_x = (tile_x as f64 - (tiles_x as f64 / 2.0) + 0.5) * tile_world_width;
    let center_offset_y = (tile_y as f64 - (tiles_y as f64 / 2.0) + 0.5) * tile_world_height;

    // Apply rotation if needed
    let (offset_x, offset_y) = if config.rotation.abs() > 0.001 {
        let cos = config.rotation.cos();
        let sin = config.rotation.sin();
        (
            center_offset_x * cos - center_offset_y * sin,
            center_offset_x * sin + center_offset_y * cos,
        )
    } else {
        (center_offset_x, center_offset_y)
    };

    (config.pan_x + offset_x, config.pan_y + offset_y)
}
```

**2. Tile Rendering**

```rust
async fn render_tile(
    device: &Device,
    queue: &Queue,
    width: u32,
    height: u32,
    tile_config: &FractalConfig,
) -> Result<Vec<u8>, String> {
    // Create renderer at tile size
    let surface_format = TextureFormat::Rgba8Unorm;
    let mut renderer = FlameRenderer::new(device, queue, surface_format, width, height, &tile_config.flame);

    // Load config
    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
        label: Some("Tile Render Encoder"),
    });

    let palette = /* get palette */;
    renderer.load_config(device, &mut encoder, queue, tile_config, palette, tile_config.iterations_per_thread);
    queue.submit(std::iter::once(encoder.finish()));

    // Render iterations
    let mut total = 0u64;
    while total < tile_config.max_iterations {
        let mut enc = device.create_command_encoder(&CommandEncoderDescriptor::default());

        renderer.compute_pass(&mut enc, queue, 128, tile_config.iterations_per_thread,
            tile_config.zoom, tile_config.pan_x, tile_config.pan_y, tile_config.rotation,
            tile_config.camera_rotation_x, tile_config.camera_rotation_y, tile_config.camera_z,
            tile_config.speed_factor, false);

        let samples = 128 * 64 * tile_config.iterations_per_thread as u64;
        renderer.accumulate_pass(&mut enc, queue, device, samples);

        total += samples;
        queue.submit(std::iter::once(enc.finish()));

        if total >= tile_config.max_iterations { break; }
    }

    // Tonemap to fractal_texture
    let mut tonemap_enc = device.create_command_encoder(&CommandEncoderDescriptor::default());
    renderer.tonemap_pass(&mut tonemap_enc);
    queue.submit(std::iter::once(tonemap_enc.finish()));

    // Read pixels
    let (_w, _h, pixels) = renderer.read_fractal_pixels(device, queue, false, tile_config.background_color).await?;
    Ok(pixels)
}
```

**3. Tile Blitting**

```rust
fn blit_tile(
    dst: &mut [u8],
    dst_width: u32,
    src: &[u8],
    dst_x: u32,
    dst_y: u32,
    tile_width: u32,
    tile_height: u32,
) {
    for y in 0..tile_height {
        let src_row_start = (y * tile_width * 4) as usize;
        let src_row_end = src_row_start + (tile_width * 4) as usize;
        let src_row = &src[src_row_start..src_row_end];

        let dst_y_pos = dst_y + y;
        let dst_row_start = ((dst_y_pos * dst_width + dst_x) * 4) as usize;
        let dst_row_end = dst_row_start + (tile_width * 4) as usize;

        dst[dst_row_start..dst_row_end].copy_from_slice(src_row);
    }
}
```

## Implementation Plan

### Phase 1: Core Tiled Export (CLI only)
- [ ] Add `export_tiled_png()` function in new file `src/app/tiled_export.rs`
- [ ] Implement tile offset calculation with rotation support
- [ ] Implement tile rendering loop
- [ ] Implement tile blitting into final image
- [ ] Add CLI flags: `--width 5000 --height 5000 --tile-size 2048`
- [ ] Test at various resolutions (2K, 4K, 8K, 10K)

### Phase 2: Tile Size Auto-Detection
- [ ] Query GPU limits: `device.limits().max_buffer_size`
- [ ] Calculate optimal tile size based on limits
- [ ] Add safety margin (e.g., 75% of max buffer size)
- [ ] Default to auto-detection, allow override with `--tile-size`

### Phase 3: Progress Reporting
- [ ] Add progress callback for tile rendering
- [ ] Show tile grid (e.g., "Rendering tile 5/9 (2048x2048)")
- [ ] Show per-tile iteration progress
- [ ] Show overall progress percentage

### Phase 4: App UI Integration
- [ ] Add "Export High-Res PNG" dialog
- [ ] Resolution presets (1080p, 4K, 8K, Custom)
- [ ] Show estimated memory usage
- [ ] Show tile grid visualization
- [ ] Async export with progress bar

### Phase 5: Supersampling Support
- [ ] Render at higher resolution (e.g., 2x)
- [ ] Downsample tiles with averaging
- [ ] Quality presets (1x, 2x, 4x supersampling)

## Configuration

### CLI Interface

```bash
# Export at 5000×5000 with auto tile size
fractal_flame_wgpu export -i config.fflame -o huge.png --width 5000 --height 5000

# Export with explicit tile size
fractal_flame_wgpu export -i config.fflame -o huge.png --width 8000 --height 8000 --tile-size 1024

# Export with 2x supersampling
fractal_flame_wgpu export -i config.fflame -o huge.png --width 4000 --height 4000 --supersample 2
```

### FractalConfig Extension

```rust
pub struct ExportConfig {
    pub output_width: u32,
    pub output_height: u32,
    pub tile_size: Option<u32>, // None = auto-detect
    pub supersample: u32, // 1 = no AA, 2 = 2×2, 4 = 4×4
}
```

## Performance Considerations

### Memory Usage

| Resolution | Tiles (2048²) | Peak Memory (per tile) | Total Time (est) |
|-----------|---------------|------------------------|------------------|
| 2048×2048 | 1×1          | 64MB                   | 1× baseline     |
| 4096×4096 | 2×2          | 64MB                   | 4× baseline     |
| 5000×5000 | 3×3          | 64MB                   | ~6× baseline    |
| 8192×8192 | 4×4          | 64MB                   | 16× baseline    |
| 10000×10000 | 5×5        | 64MB                   | ~25× baseline   |

**Key:** Memory usage stays constant, time scales with total pixel count.

### Iteration Distribution

**Challenge:** Each tile needs same iteration density for consistent quality.

**Solution:** Use same `max_iterations` and `iterations_per_thread` for all tiles.

### Seam Artifacts

**Potential Issue:** Visible seams between tiles due to RNG differences.

**Solution:** Fractal flames use random sampling - no RNG synchronization needed. Each tile independently samples the same mathematical fractal, so seams should be invisible.

**Verification:** Visual inspection at tile boundaries after Phase 1 implementation.

## Testing Plan

1. **Tile Offset Math:** Unit tests for various grid sizes, rotations
2. **Single Tile:** Verify 2048×2048 render matches viewport render
3. **2×2 Grid:** Verify 4096×4096 matches enlarged viewport
4. **Edge Cases:** Non-square images, odd dimensions, rotation
5. **Large Export:** 10000×10000 export, check memory stays < 500MB
6. **Quality Check:** Compare tiled vs viewport at same resolution

## Future Enhancements

- **Parallel Tile Rendering:** Render multiple tiles concurrently (multi-GPU)
- **Disk Caching:** Save tiles to disk for very large exports (100K×100K)
- **Adaptive Tile Size:** Smaller tiles in complex areas, larger in simple areas
- **Resume Support:** Save progress, resume interrupted exports

## Success Criteria

- ✅ Export 5000×5000 PNG without GPU buffer errors
- ✅ Export 10000×10000 PNG in reasonable time (<10min @ 1B iterations)
- ✅ Memory usage stays under GPU limits regardless of output resolution
- ✅ No visible seams between tiles
- ✅ Pixel-perfect match with viewport render (at same resolution)
- ✅ CLI and UI support

## Integration with Supersampling

**See:** [supersampling-antialiasing.md](supersampling-antialiasing.md)

### The Perfect Combination

Tiled rendering + supersampling solves BOTH problems:

1. **High Resolution:** Render at any size via tiling (5K, 8K, 10K+)
2. **Anti-Aliasing:** Apply supersampling to each tile independently
3. **Constant Memory:** Never exceeds GPU limits regardless of output size or SS factor

### Architecture

```
For each tile:
  1. Render at tile_size × supersample_factor
     Example: 2048 × 2 = 4096×4096 internal resolution
  2. Downsample tile to tile_size (reuse supersampling downsample shader)
     4096×4096 → 2048×2048 with anti-aliasing
  3. Blit downsampled tile to final image
```

### Memory Analysis

**Without Tiling:**
- 5000×5000 @ 2× SS = 10000×10000 = **1.6GB** histogram buffer ❌ FAILS

**With Tiling:**
- Tile size: 2048×2048
- With 2× SS: 4096×4096 internal = **256MB** histogram buffer ✅ SAFE
- Memory stays constant regardless of output size

### Implementation Strategy

**Order of Implementation:**

1. **Phase 1:** Basic tiled export (no supersampling)
   - Solves high-resolution export immediately
   - ~200-300 lines of code
   - Enables 5000×5000+ exports

2. **Later:** Add supersampling to full viewport
   - Implement downsample pass (per supersampling plan)
   - Works for viewport-sized exports

3. **Final:** Integrate supersampling into tiled export
   - Reuse downsample shader for each tile
   - Minimal code (~50 lines)
   - Unlocks unlimited resolution + quality

### Example Configuration

```bash
# 8K export with 2× supersampling (publication quality)
fractal_flame_wgpu export -i config.fflame -o output.png \
  --width 7680 --height 4320 \    # 8K resolution
  --supersample 2 \                # 2× anti-aliasing
  --tile-size 2048                 # Auto-detected based on GPU limits

# Tiles: 4×3 grid (12 tiles)
# Each tile renders at 4096×4096 internally (256MB)
# Downsampled to 2048×2048 before blitting
# Total time: ~12× baseline (12 tiles × no SS overhead per tile)
```

### Performance Impact

| Resolution | SS Factor | Tiles (2048²) | Render Time (relative) |
|-----------|-----------|---------------|------------------------|
| 4096×4096 | 1× | 2×2 (4 tiles) | 4× baseline |
| 4096×4096 | 2× | 2×2 (4 tiles) | 16× baseline (4 tiles × 4× SS) |
| 8192×8192 | 1× | 4×4 (16 tiles) | 16× baseline |
| 8192×8192 | 2× | 4×4 (16 tiles) | 64× baseline (16 tiles × 4× SS) |

### Recommended Tile Size

Based on GPU limits and supersampling:

```rust
fn calculate_safe_tile_size(max_buffer_size: u64, supersample_factor: u32) -> u32 {
    // Histogram buffer is the limiting factor: width × height × 16 bytes
    // Target 75% of max for safety margin
    let safe_buffer_size = (max_buffer_size as f64 * 0.75) as u64;

    // Solve: (tile_size × SS)² × 16 = safe_buffer_size
    let max_pixels = safe_buffer_size / 16;
    let tile_size_with_ss = (max_pixels as f64).sqrt() as u32;
    let tile_size = tile_size_with_ss / supersample_factor;

    // Round down to nearest power of 2 for clean tiling
    tile_size.next_power_of_two() / 2
}

// Example with 256MB limit:
// 1× SS: 3072×3072 tiles (144MB each)
// 2× SS: 1536×1536 tiles (144MB each @ 3072² internal)
// 4× SS: 768×768 tiles (144MB each @ 3072² internal)
```

## References

- [supersampling-antialiasing.md](supersampling-antialiasing.md) - Supersampling design
- [docs/STATUS.md](../STATUS.md) - Feature priority
- [CLAUDE.md](../../CLAUDE.md) - High-priority features
- Similar implementations: Apophysis 7X, Electric Sheep renderer
