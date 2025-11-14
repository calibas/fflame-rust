# PNG Export Brightness Bug

**Status**: Investigating
**Date Started**: 2025-11-14
**Priority**: High

## Problem Statement

PNG exports (both headless CLI and interactive app export) have incorrect brightness that is resolution-dependent:
- **Small resolutions**: Too BRIGHT
- **Large resolutions**: Too DARK
- **Never converges** to correct brightness regardless of iteration count

The interactive viewport renders with **correct brightness** at all resolutions and converges quickly (within millions of iterations).

## Symptoms

### Viewport (Interactive Rendering) ✓ CORRECT
- Brightness correct at any resolution (tested at 1067×1022 and 1920×1080)
- Reaches correct brightness in fraction of a second (~millions of iterations)
- No resolution-dependent brightness issues

### PNG Export (Both Headless & App) ✗ WRONG
- Small resolutions (e.g., 800×600): Too bright
- Large resolutions (e.g., 1920×1080): Too dark
- Does NOT converge to correct brightness even with 2 billion iterations
- Affects both:
  - Headless CLI export (`cargo run -- export`)
  - Interactive app export (File → Export PNG with custom dimensions)

## Test Case

**Config**: discus3.fflame
**Settings**:
- iterations_per_thread: 1000
- speed_multiplier: 4
- max_iterations: 1,995,262,314
- density_scale: 1
- histogram_color_scale: 100
- brightness: 12
- exposure: 1

**Results**:
- Viewport at 1067×1022: Correct brightness ✓
- Export at 1920×1080: Too dark ✗
- Export runs 975 accumulation passes, 1,996,800,000 iterations (verified accurate)

## What We've Ruled Out

### 1. Iteration Count ✗
- Tested with up to 2 billion iterations
- Viewport reaches correct brightness with far fewer iterations
- **Conclusion**: Not an iteration count issue

### 2. Dynamic Blend Mode ✗
- Disabled dynamic blend (`use_dynamic_blend = false`) in export code
- No change in brightness
- **Conclusion**: Not a blending issue

### 3. Tonemap Parameters ✗
- Verified all tonemap params are identical:
  - `area = width × height` (correct)
  - `sample_density = 10000 / area` (correct)
  - `brightness = 12` (correct)
- Debug output confirms values match between viewport and export
- **Conclusion**: Tonemap parameters are correct

### 4. Iteration Counting Accuracy ✗
- Added debug counters to verify actual iteration count
- Math checks out: 975 passes × 2,048,000 iter/pass = 1,996,800,000 ✓
- **Conclusion**: Iterations are being counted correctly

### 5. Brightness Formula ✗
- Formula uses `k2 = 1.0 / (area × white_level × sample_density)`
- Since `area × sample_density = constant` (10,000), k2 should be resolution-independent
- **Conclusion**: The Apophysis brightness formula itself is resolution-independent

## Critical Observations

### Code Paths
Both viewport and export use the same 3-pass pipeline:
1. **Compute pass**: Generate samples → histogram
2. **Accumulate pass**: Blend histogram → accumulation buffer
3. **Tonemap pass**: Accumulation buffer → display texture

### Key Difference
- **Viewport**: Renders continuously at viewport resolution, correct brightness
- **Export**: Creates temporary renderer at export resolution, wrong brightness

### Debug Output Comparison

**Viewport (correct)**:
```
update_tonemap: width=1067, height=1022, area=1090474, sample_density=0.009170325, brightness=12
```

**Export (wrong)**:
```
update_tonemap: width=1920, height=1080, area=2073600, sample_density=0.004822531, brightness=12
```

Both calculations are mathematically correct, yet produce different visual results.

## Hypotheses to Investigate

### 1. Accumulation Buffer Initialization
- Does the viewport accumulation buffer have different initial state?
- Is there a clear/reset difference?

### 2. Histogram Clearing Behavior
- Viewport clears histogram conditionally (batching)
- Export clears histogram every iteration
- Could this affect density accumulation?

### 3. Hidden Parameters
- Is there a parameter being set in viewport that we're missing in export?
- Check: density_scale, histogram_color_scale, blend_factor, etc.

### 4. Render Encoder Differences
- Are command encoders configured differently?
- Pipeline states?

### 5. Texture Format Differences
- Both use Rgba8Unorm for display
- Could internal buffer formats differ?

## Next Steps

1. **Line-by-line code comparison** between viewport and export render paths
2. **Add comprehensive debug logging** to both paths to identify divergence
3. **Test hypothesis**: Export at viewport resolution (1067×1022) to see if brightness matches
4. **Check renderer state** after creation vs after first frame in viewport

## References

- Main viewport render loop: `src/app/mod.rs:1050-1140`
- Custom export: `src/app/config.rs:100-221`
- Headless export: `src/app/export.rs:5-155`
- Tonemap shader: `shaders/tonemap.wgsl`
- Accumulate shader: `shaders/accumulate.wgsl`

## Related Code

### Viewport Rendering
```rust
// src/app/mod.rs ~line 1092
let samples_this_frame = renderer.compute_pass(&mut render_encoder, &self.gpu.queue, NUM_WORKGROUPS,
    final_config.iterations_per_thread, ...);
renderer.accumulate_pass(&mut render_encoder, &self.gpu.queue, &self.gpu.device, total_samples_in_batch);
renderer.tonemap_pass(&mut render_encoder);
```

### Export Rendering
```rust
// src/app/config.rs ~line 160
temp_renderer.compute_pass(&mut encoder, &self.gpu.queue, NUM_WORKGROUPS,
    iterations_per_frame, ...);
temp_renderer.accumulate_pass(&mut encoder, &self.gpu.queue, &self.gpu.device, samples);
temp_renderer.tonemap_pass(&mut final_encoder);
```

## Notes

- The viewport rendering code already solves this problem correctly
- The solution exists in the codebase - we just need to identify what makes it work
- This is NOT a fundamental Apophysis algorithm issue - our viewport proves the algorithm works
