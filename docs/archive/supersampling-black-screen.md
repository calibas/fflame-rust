# Supersampling Black Screen Debug Log

## Problem
2× supersampling shows black screen even though iterations are accumulating.

## Environment
- GPU max_buffer_size: 256 MB
- GPU max_storage_buffer_binding_size: 2047 MB
- 4× fails due to buffer size (expected), 2× should work

## Test Results

### Test 1: Debug fill stitched texture with magenta
- Location: After `accumulate_and_stitch()`, before display
- Result: **MAGENTA VISIBLE**
- Conclusion: stitched_texture → egui display path works

### Test 2: Accumulate shader outputs cyan unconditionally
- Modified `accumulate_tiled.wgsl` to output cyan at start of main()
- Result: **BLACK**
- Conclusion: Either:
  1. Accumulate shader not being dispatched
  2. Accumulate shader output not connected to tonemap input
  3. Tonemap pass not copying to stitched texture

## Pipeline Flow
```
compute_pass_frame() → histogram buffer (per tile)
     ↓
accumulate_and_stitch() for each tile:
     ↓
  accumulate shader: histogram → output_texture (Rgba16Float)
     ↓
  run_tonemap_pass(): output_texture → tonemap_output_texture (Rgba8Unorm)
     ↓
  copy_texture_to_texture: tonemap_output → stitched_texture at (start_x, start_y)
     ↓
get_final_display_view() → returns stitched_texture_view
```

## Key Code Locations
- `src/export/renderer.rs:1904` - `accumulate_and_stitch()` function
- `src/export/renderer.rs:1954-1966` - accumulate bind group creation
- `src/export/renderer.rs:1976-1984` - accumulate shader dispatch
- `src/export/renderer.rs:1987` - `run_tonemap_pass()` call
- `shaders/accumulate_tiled.wgsl` - accumulate compute shader

## Next Steps to Investigate
1. Check if accumulate pipeline is created correctly
2. Check if bind group resources are valid
3. Check if tonemap pass receives the right texture
4. Add more granular debug outputs

## Relevant Logs
```
TiledRenderer compute: 8388608 samples, total=1002700800
get_final_display_view: returning stitched texture 2134x2044
Using TILED renderer path
TiledRenderer: paused=false, should_iterate=false, iterations=1002700800
```
