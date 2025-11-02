# Scale Buffer Removal - Performance Recovery Plan

## Problem Statement

After implementing u32 histogram (commit dd4bd8a), benchmarks show minor performance regression vs commit 9ac278a:
- Root cause: 2.5× increase in memory bandwidth usage
- Histogram: 2× u32 → 4× u32 per pixel (+2× atomic writes)
- Scale buffer: Added 1.9 MB storage buffer reads per iteration
- Total memory traffic: ~3.8 MB → ~9.6 MB @ 800×600

## Performance Analysis

### Memory Bandwidth Breakdown

**Before (commit 9ac278a):**
```
Histogram: 2× u32 per pixel = ~3.8 MB @ 800×600
  - 2 atomic writes per pixel hit
  - Packed RGB (u16) + density (u16)
Scale buffer: None
Total: ~3.8 MB per iteration
```

**After u32 histogram (commit dd4bd8a):**
```
Histogram: 4× u32 per pixel = ~7.7 MB @ 800×600
  - 4 atomic writes per pixel hit
  - Separate R, G, B, density (all u32)
Scale buffer: 1× u32 per pixel = ~1.9 MB @ 800×600
  - 1 storage buffer read per pixel hit
Total: ~9.6 MB per iteration
```

**Regression sources:**
1. Histogram 2× larger (necessary for overflow prevention)
2. Scale buffer storage reads (unnecessary - all pixels use same scale)

### Why Storage Buffer Reads Are Expensive

**Storage buffer read:**
- Global memory access
- Cache-unfriendly (large buffer, random access pattern)
- Memory bandwidth bottleneck
- Latency: ~100-400 cycles

**Uniform constant read:**
- Constant memory (dedicated cache)
- Single value broadcast to all threads
- Near-zero cost (cached on first access)
- Latency: ~1-4 cycles

**Impact:** Every iteration reads scale_buffer[pixel_idx]
- At 2M iterations/second, that's 2M storage buffer reads/second
- Replacing with uniform = ~100× faster access

## Solution: Replace Scale Buffer with Uniform

### Current Implementation (Per-Pixel Scale Buffer)

**Purpose:** Enable per-pixel adaptive scaling
**Reality:** All pixels use identical scale=50
**Cost:** 1.9 MB storage buffer + slow reads

**Files using scale_buffer:**
- `src/gpu/buffers.rs` - Buffer allocation and reset
- `src/gpu/pipelines.rs` - Binding layouts and bind groups
- `shaders/core/header.wgsl` - Binding declaration
- `shaders/core/main_2d.wgsl` - Scale read
- `shaders/core/main_3d.wgsl` - Scale read
- `shaders/accumulate.wgsl` - Binding (unused)
- `src/renderer/compute_kernel.rs` - Debug stats function

### Proposed Implementation (Uniform Constant)

**Change params uniform buffer to include color_scale:**

```rust
// src/gpu/buffers.rs - TrajectoryParams
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TrajectoryParams {
    pub width: u32,
    pub height: u32,
    pub num_transforms: u32,
    pub color_mode: u32,
    pub color_scale: f32,  // NEW: Global color scale (10-100)
    pub _padding: [f32; 3], // Maintain 16-byte alignment
    // ... rest of fields
}
```

**Shaders read from uniform:**
```wgsl
// shaders/core/main_2d.wgsl
let pixel_scale = params.color_scale;  // Was: f32(scale_buffer[pixel_idx])
```

**Benefits:**
- Remove 1.9 MB storage buffer
- ~100× faster reads (uniform vs storage)
- Simpler code (no buffer management)
- Still supports runtime adjustment via UI

## Cleanup Tasks

### 1. Remove Scale Buffer Infrastructure

**Files to modify:**

**src/gpu/buffers.rs:**
- ❌ Remove `scale_buffer: Buffer` field
- ❌ Remove scale buffer creation (~line 407-417)
- ❌ Remove `reset_scale_buffer()` function (~line 624-630)
- ✅ Add `color_scale: f32` to TrajectoryParams

**src/gpu/pipelines.rs:**
- ❌ Remove binding 6 from compute_bind_group_layout (~line 100-110)
- ❌ Remove binding 4 from accumulate_bind_group_layout (~line 218-228)
- ❌ Remove scale_buffer from compute bind group (~line 408)
- ❌ Remove scale_buffer from accumulate bind group (~line 437)

**shaders/core/header.wgsl:**
- ❌ Remove `@binding(6) scale_buffer` declaration (~line 67)
- ✅ Add `color_scale: f32` to TrajectoryParams struct

**shaders/core/main_2d.wgsl:**
- ✅ Change `let pixel_scale = f32(scale_buffer[pixel_idx]);` to `let pixel_scale = params.color_scale;`

**shaders/core/main_3d.wgsl:**
- ✅ Change `let pixel_scale = f32(scale_buffer[pixel_idx]);` to `let pixel_scale = params.color_scale;`

**shaders/accumulate.wgsl:**
- ❌ Remove `@binding(4) scale_buffer` declaration (~line 17)

**src/app/mod.rs:**
- ❌ Remove debug_scale_stats() call (~line 354-355)
- ❌ Remove `reset_scale_buffer()` calls (if any)

**src/renderer/compute_kernel.rs:**
- ❌ Remove `debug_scale_stats()` function (~line 271-322)
- ✅ Update TrajectoryParams initialization to include color_scale

### 2. Remove Adjust Scale Pipeline (Unused)

**Files to remove/modify:**

**shaders/adjust_scale.wgsl:**
- ❌ Delete entire file (never used, was for per-pixel adaptive)

**src/gpu/pipelines.rs:**
- ❌ Remove `adjust_scale_pipeline: ComputePipeline` field
- ❌ Remove `adjust_scale_bind_group_layout: BindGroupLayout` field
- ❌ Remove adjust_scale pipeline creation (~line 247-293)

**src/renderer/compute_kernel.rs:**
- ❌ Remove `adjust_scale_pass()` function (if exists and unused)

### 3. Add UI Control

**src/ui/mod.rs:**
- ✅ Add slider for color_scale (range 10-100, default 50)
- ✅ Return value in UiResponse
- ✅ Apply to params buffer in app.rs

**Suggested UI location:** Rendering Settings panel, near density_scale

```rust
ui.add(egui::Slider::new(&mut color_scale, 10.0..=100.0)
    .text("Color Scale")
    .tooltip("Higher = better precision, lower = prevent overflow (10-100)"));
```

## Expected Performance Recovery

### Bandwidth Reduction

**Before cleanup:**
- Total: ~9.6 MB per iteration

**After cleanup:**
- Histogram: 4× u32 = ~7.7 MB (unchanged, necessary)
- Scale buffer: REMOVED = -1.9 MB
- **New total: ~7.7 MB per iteration**

**Reduction: 20% less bandwidth**

### Access Pattern Improvement

**Before:** Every iteration reads storage buffer (100-400 cycles)
**After:** Every iteration reads uniform constant (1-4 cycles)
**Speedup: ~100× faster scale access**

### Expected Benchmark Result

**Realistic expectation:**
- Recover most of the regression vs commit 9ac278a
- May not be 100% due to 2× histogram size (unavoidable)
- Overall: Small regression acceptable for overflow fix

**If still slower:**
- Consider increasing batching factor (4× → 8×)
- Fewer accumulate passes may offset histogram size
- Test with benchmarks after cleanup

## Implementation Order

1. ✅ Document cleanup plan (this file)
2. ✅ Add color_scale to TrajectoryParams struct
3. ✅ Update shaders to read from params.color_scale
4. ✅ Remove scale_buffer from buffers.rs
5. ✅ Remove scale_buffer bindings from pipelines.rs
6. ✅ Remove debug_scale_stats() and reset_scale_buffer()
7. ✅ Remove adjust_scale pipeline and shader
8. ✅ Add UI slider for color_scale
9. ✅ Test and verify correctness
10. ✅ Run benchmarks to measure performance recovery

## Compatibility Notes

**Breaking changes:**
- Scale is now global (all pixels same scale)
- Per-pixel adaptive scaling no longer possible without re-adding infrastructure
- If we want adaptive scaling in future, need different approach

**Non-breaking:**
- UI can still adjust scale at runtime
- Scale can be saved in presets
- Same functional behavior (just global instead of per-pixel)

## Future Optimizations (After This Cleanup)

Once performance is recovered, consider:

1. **Increase batching factor** (4× → 8×)
   - Test if 2× histogram size is offset by 2× fewer accumulate passes
   - May actually improve performance despite larger histogram

2. **Increase default scale** (50 → 100)
   - Better color precision
   - No overflow risk with u32 histogram
   - Test quality improvement

3. **Test convergence masking** (revisit)
   - With u32 histogram, higher thresholds are safe
   - May still be useful for very long renders
   - Not urgent, overflow is already solved

## Success Criteria

**Must achieve:**
- ✅ Functionality unchanged (same visual output)
- ✅ Performance closer to commit 9ac278a baseline
- ✅ Code simpler (less infrastructure)

**Nice to have:**
- ✅ UI control for scale parameter
- ✅ Documentation of trade-offs
- ✅ Benchmark comparison data
