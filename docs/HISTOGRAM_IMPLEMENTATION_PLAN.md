# Histogram-Based Color Accumulation Implementation Plan

## Problem Statement

Currently, when multiple iterations within a single compute dispatch hit the same pixel, they race to write their colors via `textureStore()`. The last write wins, causing color noise and incorrect blending. This is especially noticeable in complex fractals with high iteration counts per thread (256 iterations/thread).

**Example:**
- Thread 1, iteration 5 hits pixel (100, 100) → writes `(red, 0.01)`
- Thread 1, iteration 10 hits pixel (100, 100) → writes `(green, 0.01)` ← **overwrites red!**
- Thread 2, iteration 3 hits pixel (100, 100) → writes `(blue, 0.01)` ← **overwrites green!**
- Result: Only blue survives, but the pixel should be purple (average of red+green+blue)

## Solution: Atomic Histogram Accumulation

Use integer texture atomic operations to build a histogram where:
- **Color sum** accumulates in `rgba32uint` texture
- **Hit count** accumulates in `r32uint` texture
- **Final color** = color_sum / hit_count (averaged in accumulate pass)

### Why This Works
- `textureAtomicAdd` guarantees no race conditions
- All color contributions are summed correctly
- Properly averages colors that hit the same pixel
- No performance penalty (atomics are fast on modern GPUs)
- Matches Apophysis CPU histogram approach

## Architecture Changes

### 1. GPU Buffers (DONE ✓)

**Already completed in commit 75045ce:**

```rust
// In src/gpu/buffers.rs - FlameBuffers struct
pub histogram_color_texture: Texture,       // Rgba32Uint
pub histogram_color_view: TextureView,
pub histogram_density_texture: Texture,     // R32Uint
pub histogram_density_view: TextureView,
```

### 2. Bind Group Updates

**File:** `src/gpu/pipelines.rs`

**Compute Bind Group (Group 0):**
```rust
// Current bindings (0-3):
@binding(0) transforms: storage buffer
@binding(1) params: uniform
@binding(2) output_texture: texture_storage_2d<rgba16float>  // REMOVE THIS
@binding(3) palette_texture

// New bindings:
@binding(0) transforms: storage buffer
@binding(1) variation_params: storage buffer
@binding(2) params: uniform
@binding(3) histogram_color: texture_storage_2d<rgba32uint, read_write>  // NEW
@binding(4) histogram_density: texture_storage_2d<r32uint, read_write>   // NEW
@binding(5) palette_texture: texture_1d
@binding(6) palette_sampler
```

**Accumulate Bind Group (Group 0):**
```rust
// Current bindings:
@binding(0) previous_accumulation: texture_2d<f32>
@binding(1) new_samples: texture_2d<f32>              // REMOVE THIS
@binding(2) output_texture: texture_storage_2d
@binding(3) params: uniform

// New bindings:
@binding(0) previous_accumulation: texture_2d<f32>
@binding(1) histogram_color: texture_2d<u32>          // NEW (read-only)
@binding(2) histogram_density: texture_2d<u32>        // NEW (read-only)
@binding(3) output_texture: texture_storage_2d<rgba16float>
@binding(4) params: uniform
```

**Notes:**
- Remove `temp_samples_texture` (no longer needed)
- Histogram textures serve both compute (write) and accumulate (read) passes
- Must clear histogram textures at start of each frame

### 3. Shader Changes

#### 3A. Compute Shader (main_2d.wgsl, main_3d.wgsl)

**Header changes:**
```wgsl
// OLD:
@group(0) @binding(2) var output_texture: texture_storage_2d<rgba16float, write>;

// NEW:
@group(0) @binding(3) var histogram_color: texture_storage_2d<rgba32uint, read_write>;
@group(0) @binding(4) var histogram_density: texture_storage_2d<r32uint, read_write>;
```

**Main loop changes:**
```wgsl
// OLD (line 71 in main_2d.wgsl):
textureStore(output_texture, pixel, vec4<f32>(final_color, 0.01));

// NEW:
// Scale color to integers (0.0-1.0 → 0-10000 for precision)
let color_scale = 10000.0;
let color_uint = vec4<u32>(
    u32(clamp(final_color.r, 0.0, 1.0) * color_scale),
    u32(clamp(final_color.g, 0.0, 1.0) * color_scale),
    u32(clamp(final_color.b, 0.0, 1.0) * color_scale),
    0u
);

// Atomic add to histogram (no race conditions!)
textureAtomicAdd(histogram_color, pixel, color_uint.r);   // R channel
textureAtomicAdd(histogram_color, pixel, color_uint.g);   // G channel
textureAtomicAdd(histogram_color, pixel, color_uint.b);   // B channel
textureAtomicAdd(histogram_density, pixel, 1u);           // Hit count
```

**Wait, issue with textureAtomicAdd:** It operates on scalar values, not vec4. Need to do per-channel:

```wgsl
// Correct approach:
let pixel_idx = pixel.y * i32(params.width) + pixel.x;

// For rgba32uint, we need to atomic-add each channel separately
// WGSL textureAtomicAdd only works on r32uint, not rgba32uint!
// Solution: Use 3 separate r32uint textures or one rgba32uint with component selection

// Actually, textureAtomicAdd for rgba32uint doesn't exist in WGSL
// We need to use r32uint storage and manually pack/unpack channels
```

**Revised approach - use storage buffer instead of texture:**

Actually, WGSL supports `textureAtomicAdd` for `r32uint` format only. For RGBA, we need either:
1. Use 4 separate `r32uint` textures (R, G, B, A)
2. Use a storage buffer instead
3. Use `atomicAdd` on a storage buffer with manual indexing

**Best approach: Storage buffer histogram**

### 4. Revised Architecture Using Storage Buffer

**Buffers struct:**
```rust
pub histogram_buffer: Buffer,  // Storage buffer: [width * height * 4] u32s
                                // Layout: [r, g, b, density] for each pixel
```

**Compute shader:**
```wgsl
@group(0) @binding(3) var<storage, read_write> histogram: array<atomic<u32>>;

// In main():
let pixel_idx = u32(pixel.y) * params.width + u32(pixel.x);
let base_idx = pixel_idx * 4u;

// Scale color to integers
let color_scale = 10000.0;
let r = u32(clamp(final_color.r, 0.0, 1.0) * color_scale);
let g = u32(clamp(final_color.g, 0.0, 1.0) * color_scale);
let b = u32(clamp(final_color.b, 0.0, 1.0) * color_scale);

// Atomic add (thread-safe!)
atomicAdd(&histogram[base_idx + 0u], r);
atomicAdd(&histogram[base_idx + 1u], g);
atomicAdd(&histogram[base_idx + 2u], b);
atomicAdd(&histogram[base_idx + 3u], 1u);  // density
```

**Accumulate shader:**
```wgsl
@group(0) @binding(1) var<storage, read> histogram: array<u32>;

// In main():
let pixel_idx = u32(global_id.y) * params.width + u32(global_id.x);
let base_idx = pixel_idx * 4u;

// Read histogram values
let r_sum = f32(histogram[base_idx + 0u]);
let g_sum = f32(histogram[base_idx + 1u]);
let b_sum = f32(histogram[base_idx + 2u]);
let density = f32(histogram[base_idx + 3u]);

// Convert back to float color (average)
let color_scale = 10000.0;
var new_color = vec3<f32>(0.0);
if (density > 0.0) {
    new_color = vec3<f32>(
        r_sum / (density * color_scale),
        g_sum / (density * color_scale),
        b_sum / (density * color_scale)
    );
}

// Continue with existing accumulation logic
let prev = textureLoad(previous_accumulation, pixel, 0);
let rgb_accumulated = prev.rgb * (1.0 - params.blend_factor) + new_color * params.blend_factor;
let alpha_accumulated = prev.a + (density / color_scale * 0.01);  // Normalize density
```

## Implementation Steps

### Step 1: Update FlameBuffers ✓ (DONE - 2025-10-25)
- [x] Add histogram textures to struct
- [x] Create textures in `new()`
- [x] Add to clear functions

### Step 2: Replace Textures with Storage Buffer ✓ (DONE - 2025-10-25)
- [x] Remove `histogram_color_texture` and `histogram_density_texture`
- [x] Add `histogram_buffer: Buffer` (size: width × height × 4 × sizeof(u32))
- [x] Update `clear_all()` to zero the buffer
- [x] Update `resize()` to recreate buffer (handled by FlameBuffers::new())
- [x] Add `clear_histogram()` method for per-frame clearing

### Step 3: Update Bind Group Layouts ✓ (DONE - 2025-10-25)
- [x] Update compute bind group layout in `pipelines.rs` (binding 2 → histogram buffer)
- [x] Update accumulate bind group layout in `pipelines.rs` (binding 1 → histogram buffer)
- [x] Update `create_compute_bind_group()` to bind histogram buffer
- [x] Update `create_accumulate_bind_group()` to bind histogram buffer

### Step 4: Update Shaders ✓ (DONE - 2025-10-25)
- [x] Modify `shaders/core/header.wgsl` to declare histogram buffer (binding 2, atomic<u32>)
- [x] Modify `shaders/core/main_2d.wgsl` to use atomicAdd
- [x] Modify `shaders/core/main_3d.wgsl` to use atomicAdd
- [x] Modify `shaders/accumulate.wgsl` to read histogram and convert

### Step 5: Frame Management ✓ (DONE - 2025-10-25)
- [x] Clear histogram buffer at start of each frame (before compute pass)
- [x] Ensure compute pass completes before accumulate pass reads histogram

### Step 6: Testing (IN PROGRESS)
- [ ] Test with simple single-color fractal (verify color accuracy)
- [ ] Test with multi-color complex fractal (verify no color noise)
- [ ] Compare visual output with Apophysis
- [ ] Verify performance (should be similar or better)

## Performance Considerations

**Memory:**
- Histogram buffer: width × height × 4 × 4 bytes
- Example: 1920×1080 = 31MB (reasonable)

**Bandwidth:**
- Atomic operations are coalesced by GPU
- Storage buffer access is cached
- Should be similar performance to current approach

**Atomics:**
- Modern GPUs handle atomics efficiently
- Much faster than multiple shader dispatches
- No synchronization overhead (happens in hardware)

## Expected Results

**Before:**
- Color noise in complex fractals
- Random color wins at each pixel
- Doesn't match Apophysis

**After:**
- Proper color averaging
- All iterations contribute to final color
- Matches Apophysis exactly

## Rollback Plan

If histogram approach has issues:
1. Keep changes in feature branch
2. Can revert to main branch
3. Already committed initial work (75045ce)

## References

- WGSL Atomic Functions: https://www.w3.org/TR/WGSL/#atomic-builtin-functions
- Fractal Flame Algorithm: Uses histogram on CPU (Apophysis, flam3)
- Storage Buffer Atomics: Supported in all WebGPU implementations
