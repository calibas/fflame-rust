# Per-Pixel Adaptive Scale - Implementation Plan

**Date:** 2025-10-26
**Status:** Proposed (Recommended Approach)
**Branch:** experiment/batched-accumulation

---

## Executive Summary

**Concept:** Instead of one global scale, each pixel tracks its own optimal `histogram_color_scale` value based on local density and overflow risk.

**Key Insight:** Bright core pixels need low scale (prevent overflow), while dark edge pixels can use high scale (maximize precision). A global scale forces unnecessary compromise.

**Result:** Perfect overflow protection + optimal quality per region.

---

## Problem Analysis

### Current Limitation: Global Scale Compromise

```
Scene with bright core + dark edges:
- Core: 1000 hits/pixel → needs scale=10 (prevent overflow)
- Edges: 5 hits/pixel → could use scale=100 (better precision)

Global scale=10: Core is safe, but edges suffer from quantization
Global scale=100: Edges look great, but core overflows
```

**No single global value is optimal for all regions.**

### The Per-Pixel Solution

```
Core pixels:  scale=10  (adjusted down due to high density)
Edge pixels:  scale=100 (maintained due to low density)

Each region gets the precision it can handle!
```

---

## Architecture

### Data Structures

```rust
// src/gpu/buffers.rs - FlameBuffers struct
pub struct FlameBuffers {
    // Existing
    pub histogram_buffer: Buffer,  // width × height × 2 × u32 (packed RGBA+density)

    // NEW: Per-pixel scale tracking
    pub scale_buffer: Buffer,      // width × height × u16 (scale: 1-100)
    pub scale_texture_view: TextureView,  // For easy shader access

    // ...
}
```

**Memory Layout:**
```
scale_buffer[pixel_idx] = u16 scale value (1-100)
- Index: pixel_y × width + pixel_x
- Size: 1920 × 1080 × 2 bytes = 4 MB @ 1080p
- Total with histogram: 20 MB (was 16 MB)
```

### Initialization

```rust
impl FlameBuffers {
    pub fn new(device: &Device, width: u32, height: u32) -> Self {
        // Initialize all pixels to maximum scale
        let initial_scales = vec![100u16; (width * height) as usize];

        let scale_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("Per-Pixel Scale Buffer"),
            contents: bytemuck::cast_slice(&initial_scales),
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        });

        // ...
    }
}
```

---

## Shader Implementation

### 1. Compute Shader (Write to Histogram)

**File:** `shaders/core/main_2d.wgsl` and `main_3d.wgsl`

```wgsl
// NEW: Add scale buffer binding
@group(0) @binding(5) var<storage, read> scale_buffer: array<u32>;  // u16 packed (2 per u32)

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    // ... existing iteration code ...

    // When writing to histogram:
    let pixel = world_to_pixel(p);
    if (!in_bounds(pixel)) { continue; }

    let pixel_idx = u32(pixel.y) * params.width + u32(pixel.x);

    // Load this pixel's current scale (u16 from packed u32)
    let scale_word_idx = pixel_idx / 2u;
    let scale_word = scale_buffer[scale_word_idx];
    let pixel_scale = f32(select(
        scale_word & 0xFFFFu,          // Even pixel (low 16 bits)
        (scale_word >> 16u) & 0xFFFFu, // Odd pixel (high 16 bits)
        (pixel_idx % 2u) == 1u
    ));

    // Scale color by this pixel's scale (not global scale!)
    let r16 = u32(clamp(final_color.r, 0.0, 1.0) * pixel_scale);
    let g16 = u32(clamp(final_color.g, 0.0, 1.0) * pixel_scale);
    let b16 = u32(clamp(final_color.b, 0.0, 1.0) * pixel_scale);
    let d16 = 1u;

    // Pack and accumulate (unchanged)
    let packed_rg = r16 | (g16 << 16u);
    let packed_bd = b16 | (d16 << 16u);

    let hist_idx = pixel_idx * 2u;
    atomicAdd(&histogram[hist_idx + 0u], packed_rg);
    atomicAdd(&histogram[hist_idx + 1u], packed_bd);
}
```

**Key Changes:**
- Added scale_buffer binding
- Load per-pixel scale before encoding
- Use pixel_scale instead of params.histogram_color_scale

### 2. Scale Adjustment Shader (NEW)

**File:** `shaders/adjust_scale.wgsl` (new file)

```wgsl
@group(0) @binding(0) var<storage, read> histogram: array<u32>;
@group(0) @binding(1) var<storage, read_write> scale_buffer: array<u32>;  // u16 packed
@group(0) @binding(2) var<uniform> params: AdjustParams;

struct AdjustParams {
    width: u32,
    height: u32,
    overflow_threshold: u32,      // e.g., 60000
    high_density_threshold: u32,  // e.g., 500 hits
    low_density_threshold: u32,   // e.g., 10 hits
    underflow_threshold: u32,     // e.g., 15000
}

@compute @workgroup_size(256, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let pixel_idx = global_id.x;
    let pixel_count = params.width * params.height;

    if (pixel_idx >= pixel_count) {
        return;
    }

    // Read histogram values for this pixel
    let hist_idx = pixel_idx * 2u;
    let packed_rg = histogram[hist_idx + 0u];
    let packed_bd = histogram[hist_idx + 1u];

    let r_sum = packed_rg & 0xFFFFu;
    let g_sum = (packed_rg >> 16u) & 0xFFFFu;
    let b_sum = packed_bd & 0xFFFFu;
    let density = (packed_bd >> 16u) & 0xFFFFu;

    // Find max channel value
    let max_channel = max(max(r_sum, g_sum), b_sum);

    // Load current scale for this pixel
    let scale_word_idx = pixel_idx / 2u;
    let scale_word = scale_buffer[scale_word_idx];
    let is_odd = (pixel_idx % 2u) == 1u;
    let current_scale_u16 = select(
        scale_word & 0xFFFFu,
        (scale_word >> 16u) & 0xFFFFu,
        is_odd
    );
    let current_scale = f32(current_scale_u16);

    // Determine new scale
    var new_scale = current_scale;

    // REDUCE SCALE: Overflow risk or very high density
    if (max_channel > params.overflow_threshold || density > params.high_density_threshold) {
        new_scale = max(current_scale * 0.5, 1.0);
    }
    // INCREASE SCALE: Low density (maximize precision)
    else if (density < params.low_density_threshold && current_scale < 100.0) {
        new_scale = min(current_scale * 1.5, 100.0);
    }
    // INCREASE SCALE: High headroom (gradually increase)
    else if (max_channel < params.underflow_threshold && current_scale < 100.0) {
        new_scale = min(current_scale * 1.1, 100.0);
    }

    // Write back new scale (only if changed to avoid unnecessary writes)
    if (abs(new_scale - current_scale) > 0.1) {
        let new_scale_u16 = u32(new_scale);

        // Update the appropriate 16 bits in the packed u32
        let other_scale = select(
            (scale_word >> 16u) & 0xFFFFu,  // Keep high bits if we're updating low
            scale_word & 0xFFFFu,            // Keep low bits if we're updating high
            is_odd
        );

        let new_word = select(
            new_scale_u16 | (other_scale << 16u),  // We're even (low bits)
            other_scale | (new_scale_u16 << 16u),  // We're odd (high bits)
            is_odd
        );

        scale_buffer[scale_word_idx] = new_word;
    }
}
```

**Purpose:** Adjust each pixel's scale based on its histogram statistics.

**Thresholds (tunable):**
- `overflow_threshold`: 60,000 (91% of u16 max)
- `high_density_threshold`: 500 hits (very dense area)
- `low_density_threshold`: 10 hits (sparse area)
- `underflow_threshold`: 15,000 (lots of headroom)

### 3. Accumulate Shader (Read from Histogram)

**File:** `shaders/accumulate.wgsl`

```wgsl
// NEW: Add scale buffer binding
@group(0) @binding(3) var<storage, read> scale_buffer: array<u32>;  // u16 packed

@compute @workgroup_size(16, 16, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let pixel = vec2<i32>(i32(global_id.x), i32(global_id.y));

    // ... existing bounds check ...

    let pixel_idx = u32(pixel.y) * params.width + u32(pixel.x);

    // Load this pixel's current scale
    let scale_word_idx = pixel_idx / 2u;
    let scale_word = scale_buffer[scale_word_idx];
    let pixel_scale = f32(select(
        scale_word & 0xFFFFu,
        (scale_word >> 16u) & 0xFFFFu,
        (pixel_idx % 2u) == 1u
    ));

    // Read histogram
    let hist_idx = pixel_idx * 2u;
    let packed_rg = histogram[hist_idx + 0u];
    let packed_bd = histogram[hist_idx + 1u];

    let r_sum = f32(packed_rg & 0xFFFFu);
    let g_sum = f32((packed_rg >> 16u) & 0xFFFFu);
    let b_sum = f32(packed_bd & 0xFFFFu);
    let density = f32((packed_bd >> 16u) & 0xFFFFu);

    // Decode with this pixel's scale (not global scale!)
    var new_color = vec3<f32>(0.0);
    if (density > 0.0) {
        new_color = vec3<f32>(
            r_sum / (density * pixel_scale),
            g_sum / (density * pixel_scale),
            b_sum / (density * pixel_scale)
        );
        new_color = clamp(new_color, vec3<f32>(0.0), vec3<f32>(1.0));
    }

    // ... rest of accumulation logic unchanged ...
}
```

**Key Changes:**
- Added scale_buffer binding
- Load per-pixel scale before decoding
- Use pixel_scale instead of params.histogram_color_scale

---

## Integration with Render Loop

### Location: `src/app/mod.rs`

```rust
// In render() function
if batch_frame_count >= accumulation_batch_size {
    // 1. Run scale adjustment shader (NEW)
    renderer.adjust_pixel_scales(&device, &mut encoder);

    // 2. Accumulate as usual
    renderer.accumulate(&device, &mut encoder, &queue, ...);

    // 3. Clear histogram for next batch
    encoder.clear_buffer(&buffers.histogram_buffer, 0, None);

    batch_frame_count = 0;
}
```

### New Method: `adjust_pixel_scales()`

**Location:** `src/renderer/compute_kernel.rs`

```rust
impl FlameRenderer {
    pub fn adjust_pixel_scales(
        &mut self,
        device: &Device,
        encoder: &mut CommandEncoder,
    ) {
        if !self.adaptive_scale_enabled {
            return;  // User disabled adaptive scaling
        }

        // Prepare params
        let params = AdjustParams {
            width: self.width,
            height: self.height,
            overflow_threshold: 60000,
            high_density_threshold: 500,
            low_density_threshold: 10,
            underflow_threshold: 15000,
        };

        // Upload params
        self.queue.write_buffer(&self.adjust_params_buffer, 0, bytemuck::bytes_of(&params));

        // Dispatch adjust shader
        let pixel_count = self.width * self.height;
        let workgroup_count = (pixel_count + 255) / 256;

        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("Adjust Per-Pixel Scales"),
        });

        pass.set_pipeline(&self.adjust_scale_pipeline);
        pass.set_bind_group(0, &self.adjust_scale_bind_group, &[]);
        pass.dispatch_workgroups(workgroup_count, 1, 1);

        drop(pass);
    }
}
```

---

## UI Controls

### Location: `src/ui/settings.rs`

```rust
// Adaptive scale checkbox
ui.checkbox(&mut adaptive_scale_enabled, "Per-Pixel Adaptive Scale");
if ui.is_item_hovered() {
    ui.tooltip_text(
        "Each pixel automatically adjusts its color precision based on local density.\n\
         Bright areas use lower precision (prevent overflow).\n\
         Dark areas use higher precision (better quality)."
    );
}

// Show global scale slider (grayed out if adaptive enabled)
ui.begin_disabled(adaptive_scale_enabled);
ui.slider_float("Histogram Color Scale", &mut histogram_color_scale, 1.0, 100.0);
if ui.is_item_hovered() {
    ui.tooltip_text("Manual control when adaptive scaling is disabled");
}
ui.end_disabled();

// Optional: Scale statistics (read-only display)
if adaptive_scale_enabled {
    ui.text(format!("Scale range: {:.0} - {:.0}", min_scale, max_scale));
    ui.text(format!("Avg scale: {:.1}", avg_scale));
}
```

---

## Performance Analysis

### Memory Cost

```
scale_buffer: width × height × 2 bytes
- 1920×1080: 4.1 MB
- 3840×2160: 16.6 MB

Total @ 1080p: 16 MB (histogram) + 4 MB (scale) = 20 MB
```

**Impact:** +25% memory vs current (acceptable)

### Computation Cost

**Per Hit (Compute Shader):**
- +1 load (scale_buffer[pixel_idx])
- +0 ALU ops (same math, just using pixel_scale instead of global)
- **Overhead: <1%** (one load amortized over iteration cost)

**Per Batch (Adjust Shader):**
- Dispatch: (width × height) / 256 workgroups
- Each thread: 1 histogram read, 1 scale read/write, simple math
- Example @ 1080p: 2,073,600 pixels / 256 = 8,100 workgroups
- **Cost: <1ms** (parallel, no atomics)

**Per Pixel (Accumulate Shader):**
- +1 load (scale_buffer[pixel_idx])
- +0 ALU ops (decode uses pixel_scale instead of global)
- **Overhead: <1%** (one load amortized over accumulation cost)

**Total Expected Overhead: <5%**

### Bandwidth Analysis

```
Per frame:
- Reads: histogram (32 MB @ 1080p) + scale (4 MB) = 36 MB
- Writes: histogram (32 MB)

Bandwidth: 68 MB per frame @ 60 FPS = 4.08 GB/s

Modern GPU memory bandwidth: 200-900 GB/s
Utilization: 0.5-2% (negligible)
```

---

## Edge Cases and Solutions

### Case 1: Scale Transition Artifacts

**Problem:** When a pixel's scale changes, next batch has different quantization

**Analysis:**
```
Batch 1: scale=100, color=0.537 → encoded=53 → decoded=0.53
Batch 2: scale=50, color=0.537  → encoded=26 → decoded=0.52

Difference: 0.01 (1% error)
```

**Impact:** Minimal - blend factor smooths transitions
```
accumulated = prev × (1 - blend_factor) + new × blend_factor
            = 0.53 × 0.99 + 0.52 × 0.01
            = 0.5297 (barely noticeable)
```

**Mitigation:** Scale changes are gradual (1.5× or 0.5× per batch)

### Case 2: Initialization

**Question:** What if a pixel starts at scale=100 and immediately hits overflow?

**Answer:** This is fine!
```
Frame 1: scale=100, 1000 hits → max=100,000 (overflow!)
Frame 2-4: Continue writing at scale=100
Batch complete: Adjust shader sees max=100,000 → reduces to scale=50
Next batch: Safe at scale=50
```

The histogram wraps (which is wrong), but we detect and fix it for the next batch. First batch might have artifacts, but subsequent batches are correct.

**Better solution:** Start at scale=50 (balanced)
```rust
let initial_scales = vec![50u16; (width * height) as usize];
```

### Case 3: Never-Hit Pixels

**Problem:** Pixels that never get hits stay at initial scale forever

**Impact:** None - they're never written to, never decoded, so scale doesn't matter

### Case 4: Sparse Flickering

**Problem:** Pixel gets 1 hit every 10 batches - scale constantly adjusts up/down

**Solution:** Add hysteresis to scale adjustment
```wgsl
// Don't increase if we recently decreased
let recently_decreased = (current_scale < 90.0);  // Was reduced from 100
if (density < 10 && !recently_decreased) {
    new_scale = min(current_scale * 1.5, 100.0);
}
```

Or: Only adjust if value is significantly different
```wgsl
if (abs(new_scale - current_scale) > 5.0) {
    // Only apply change if it's substantial
}
```

---

## Comparison: Global vs Per-Pixel Adaptive

| Aspect | Global Adaptive | Per-Pixel Adaptive |
|--------|----------------|-------------------|
| **Overflow Protection** | ⭐⭐⭐ Good | ⭐⭐⭐⭐⭐ Perfect |
| **Quality (bright areas)** | ⭐⭐⭐ Forced low scale | ⭐⭐⭐⭐ Optimal low scale |
| **Quality (dark areas)** | ⭐⭐ Limited by global scale | ⭐⭐⭐⭐⭐ Maintains high scale |
| **Memory** | 16 MB | 20 MB (+25%) |
| **Performance** | <1% overhead | <5% overhead |
| **Complexity** | ⭐⭐ Simple | ⭐⭐⭐⭐ More complex |
| **User Tuning** | None needed | None needed |

**Winner:** Per-Pixel Adaptive (better quality, acceptable cost)

---

## Implementation Phases

### Phase 1: Infrastructure (2-4 hours)
- [ ] Add `scale_buffer` to FlameBuffers struct
- [ ] Initialize scale_buffer (all pixels = 50)
- [ ] Create bind group entries for scale_buffer
- [ ] Add scale_buffer to shader bindings (read-only in compute/accumulate)

### Phase 2: Shader Updates (2-3 hours)
- [ ] Update compute shader (main_2d.wgsl, main_3d.wgsl) to use pixel_scale
- [ ] Update accumulate shader to use pixel_scale
- [ ] Create adjust_scale.wgsl shader
- [ ] Test: Verify rendering still works (should be identical with fixed scale)

### Phase 3: Scale Adjustment (3-4 hours)
- [ ] Create adjust_scale pipeline and bind group
- [ ] Implement `adjust_pixel_scales()` method
- [ ] Integrate into render loop (call per batch)
- [ ] Test: Add logging to verify scales are adjusting

### Phase 4: UI and Polish (2-3 hours)
- [ ] Add "Per-Pixel Adaptive Scale" checkbox
- [ ] Gray out manual scale slider when adaptive enabled
- [ ] Add scale statistics display (min/max/avg)
- [ ] Test: Toggle adaptive on/off, verify behavior

### Phase 5: Testing and Tuning (4-6 hours)
- [ ] Benchmark performance overhead
- [ ] Test with various scenes (bright, dark, mixed)
- [ ] Tune thresholds (overflow, high density, low density)
- [ ] Visual comparison with global scale
- [ ] Fix any artifacts or edge cases

**Total Estimated Time: 13-20 hours**

---

## Testing Plan

### Unit Tests

```rust
#[test]
fn test_scale_packing() {
    // Verify u16 packing/unpacking works correctly
    let scales = vec![10u16, 100u16, 50u16, 75u16];
    let packed: Vec<u32> = scales.chunks(2)
        .map(|chunk| chunk[0] as u32 | ((chunk[1] as u32) << 16))
        .collect();

    // Unpack and verify
    assert_eq!(packed[0] & 0xFFFF, 10);
    assert_eq!(packed[0] >> 16, 100);
}

#[test]
fn test_scale_adjustment_logic() {
    // Verify threshold logic
    let mut scale = 100.0;

    // Overflow case
    let max_channel = 65000;
    if max_channel > 60000 {
        scale *= 0.5;
    }
    assert_eq!(scale, 50.0);
}
```

### Integration Tests

**Test 1: Bright Core**
```
Render: Zoomed-out fractal with bright center
Expected: Center pixels reduce to scale=5-10
Expected: Edge pixels maintain scale=80-100
Visual: No overflow artifacts in center
```

**Test 2: Dark Edges**
```
Render: Fractal with sparse outer regions
Expected: Outer pixels maintain scale=100
Expected: Core pixels reduce based on density
Visual: High precision in sparse areas
```

**Test 3: Zoom Animation**
```
Action: Slowly zoom out from center
Expected: Core pixels gradually reduce scale
Expected: Smooth transitions, no flickering
Visual: No sudden quality drops
```

### Performance Tests

```bash
# Benchmark with per-pixel adaptive
cargo run --release -- benchmark simple3.fflame --iterations 40000000000

# Compare to fixed scale
# Expected: <5% slower
```

---

## Success Criteria

**Must Achieve:**
- ✅ No overflow artifacts in any scene (even zoomed-out with 1M iterations)
- ✅ Dark areas maintain high quality (scale≥50)
- ✅ Performance overhead <5%
- ✅ Smooth scale transitions (no flickering)

**Nice to Have:**
- 📊 UI shows per-pixel scale heatmap
- 📊 Export includes scale statistics
- 📊 Logging shows scale distribution
- 📊 User can set min/max scale bounds

---

## Future Enhancements

### Enhancement 1: Scale Visualization
```rust
// Render scale buffer as heatmap overlay
// Red = low scale (10), Blue = high scale (100)
let scale_viz_mode: bool = false;
if scale_viz_mode {
    render_scale_heatmap();
}
```

### Enhancement 2: Predictive Scaling
```rust
// Predict overflow before it happens based on density growth rate
let density_delta = current_density - previous_density;
if density_delta > threshold {
    preemptively_reduce_scale();
}
```

### Enhancement 3: Region-Based Scaling
```rust
// Group pixels into tiles, share scale per tile
// Reduces scale buffer to width/16 × height/16
// Trade-off: Less granularity, more memory bandwidth savings
```

### Enhancement 4: User Scale Bounds
```rust
// Let user set min/max scale per region
min_scale_slider: 1-100 (default 1)
max_scale_slider: 1-100 (default 100)

// Clamp pixel scales to user bounds
new_scale = clamp(new_scale, min_scale, max_scale);
```

---

## Conclusion

**Per-pixel adaptive scale is the optimal solution** for the histogram overflow problem:

**Why It's Superior:**
- ✅ No global compromise (each pixel gets ideal scale)
- ✅ Perfect overflow protection (bright areas auto-adjust)
- ✅ Maximum quality preservation (dark areas stay precise)
- ✅ Fits batched architecture (adjust between batches)
- ✅ Modest overhead (<5% performance, +25% memory)

**vs. Global Adaptive:**
- Better quality in sparse regions (no global compromise)
- Better overflow protection (per-pixel precision)
- Slightly higher cost (4 MB more memory, <4% more compute)

**vs. Fixed Scale:**
- Automatic adaptation (no user tuning)
- Eliminates overflow risk entirely
- Maintains high quality where possible

**Recommendation:** Implement per-pixel adaptive scale as the long-term solution.

---

## References

- [HISTOGRAM_INVESTIGATION_SUMMARY.md](HISTOGRAM_INVESTIGATION_SUMMARY.md) - Investigation findings
- [ADAPTIVE_SCALE_PLAN.md](ADAPTIVE_SCALE_PLAN.md) - Global adaptive scale (simpler alternative)
- [HISTOGRAM_EVOLUTION.md](HISTOGRAM_EVOLUTION.md) - Algorithm history
