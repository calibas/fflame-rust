# High-Resolution Export with Supersampling

**Status:** Planning
**Priority:** High
**Created:** 2025-11-27

## Overview

Enable export at any resolution with optional supersampling for anti-aliasing. Tiling is handled automatically when the render size exceeds GPU limits - no user configuration needed.

## User-Facing Features

### UI Controls (Export Dialog)
- **Resolution:** Width × Height (any size)
- **Supersampling:** Off (1×), 2× (4 pixels), 4× (16 pixels)

That's it. No tiling options, no tile size configuration.

### What Happens Internally

```
User requests: 1920×1080 with 2× supersampling
→ Internal render size: 3840×2160
→ If 3840×2160 > GPU limit: automatically tile
→ Stitch tiles together
→ Downsample to 1920×1080
→ Save PNG
```

## Architecture

### Resolution Calculation

```rust
fn calculate_render_size(output_width: u32, output_height: u32, supersample: u32) -> (u32, u32) {
    (output_width * supersample, output_height * supersample)
}
```

### Auto-Tiling Decision

```rust
const MAX_TILE_PIXELS: u64 = 4_000_000; // ~2000×2000, safe for most GPUs

fn needs_tiling(render_width: u32, render_height: u32) -> bool {
    (render_width as u64 * render_height as u64) > MAX_TILE_PIXELS
}

fn calculate_tile_grid(render_width: u32, render_height: u32) -> (u32, u32, u32) {
    // Find tile size that keeps each tile under the limit
    let tile_size = (MAX_TILE_PIXELS as f64).sqrt() as u32;
    let tiles_x = (render_width + tile_size - 1) / tile_size;
    let tiles_y = (render_height + tile_size - 1) / tile_size;
    (tiles_x, tiles_y, tile_size)
}
```

### Export Pipeline

```rust
pub async fn export_high_res(
    config: &FractalConfig,
    output_width: u32,
    output_height: u32,
    supersample: u32,  // 1, 2, or 4
    output_path: &Path,
) -> Result<(), ExportError> {
    let render_width = output_width * supersample;
    let render_height = output_height * supersample;

    // Render at full internal resolution (tiling if needed)
    let full_image = if needs_tiling(render_width, render_height) {
        render_tiled(config, render_width, render_height).await?
    } else {
        render_single(config, render_width, render_height).await?
    };

    // Downsample if supersampling enabled
    let final_image = if supersample > 1 {
        downsample(&full_image, render_width, render_height, output_width, output_height)
    } else {
        full_image
    };

    // Save
    save_png(output_path, output_width, output_height, &final_image)?;
    Ok(())
}
```

## Tiled Rendering

When render size exceeds GPU limits, automatically split into tiles.

### Tile Coordinate Transform

Each tile renders a portion of the fractal by adjusting pan coordinates:

```rust
fn calculate_tile_viewport(
    config: &FractalConfig,
    tile_x: u32, tile_y: u32,
    tiles_x: u32, tiles_y: u32,
    render_width: u32, render_height: u32,
) -> (f64, f64) {
    // World space dimensions
    let aspect = render_width as f64 / render_height as f64;
    let world_height = 4.0 / config.zoom;
    let world_width = world_height * aspect;

    // Tile world dimensions
    let tile_world_width = world_width / tiles_x as f64;
    let tile_world_height = world_height / tiles_y as f64;

    // Offset from center
    let offset_x = (tile_x as f64 - tiles_x as f64 / 2.0 + 0.5) * tile_world_width;
    let offset_y = (tile_y as f64 - tiles_y as f64 / 2.0 + 0.5) * tile_world_height;

    // Apply rotation if needed
    let (rot_x, rot_y) = if config.rotation.abs() > 0.001 {
        let cos = config.rotation.cos();
        let sin = config.rotation.sin();
        (offset_x * cos - offset_y * sin, offset_x * sin + offset_y * cos)
    } else {
        (offset_x, offset_y)
    };

    (config.pan_x + rot_x, config.pan_y + rot_y)
}
```

### Tile Rendering Loop

```rust
async fn render_tiled(
    config: &FractalConfig,
    render_width: u32,
    render_height: u32,
) -> Result<Vec<u8>, ExportError> {
    let (tiles_x, tiles_y, tile_size) = calculate_tile_grid(render_width, render_height);
    let mut final_image = vec![0u8; (render_width * render_height * 4) as usize];

    for ty in 0..tiles_y {
        for tx in 0..tiles_x {
            // Calculate actual tile dimensions (edge tiles may be smaller)
            let tile_w = (render_width - tx * tile_size).min(tile_size);
            let tile_h = (render_height - ty * tile_size).min(tile_size);

            // Adjust viewport for this tile
            let (pan_x, pan_y) = calculate_tile_viewport(
                config, tx, ty, tiles_x, tiles_y, render_width, render_height
            );

            // Render tile
            let tile_config = config.with_pan(pan_x, pan_y);
            let tile_pixels = render_single(&tile_config, tile_w, tile_h).await?;

            // Blit to final image
            blit_tile(&mut final_image, render_width, &tile_pixels,
                      tx * tile_size, ty * tile_size, tile_w, tile_h);
        }
    }

    Ok(final_image)
}
```

## Downsampling

### CPU Box Filter (Simple, Initial Implementation)

```rust
fn downsample(
    src: &[u8],
    src_width: u32, src_height: u32,
    dst_width: u32, dst_height: u32,
) -> Vec<u8> {
    let scale = src_width / dst_width; // Assumes integer scale (2 or 4)
    let mut dst = vec![0u8; (dst_width * dst_height * 4) as usize];

    for y in 0..dst_height {
        for x in 0..dst_width {
            let mut r_sum = 0u32;
            let mut g_sum = 0u32;
            let mut b_sum = 0u32;
            let mut a_sum = 0u32;

            // Average scale×scale source pixels
            for sy in 0..scale {
                for sx in 0..scale {
                    let src_x = x * scale + sx;
                    let src_y = y * scale + sy;
                    let src_idx = ((src_y * src_width + src_x) * 4) as usize;

                    r_sum += src[src_idx] as u32;
                    g_sum += src[src_idx + 1] as u32;
                    b_sum += src[src_idx + 2] as u32;
                    a_sum += src[src_idx + 3] as u32;
                }
            }

            let count = (scale * scale) as u32;
            let dst_idx = ((y * dst_width + x) * 4) as usize;
            dst[dst_idx] = (r_sum / count) as u8;
            dst[dst_idx + 1] = (g_sum / count) as u8;
            dst[dst_idx + 2] = (b_sum / count) as u8;
            dst[dst_idx + 3] = (a_sum / count) as u8;
        }
    }

    dst
}
```

### GPU Downsample (Future Enhancement)

For very large images, GPU downsampling would be faster. This would:
1. Add a downsample compute shader
2. Run per-tile before CPU readback
3. Reduce data transfer size by 4× or 16×

**Deferred:** CPU downsample is fast enough for initial implementation. GPU optimization can be added later if needed.

## Implementation Plan

### Phase 1: Tiled Export (No Supersampling)
- [ ] Add `render_tiled()` function
- [ ] Implement tile coordinate calculation
- [ ] Implement tile blitting
- [ ] Auto-detect when tiling is needed
- [ ] Test at 5000×5000, 8000×8000, 10000×10000

### Phase 2: Supersampling
- [ ] Add supersample parameter to export
- [ ] Calculate render size = output × supersample
- [ ] Implement CPU downsample function
- [ ] Add UI toggle (Off/2×/4×)
- [ ] Test quality improvement vs performance cost

### Phase 3: Polish
- [ ] Progress reporting (tile X of Y)
- [ ] Memory usage estimation in UI
- [ ] Error handling for OOM
- [ ] CLI support (`--supersample 2`)

### Future: GPU Downsample (Optional)
- [ ] Add downsample.wgsl shader
- [ ] Downsample per-tile before readback
- [ ] Benchmark vs CPU downsample

## Examples

| Request | Internal Render | Tiles | Final Output |
|---------|-----------------|-------|--------------|
| 1920×1080, 1× | 1920×1080 | 1 | 1920×1080 |
| 1920×1080, 2× | 3840×2160 | 2×2 | 1920×1080 |
| 4000×4000, 1× | 4000×4000 | 2×2 | 4000×4000 |
| 4000×4000, 2× | 8000×8000 | 4×4 | 4000×4000 |
| 8000×8000, 1× | 8000×8000 | 4×4 | 8000×8000 |

## Memory Analysis

With MAX_TILE_PIXELS = 4,000,000 (~2000×2000):
- Histogram buffer per tile: 2000×2000×16 = **64 MB**
- Accumulation textures: 2000×2000×8×2 = **64 MB**
- Safe for GPUs with 256 MB buffer limit

Final image buffer (CPU):
- 8000×8000×4 = 256 MB
- 10000×10000×4 = 400 MB
- Manageable for systems with 8+ GB RAM

## Success Criteria

- [ ] Export 5000×5000 without GPU buffer errors
- [ ] Export 10000×10000 in reasonable time
- [ ] No visible seams between tiles
- [ ] 2× supersampling produces noticeably smoother edges
- [ ] Memory stays within GPU limits regardless of output size

## References

- Archived: [supersampling-antialiasing.md](../archive/supersampling-antialiasing.md)
- Archived: [tiled-high-res-export.md](../archive/tiled-high-res-export.md)
