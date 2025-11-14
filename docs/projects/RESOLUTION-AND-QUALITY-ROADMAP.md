# Resolution and Quality Roadmap

**Overview:** Integrated plan for unlimited resolution exports with anti-aliasing

## Current Limitations

### Buffer Size Limits
- **Histogram buffer:** `width × height × 16 bytes`
- **GPU max buffer size:** ~256MB (typical)
- **Current max resolution:** ~3000×3000 @ 1× (144MB)
- **User request:** 5000×5000 fails with "Buffer size 484000000 is greater than the maximum buffer size (268435456)"

### No Spatial Anti-Aliasing
- Temporal accumulation (progressive refinement) ✅
- Spatial supersampling ❌
- Result: Jagged edges, stairstepping on diagonals

## Solution: Two Complementary Features

### 1. Tiled High-Resolution Export
**Status:** Planning
**Doc:** [tiled-high-res-export.md](tiled-high-res-export.md)

**What it solves:**
- Export at any resolution (5K, 8K, 10K+)
- Memory stays constant (~64-256MB per tile)
- Works around GPU buffer limits

**How it works:**
- Render large image in tiles (e.g., 2048×2048 chunks)
- Each tile is a separate viewport into fractal space
- Composite tiles into final image
- Memory usage independent of output size

**Example:**
```
5000×5000 output:
├─ Tile grid: 3×3 (9 tiles @ 2048×2048 each)
├─ Memory per tile: 64MB
├─ Total memory: 64MB (constant)
└─ Render time: 9× baseline
```

### 2. Supersampling Anti-Aliasing
**Status:** Planning
**Doc:** [supersampling-antialiasing.md](supersampling-antialiasing.md)

**What it solves:**
- Smooth edges and curves
- Eliminate stairstepping artifacts
- Publication-quality output

**How it works:**
- Render at higher resolution (2× or 4× in each dimension)
- Downsample with box/bilinear filter
- Result: Each output pixel = average of 4 or 16 samples

**Example:**
```
1920×1080 output @ 2× SS:
├─ Render at: 3840×2160 internally
├─ Downsample: 4 samples per pixel
├─ Memory: 4× normal (124MB @ 1080p)
└─ Render time: 4× baseline
```

## The Perfect Combination

**Tiled Export + Supersampling = Unlimited Resolution + Quality**

### Architecture

```
8K export (7680×4320) with 2× supersampling:

Without tiling:
├─ Internal resolution: 15360×8640
├─ Histogram buffer: 2.0GB
└─ Result: ❌ FAILS (exceeds 256MB limit)

With tiling:
├─ Tile size: 2048×2048
├─ Internal per tile: 4096×4096 (with 2× SS)
├─ Histogram per tile: 256MB
├─ Tiles: 4×3 = 12 tiles
├─ For each tile:
│   1. Render at 4096×4096
│   2. Downsample to 2048×2048 (anti-aliasing)
│   3. Blit to final image
└─ Result: ✅ WORKS (constant 256MB memory)
```

### Memory Analysis

| Output Size | SS Factor | Without Tiling | With Tiling (2048² tiles) |
|------------|-----------|----------------|---------------------------|
| 4096×4096 | 1× | 256MB ✅ | 64MB ✅ |
| 4096×4096 | 2× | 1024MB ❌ | 256MB ✅ |
| 8192×8192 | 1× | 1024MB ❌ | 64MB ✅ |
| 8192×8192 | 2× | 4096MB ❌ | 256MB ✅ |
| 10000×10000 | 1× | 1600MB ❌ | 64MB ✅ |
| 10000×10000 | 2× | 6400MB ❌ | 256MB ✅ |

**Key Insight:** Tiling makes supersampling viable at any resolution.

## Implementation Roadmap

### Phase 1: Tiled Export (CLI Only) - **IMMEDIATE PRIORITY**
**Time:** ~3-5 days
**Solves:** User's 5000×5000 export need

- [ ] Tile offset calculation with rotation support
- [ ] Tile rendering loop
- [ ] Tile blitting into final image
- [ ] CLI flags: `--width`, `--height`, `--tile-size`
- [ ] Auto-detect safe tile size from GPU limits
- [ ] Progress reporting

**Outcome:** Enable unlimited resolution exports at 1× quality

### Phase 2: Supersampling (Viewport + Export)
**Time:** ~5-6 days
**Requires:** Phase 1 complete (for high-res SS exports)

- [ ] Add render/display resolution separation to FlameRenderer
- [ ] Create downsample shader (box or bilinear filter)
- [ ] Add downsample pass to render pipeline
- [ ] Add `supersample_factor` to FractalConfig
- [ ] UI controls with memory usage display
- [ ] Works for viewport-sized exports

**Outcome:** Enable anti-aliased exports up to GPU buffer limit

### Phase 3: Integrate Tiling + Supersampling
**Time:** ~1-2 days
**Requires:** Phases 1 & 2 complete

- [ ] Apply supersampling per-tile in tiled export
- [ ] Reuse downsample shader for each tile
- [ ] Update tile size calculation for SS factor
- [ ] CLI flag: `--supersample 1|2|4`
- [ ] Test quality at various tile sizes and SS factors

**Outcome:** Unlimited resolution + anti-aliased exports

### Phase 4: UI Integration
**Time:** ~2-3 days

- [ ] "Export High-Res PNG" dialog
- [ ] Resolution presets (1080p, 4K, 8K, Custom)
- [ ] Show estimated memory and render time
- [ ] Tile grid visualization
- [ ] Async export with progress bar
- [ ] Preview mode (render single center tile)

**Outcome:** User-friendly high-res export from app

## Recommended Settings

### For 5000×5000 Export (User's Current Need)
```bash
# No anti-aliasing (fastest)
fractal_flame_wgpu export -i config.fflame -o output.png \
  --width 5000 --height 5000

# With 2× anti-aliasing (recommended quality)
fractal_flame_wgpu export -i config.fflame -o output.png \
  --width 5000 --height 5000 --supersample 2

# Tile size auto-detected based on GPU (typically 2048 or 3072)
```

### For 8K Export (Publication Quality)
```bash
fractal_flame_wgpu export -i config.fflame -o output.png \
  --width 7680 --height 4320 \   # 8K UHD
  --supersample 2 \               # Anti-aliasing
  --tile-size 2048                # Safe for most GPUs
```

### For 10K+ Export (Poster/Print)
```bash
fractal_flame_wgpu export -i config.fflame -o output.png \
  --width 10000 --height 10000 \
  --supersample 2 \
  --tile-size 1536                # Smaller tiles for 2× SS safety
```

## Performance Expectations

### Tiling Overhead
- **Per-tile:** Same speed as viewport render
- **Total time:** Scales linearly with pixel count
- **Example:** 4K image (4 tiles) = 4× baseline render time

### Supersampling Overhead
- **2× SS:** 4× slower (4× pixels)
- **4× SS:** 16× slower (16× pixels)
- **Downsample pass:** Negligible (~0.1ms)

### Combined
- **4K @ 2× SS:** 16× baseline (4 tiles × 4× SS)
- **8K @ 2× SS:** 64× baseline (16 tiles × 4× SS)
- **Typical render:** 10 billion iterations @ 800×600 = ~5 seconds
  - 8K @ 2× SS with same iteration density = ~320 seconds (~5 minutes)

## Success Metrics

### Phase 1 Complete When:
- ✅ Export 5000×5000 PNG without GPU errors
- ✅ Export 10000×10000 PNG successfully
- ✅ Memory stays under GPU limits
- ✅ No visible seams between tiles
- ✅ Pixel-perfect match with viewport (same resolution)

### Phase 2 Complete When:
- ✅ Viewport renders with 2× and 4× supersampling
- ✅ Smooth edges visible on test fractals
- ✅ Export up to 1920×1080 @ 4× SS (497MB, under limit)
- ✅ UI shows memory usage and auto-disables if unsafe

### Phase 3 Complete When:
- ✅ Export 8K @ 2× SS successfully
- ✅ Quality matches viewport SS render
- ✅ Memory stays constant regardless of resolution
- ✅ Tile size auto-adjusts for SS factor

### Phase 4 Complete When:
- ✅ UI dialog for high-res export
- ✅ Async export with progress updates
- ✅ Preview mode renders single tile
- ✅ User-friendly presets and estimates

## Decision: Implement Phase 1 Now?

**Recommendation:** YES

**Reasons:**
1. **User has immediate need:** Cannot export at 5000×5000
2. **Straightforward implementation:** ~200-300 lines, no complex dependencies
3. **High impact:** Unlocks unlimited resolution exports immediately
4. **Foundation for Phase 3:** Tiling infrastructure needed for SS integration
5. **Low risk:** Self-contained feature, doesn't affect existing code

**Estimated Time:**
- Implementation: 1-2 days
- Testing: 0.5 days
- Total: **1.5-2.5 days**

**Deliverable:**
```bash
# User can immediately do this:
cargo run --release -- export -i config.fflame -o huge.png \
  --width 5000 --height 5000

# And this will work without GPU errors
```

## Next Steps

**If approved to proceed with Phase 1:**

1. Create branch `feature/tiled-export`
2. Implement core functions:
   - `calculate_tile_offset()`
   - `render_tile()`
   - `blit_tile()`
   - `export_tiled_png()`
3. Add CLI args parsing for `--width`, `--height`, `--tile-size`
4. Test at 2K, 4K, 5K, 8K, 10K resolutions
5. Verify no visible seams
6. Merge to main

Would you like me to proceed with Phase 1 implementation?

## Related Documentation

- [tiled-high-res-export.md](tiled-high-res-export.md) - Detailed tiled rendering design
- [supersampling-antialiasing.md](supersampling-antialiasing.md) - Detailed supersampling design
- [../STATUS.md](../STATUS.md) - Feature priority tracking
- [../../CLAUDE.md](../../CLAUDE.md) - Project overview

---

**Created:** 2025-11-13
**Status:** Planning → Ready for Implementation
