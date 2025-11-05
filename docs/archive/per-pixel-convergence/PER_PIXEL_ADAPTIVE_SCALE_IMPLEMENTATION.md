# Per-Pixel Adaptive Scale Implementation

## Status: ✅ COMPLETE (All 4 Phases)

Implementation of per-pixel adaptive histogram scaling to prevent overflow while maximizing precision.

## Overview

Each pixel tracks its own scale value (u16, range 1-100) which controls how color values are encoded/decoded in the histogram buffer. The scale is dynamically adjusted each frame based on local density to prevent overflow in high-density areas and maximize precision in low-density areas.

## Architecture

### Memory Layout

**Scale Buffer**: `Buffer` (storage, read-write in adjust_scale, read-only elsewhere)
- Size: `(width * height) / 2` u32 words
- Packing: 2× u16 scale values per u32
  - Even pixels: low 16 bits `(word & 0xFFFF)`
  - Odd pixels: high 16 bits `((word >> 16) & 0xFFFF)`
- Initial value: All pixels = 50 (balanced starting point)
- Memory cost: 4 MB @ 1080p (1920×1080×2 bytes)

### Data Flow

```
Frame N:
1. Compute Pass
   - Read scale_buffer[pixel] → pixel_scale
   - Encode: r16 = color.r * pixel_scale
   - atomicAdd to histogram

2. Adjust Scale Pass (NEW!)
   - Read histogram[pixel] → density, r_sum, g_sum, b_sum
   - Calculate max_accumulated = max(r_sum, g_sum, b_sum)
   - if max_accumulated > overflow_threshold:
       new_scale = scale / (1 + adjust_rate * overflow_risk)
   - else if density > high_density_threshold:
       new_scale = scale / (1 + adjust_rate * density_factor)
   - else if density < low_density_threshold:
       new_scale = scale * (1 + adjust_rate * sparsity_factor)
   - Write scale_buffer[pixel] = clamp(new_scale, min_scale, max_scale)

3. Accumulate Pass
   - Read scale_buffer[pixel] → pixel_scale
   - Decode: color.r = r_sum / (density * pixel_scale)
   - Blend with previous accumulation
```

### Parameters

**AdjustScaleParams** (uniform buffer):
- `overflow_threshold: 50000.0` - Max safe accumulated value before aggressive scale reduction
- `high_density_threshold: 100.0` - Density to trigger scale reduction
- `low_density_threshold: 10.0` - Density to trigger scale increase
- `scale_adjust_rate: 0.1` - How aggressively to adjust (0.0-1.0)
- `min_scale: 1.0` - Minimum allowed scale
- `max_scale: 100.0` - Maximum allowed scale

## Implementation Phases

### Phase 1: Infrastructure ✅
**Commit**: 41a12db
- Added `scale_buffer: Buffer` to FlameBuffers
- Initialized all pixels to scale=50
- u32 array with u16 values packed 2 per word
- Added binding(6) to compute bind group layout
- Added scale_buffer to shader header

### Phase 2: Shader Integration ✅
**Commit**: 1d86ebc
- Updated main_2d.wgsl to load per-pixel scale
- Updated main_3d.wgsl to load per-pixel scale
- Updated accumulate.wgsl to decode with per-pixel scale
- Added scale_buffer to accumulate bind group layout
- Both write (compute) and read (accumulate) paths use same scale

### Phase 3: Adjust Scale Shader ✅
**Commit**: 8146cb4
- Created shaders/adjust_scale.wgsl
- Added AdjustScaleParams struct to buffers.rs
- Created adjust_scale pipeline and bind group layout
- Added adjust_scale_params_buffer initialization
- Shader analyzes histogram density and adjusts scales

### Phase 4: Render Loop Integration ✅
**Commit**: c13f1f5
- Added adjust_scale_bind_group to FlameRenderer
- Created FlameRenderer::adjust_scale_pass() method
- Integrated into app render loop (src/app/mod.rs)
- Integrated into export render loop (src/app/export.rs)
- Runs every frame: compute → adjust_scale → accumulate

## Performance

### Computational Cost
- **Adjust Scale Pass**: 8×8 workgroups (same as accumulate)
  - @ 1080p: 135 workgroups (15×9 tiles)
  - Per-pixel: Read 2× u32 histogram, read+write 1× u32 scale
  - Estimated overhead: **1-2% of total frame time**

### Memory Cost
- **Scale Buffer**: 4 MB @ 1080p
- **Total Histogram System**: 20 MB @ 1080p
  - 16 MB histogram (4× u32 per pixel: R, G, B, density)
  - 4 MB scale buffer (2× u16 per pixel)

## Benefits

1. **No Overflow**: High-density pixels automatically reduce scale before overflow
2. **Maximum Precision**: Low-density pixels use highest safe scale
3. **Adaptive**: Each pixel independently optimizes its own scale
4. **Automatic**: No manual tuning needed, works for any scene
5. **Per-Frame**: Scales update every frame based on current density

## Comparison to Global Scale

| Aspect | Global Scale | Per-Pixel Adaptive |
|--------|-------------|-------------------|
| Precision | One-size-fits-all | Optimal per pixel |
| Overflow Risk | High in dense areas | Auto-prevented |
| Low-Density Quality | Underutilized precision | Maximized |
| Memory | 0 bytes | 4 MB @ 1080p |
| Performance | 0% overhead | 1-2% overhead |
| Manual Tuning | Required | Automatic |

## Example Scale Evolution

**High-Density Pixel** (center of fractal):
```
Frame   Density  Max_Accum  Scale  Action
-----   -------  ---------  -----  ------
1       0        0          50     (initial)
10      150      7500       45     Reduce (high density)
20      300      13500      40     Reduce (high density)
50      800      32000      35     Reduce (approaching overflow)
100     1500     52500      25     Reduce aggressive (overflow risk)
```

**Low-Density Pixel** (background):
```
Frame   Density  Max_Accum  Scale  Action
-----   -------  ---------  -----  ------
1       0        0          50     (initial)
10      5        250        60     Increase (low density)
20      8        480        70     Increase (low density)
50      12       840        80     Increase (low density)
100     15       1200       90     Increase (low density)
```

## Future Enhancements (Optional)

### UI Controls (Phase 5)
- [ ] "Adaptive Scale" checkbox (enable/disable)
- [ ] Live scale statistics display (min/max/avg)
- [ ] Gray out manual scale slider when adaptive enabled
- [ ] Scale heatmap visualization (debug view)

### Advanced Tuning
- [ ] Expose threshold parameters in UI
- [ ] Scene-specific presets (bright/dark/mixed)
- [ ] Adaptive adjustment rate based on frame number
- [ ] Hysteresis to prevent scale oscillation

### Performance Optimization
- [ ] Run adjust_scale less frequently (every N frames)
- [ ] Coarser adjustment for distant pixels (mipmap-style)
- [ ] SIMD optimization for scale calculation

## Testing

Build and run to verify:
```bash
cargo build --release
cargo run --release
```

Expected behavior:
- Rendering should look identical to before (scale=50 everywhere initially)
- No overflow artifacts even in very dense areas
- Smooth gradients maintained in low-density areas
- No performance degradation (< 2% slower)

## Technical Notes

### Why After Compute, Before Accumulate?

1. **Compute writes** with current frame's scales
2. **Adjust reads** histogram to determine next frame's scales
3. **Accumulate reads** with current frame's scales (must match compute!)

If adjust ran before accumulate, scales would be inconsistent (write scale ≠ read scale).

### Why u16 Instead of u32?

- u16 range (1-100) is sufficient for scale values
- Packing 2 per u32 halves memory cost (4 MB vs 8 MB @ 1080p)
- Unpacking is trivial: `select(word & 0xFFFF, word >> 16, is_odd)`

### Why Storage Buffer Instead of Texture?

- Integer read-write atomicity (storage buffer guarantees)
- Simpler indexing: `scale_buffer[pixel_idx / 2]`
- No format conversion (direct u32 access)

## Related Documentation

- [docs/HISTOGRAM_INVESTIGATION_SUMMARY.md](HISTOGRAM_INVESTIGATION_SUMMARY.md) - Background research
- [docs/PER_PIXEL_ADAPTIVE_SCALE_PLAN.md](PER_PIXEL_ADAPTIVE_SCALE_PLAN.md) - Original implementation plan
- [docs/FINDINGS_PRESENTATION.md](FINDINGS_PRESENTATION.md) - Option comparison
