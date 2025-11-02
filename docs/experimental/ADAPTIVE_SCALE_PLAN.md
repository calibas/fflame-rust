# Adaptive Histogram Scale - Implementation Plan

**Date:** 2025-10-26
**Status:** Proposed
**Branch:** experiment/batched-accumulation

---

## Problem Statement

With u16 packed histogram, there's a fundamental trade-off:
- **High scale (100):** Good precision, but overflows at 655 hits
- **Low scale (10):** No overflow, but severe color quantization

**Key insight:** Bit-packing ANY integer type has this problem. u8, u16, u32 - all overflow into adjacent channels.

---

## The Adaptive Solution

Dynamically adjust `histogram_color_scale` based on observed histogram values.

### Core Concept

```rust
// After each accumulation batch
let max_channel = scan_histogram_max();

if max_channel > OVERFLOW_THRESHOLD {
    // Risk of overflow - reduce precision
    histogram_color_scale *= 0.5;
} else if max_channel < UNDERFLOW_THRESHOLD {
    // Lots of headroom - increase precision
    histogram_color_scale = min(histogram_color_scale * 1.5, MAX_SCALE);
}
```

### Why This Works

1. **Starts with high quality** - Begin at scale=100 (100 color levels)
2. **Adapts to scene** - Zoomed out → auto-reduces, zoomed in → maintains precision
3. **Fits batched architecture** - Scale adjustment happens between batches (no mid-batch disruption)
4. **User still controls** - Manual slider overrides adaptive behavior

---

## Implementation Details

### 1. Histogram Max Scan (GPU)

**New Compute Shader:** `shaders/histogram_reduce.wgsl`

```wgsl
@group(0) @binding(0) var<storage, read> histogram: array<u32>;
@group(0) @binding(1) var<storage, read_write> max_values: array<atomic<u32>, 3>; // R, G, B max

@compute @workgroup_size(256, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let idx = global_id.x;
    let pixel_count = params.width * params.height;

    if (idx >= pixel_count) {
        return;
    }

    let base_idx = idx * 2u;
    let packed_rg = histogram[base_idx + 0u];
    let packed_bd = histogram[base_idx + 1u];

    // Unpack
    let r = packed_rg & 0xFFFFu;
    let g = (packed_rg >> 16u) & 0xFFFFu;
    let b = packed_bd & 0xFFFFu;

    // Track max via atomics
    atomicMax(&max_values[0], r);
    atomicMax(&max_values[1], g);
    atomicMax(&max_values[2], b);
}
```

**Performance:** Single pass, parallel reduction via atomicMax. Very cheap (<1ms @ 1080p).

### 2. Scale Adjustment Logic (CPU)

**Location:** `src/renderer/compute_kernel.rs`

```rust
pub struct FlameRenderer {
    // ...
    histogram_color_scale: f32,
    adaptive_scale_enabled: bool,
    max_values_buffer: Buffer,  // NEW: 3× u32 for R,G,B max
    // ...
}

impl FlameRenderer {
    pub fn adaptive_scale_adjustment(&mut self, queue: &Queue) {
        if !self.adaptive_scale_enabled {
            return;  // User disabled adaptive scaling
        }

        // Read max values from GPU
        let max_values = self.read_max_values(queue);
        let max_channel = max_values.iter().max().unwrap_or(&0);

        const OVERFLOW_THRESHOLD: u32 = 60000;   // 91% of u16 max
        const UNDERFLOW_THRESHOLD: u32 = 15000;  // 23% of u16 max

        let old_scale = self.histogram_color_scale;

        if *max_channel > OVERFLOW_THRESHOLD {
            // Reduce scale to prevent overflow
            self.histogram_color_scale *= 0.5;
            self.histogram_color_scale = self.histogram_color_scale.max(1.0);
            log::info!("Adaptive scale: {} → {} (overflow risk)", old_scale, self.histogram_color_scale);
        } else if *max_channel < UNDERFLOW_THRESHOLD && self.histogram_color_scale < 100.0 {
            // Increase scale for better precision
            self.histogram_color_scale *= 1.5;
            self.histogram_color_scale = self.histogram_color_scale.min(100.0);
            log::info!("Adaptive scale: {} → {} (increasing precision)", old_scale, self.histogram_color_scale);
        }

        // If scale changed, we need to clear histogram and restart batch
        if (self.histogram_color_scale - old_scale).abs() > 0.01 {
            self.reset_accumulation();
        }
    }
}
```

### 3. Integration with Batched Accumulation

**Location:** `src/app/mod.rs` render loop

```rust
// After compute pass, before accumulation
if batch_frame_count == accumulation_batch_size {
    // Run histogram max scan
    renderer.scan_histogram_max(&device, &mut encoder);

    // Adjust scale if needed
    renderer.adaptive_scale_adjustment(&queue);

    // Accumulate as normal
    renderer.accumulate(&device, &mut encoder, &queue, ...);

    batch_frame_count = 0;
}
```

### 4. UI Controls

**Location:** `src/ui/settings.rs`

```rust
// New checkbox
ui.checkbox(&mut adaptive_scale_enabled, "Adaptive Scale");
if ui.is_item_hovered() {
    ui.tooltip_text("Automatically adjust color precision based on scene density");
}

// Existing slider (grayed out if adaptive enabled)
ui.begin_disabled(adaptive_scale_enabled);
ui.slider_float("Histogram Color Scale", &mut histogram_color_scale, 1.0, 100.0);
ui.end_disabled();
```

---

## Performance Analysis

### Additional Cost

1. **Histogram Max Scan:**
   - One compute dispatch per batch (not per frame!)
   - 1920×1080 = 2,073,600 pixels
   - 256 threads per workgroup = ~8,100 workgroups
   - Parallel atomicMax: ~0.5ms @ 1080p

2. **Buffer Readback:**
   - 3× u32 (12 bytes) from GPU to CPU
   - Async read, doesn't block rendering
   - Negligible cost

**Total overhead:** <1ms per batch (every 4 frames) = 0.25ms per frame average

### Benefit

- Starts with high quality (scale=100)
- Auto-protects from overflow
- No user tuning required
- Better than fixed scale in all cases

---

## Edge Cases and Handling

### Case 1: Rapid Scale Changes

**Problem:** Zooming out quickly causes rapid scale reductions

**Solution:** Rate-limit scale changes
```rust
const MIN_FRAMES_BETWEEN_CHANGES: u32 = 4;
if frames_since_last_scale_change < MIN_FRAMES_BETWEEN_CHANGES {
    return;  // Skip adjustment
}
```

### Case 2: Scale Oscillation

**Problem:** Scale bounces between two values

**Solution:** Hysteresis
```rust
const OVERFLOW_THRESHOLD_HIGH: u32 = 60000;
const OVERFLOW_THRESHOLD_LOW: u32 = 50000;

if *max_channel > OVERFLOW_THRESHOLD_HIGH {
    scale_down();
} else if *max_channel < OVERFLOW_THRESHOLD_LOW && scale_recently_reduced {
    // Don't scale back up immediately
}
```

### Case 3: User Override

**Problem:** User manually adjusts scale, but adaptive keeps changing it

**Solution:** Disable adaptive when user moves slider
```rust
if ui.slider_changed("histogram_color_scale") {
    adaptive_scale_enabled = false;  // User took control
}
```

---

## Testing Plan

### 1. Visual Quality Tests

**Scenarios:**
- Zoom out gradually (watch scale reduce)
- Zoom in gradually (watch scale increase)
- Sudden zoom changes (test rate limiting)
- Dense fractals (test overflow prevention)
- Sparse fractals (test precision maximization)

### 2. Performance Benchmarks

**Measure:**
- Histogram scan overhead (should be <1ms)
- Frame time impact (should be <5%)
- Buffer readback latency

### 3. Scale Transition Tests

**Check:**
- No visible artifacts when scale changes
- Smooth quality transitions
- No flickering or popping

---

## Alternative Approaches (Considered and Rejected)

### Alt 1: Per-Frame Adaptation
**Idea:** Adjust scale every frame, not just per batch

**Rejected because:**
- Adds overhead every frame (not just per batch)
- More scale transitions = more visual disruption
- Batched architecture is optimized for fewer passes

### Alt 2: Separate u32 Per Channel
**Idea:** Use 4× u32 histogram (no packing)

**Rejected because:**
- Loses 13.8% performance gain from packing
- 2× memory usage
- Adaptive scaling solves the problem without performance loss

### Alt 3: Saturation Instead of Wrapping
**Idea:** Clamp channels at 65535 instead of wrapping

**Rejected because:**
- Requires compare-and-swap loop (much slower)
- Still loses precision when saturated
- Adaptive scaling prevents saturation entirely

---

## Configuration Options

### Defaults (Recommended)

```rust
// src/config.rs
fn default_histogram_color_scale() -> f32 {
    100.0  // Start high for quality
}

fn default_adaptive_scale_enabled() -> bool {
    true  // Enable by default
}

fn default_low_density_smoothing() -> f32 {
    0.0  // Pure blending by default
}
```

### User Profiles

**Quality Mode:**
- adaptive_scale: enabled
- histogram_color_scale: 100 (initial)
- low_density_smoothing: 0.0

**Robust Mode:**
- adaptive_scale: disabled
- histogram_color_scale: 10
- low_density_smoothing: 0.5

**Balanced Mode:**
- adaptive_scale: enabled
- histogram_color_scale: 50 (initial)
- low_density_smoothing: 0.25

---

## Implementation Steps

### Phase 1: Histogram Scan (MVP)
1. ✅ Create `shaders/histogram_reduce.wgsl`
2. ✅ Add max_values_buffer to FlameRenderer
3. ✅ Implement scan_histogram_max() dispatch
4. ✅ Test: Verify max values are correct

### Phase 2: Scale Adjustment
1. ✅ Implement adaptive_scale_adjustment() logic
2. ✅ Add thresholds and hysteresis
3. ✅ Integrate with render loop
4. ✅ Test: Watch scale adjust with zoom changes

### Phase 3: UI Integration
1. ✅ Add "Adaptive Scale" checkbox
2. ✅ Gray out slider when adaptive enabled
3. ✅ Add tooltips explaining behavior
4. ✅ Test: User can enable/disable

### Phase 4: Polish
1. ✅ Add rate limiting
2. ✅ Smooth scale transitions
3. ✅ Add logging for debugging
4. ✅ Performance profiling

---

## Success Criteria

**Must achieve:**
- ✅ No visible overflow artifacts in any scene
- ✅ High quality (scale≥50) maintained when possible
- ✅ Performance overhead <5%
- ✅ User can disable if desired

**Nice to have:**
- 📊 UI indicator showing current scale
- 📊 Visual feedback when scale changes
- 📊 Statistics (scale history, overflow events)
- 📊 Export includes adaptive scale events

---

## Open Questions

1. **Should scale changes be logged to console?**
   - Pro: Helps debug, user can see what's happening
   - Con: Might be noisy

2. **Should we smooth scale transitions?**
   - Pro: Avoids sudden quality changes
   - Con: Adds complexity, may delay overflow protection

3. **What's the ideal OVERFLOW_THRESHOLD?**
   - Too low (50k): Changes scale unnecessarily
   - Too high (62k): Cuts it too close
   - Current proposal: 60k (91% of max)

4. **Should manual slider override persist?**
   - Option A: Once user touches slider, adaptive stays off forever
   - Option B: Reset to adaptive on preset load
   - Current proposal: Option B

---

## Conclusion

Adaptive scaling is the **ideal solution** for the histogram overflow vs precision trade-off:

**Advantages:**
- ✅ Solves overflow problem automatically
- ✅ Maintains high quality when possible
- ✅ Fits perfectly with batched accumulation
- ✅ Minimal performance cost (<1ms per batch)
- ✅ User retains manual control
- ✅ No code changes to existing pipeline

**vs. Other Options:**
- Better than fixed scale (adapts to scene)
- Better than u8 packing (same overflow issue)
- Better than 4 atomics (keeps performance gain)
- Better than saturation (prevents loss, not just limits it)

**Recommendation:** Implement adaptive scaling as the long-term solution. Update defaults to scale=100, smoothing=0.0 in the short-term.

---

## References

- [HISTOGRAM_INVESTIGATION_SUMMARY.md](HISTOGRAM_INVESTIGATION_SUMMARY.md) - Investigation findings
- [HISTOGRAM_EVOLUTION.md](HISTOGRAM_EVOLUTION.md) - Algorithm history
- [FINDINGS_PRESENTATION.md](FINDINGS_PRESENTATION.md) - Visual presentation
