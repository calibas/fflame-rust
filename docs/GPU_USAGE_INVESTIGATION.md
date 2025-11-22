# GPU Usage Investigation

**Date Started:** 2025-11-22

## Problem Statement

GPU usage is **HIGHER when idle** and **LOWER when actively rendering** - opposite of expected behavior.

- **When rendering:** GPU usage lower, doing compute + accumulate + tonemap + egui
- **When idle:** GPU usage higher, only doing egui render
- **Disabling tonemap:** Makes no difference

## Observations

### Frame Rates
- **When rendering:** 60 × speed_multiplier FPS (can be 240-960 FPS)
- **When idle:** 60 FPS (no multiplier)

So idle actually has **lower frame rate**, yet **higher GPU usage**.

### Current Render Pipeline

**Active rendering (each frame):**
1. Compute pass - 128 workgroups × iterations_per_thread iterations
2. Accumulate pass - Blend new samples with history (every N frames)
3. Tonemap pass - Convert accumulation buffer to display
4. egui render - UI overlay

**Idle (each frame):**
1. (no compute)
2. (no accumulate)
3. Tonemap pass - Still runs every frame
4. egui render - UI overlay

## Hypotheses

### Hypothesis 1: egui requesting excessive repaints
- egui has hover effects, animations, blinking cursors
- May be requesting repaints continuously
- When rendering active: repaints absorbed into high frame rate
- When idle: repaints are the ONLY thing driving redraw events
- **Test:** Add logging to track egui repaint requests

### Hypothesis 2: GPU context switching overhead
- When idle: GPU switches between idle → UI render → idle
- When rendering: GPU pipelines compute + accumulate + tonemap + UI efficiently
- Frequent context switches may show as higher "usage" in metrics
- **Test:** Profile actual GPU work vs idle time

### Hypothesis 3: Batch submission efficiency
- All rendering submitted in single `queue.submit()` call
- GPU may be more efficient with larger batches
- Idle egui-only renders are smaller batches, processed less efficiently
- **Test:** Check if batching multiple frames helps

### Hypothesis 4: VSync/Present timing
- `frame.present()` called every frame regardless
- When idle: Present may be waiting more (VSync stalls)
- GPU metrics may include Present waiting time as "usage"
- **Test:** Check if Present timing differs between idle/active

## Investigation Steps

### Step 1: Add egui repaint logging ✅ (Completed, then reverted)
Added temporary TRACE logging to track:
- `ctx.has_requested_repaint()` check after each UI frame
- Frame interval timing with rendering_complete status

**Result:**
- egui was NOT requesting excessive repaints
- Found the real issue: texture bind group recreation every frame
- **Logging changes discarded** - no longer needed

### Step 2: Add frame metrics logging
Track per-frame:
- Compute time
- Accumulate time
- Tonemap time
- egui render time
- Present time
- Total frame time

### Step 3: Conditional tonemap (already tried)
- Added `rendering_complete` flag
- Can now skip tonemap when idle
- **Result:** TBD

## Code Changes

### Added rendering_complete flag (Commit b0c64a6)
```rust
pub(super) rendering_complete: bool,  // True when rendering has finished
```
- Set true: Frame after max_iterations reached
- Reset false: Whenever iterations reset (3 locations)

## Findings

### Test Results - TRACE logging (2025-11-22)

**Frame rate:** ~40 FPS (25ms intervals) when rendering_complete=false

**egui repaint requests:** NONE detected via `ctx.has_requested_repaint()`

**The Real Problem: Texture Recreation Every Frame**
```
[TRACE] BindGroup::drop Id(4,143)
[TRACE] Destroy raw BindGroup with 'egui_user_image_142' label
[TRACE] Destroy raw Sampler with 'egui_user_image_142' label
[TRACE] Device::create_sampler -> Id(1,144)
[TRACE] Device::create_bind_group -> Id(4,144)
```

**egui is destroying and recreating the fractal texture bind group EVERY FRAME!**

This explains the high GPU usage:
- Destroying samplers/bind groups each frame
- Recreating samplers/bind groups each frame
- Massive draw calls (31K+ indices, 10K+ indices per frame)
- ConfigSlider reading values every frame (minor issue, not cached)

**Root cause:** Likely in `EguiLayer::register_fractal_texture()` or how we're handling the fractal texture ID. Need to investigate why the texture is being re-registered every frame instead of cached.

## Root Cause Found

**Location:** `src/ui/mod.rs` lines 134-148 in `register_fractal_texture()`

**The problem:**
```rust
// ALWAYS update the texture view, even if size didn't change
// This is critical for minimize/restore - the texture view can become stale
if let Some(old_id) = self.fractal_texture_id.take() {
    if !needs_reregister {
        self.renderer.free_texture(&old_id);  // ← Frees every frame!
    }
}
let texture_id = self.renderer.register_native_texture(...);  // ← Creates new bind group every frame!
```

The code **unconditionally** frees and re-registers the texture every frame, even when nothing changed. This was added to fix minimize/restore issues, but causes massive overhead.

**Called from:** `src/app/mod.rs` line 311 - inside the render loop (every frame)

## Fix Attempt #1: Only register texture when size changes

**Change:** Moved closing bracket in `register_fractal_texture()` to only register when `needs_reregister == true`

**Result:** ❌ GPU usage still increases AFTER rendering finishes - this was NOT the root cause!

## Test Results - Idle State (2025-11-22)

**Re-tested egui repaint hypothesis specifically when IDLE** (rendering_complete=true):

**Frame intervals:**
```
Frame interval: 16.667ms (rendering_complete=true)
Frame interval: 16.667ms (rendering_complete=true)
Frame interval: 17.000ms (rendering_complete=true)
```

**Observations:**
- Frame rate: Steady 60 FPS (~16-17ms intervals)
- egui repaint requests: **ZERO** detected via `ctx.has_requested_repaint()`
- No TRACE logs showing texture bind group recreation
- GPU usage still observed to be higher than during active rendering

**Conclusion:** egui is NOT requesting excessive repaints when idle. Hypothesis 1 **ELIMINATED**.

The texture registration fix from earlier tests appears to have worked (no bind group recreation in TRACE logs). The GPU usage mystery likely stems from **metrics interpretation** rather than actual wasted work.

## Revised Theory: Metrics Interpretation Issue

The GPU usage increase when idle may not be about wasted work, but about **how GPU metrics are calculated**:

**When rendering active:**
- GPU doing: Massive compute work (128 workgroups × iterations) + accumulate + tonemap + egui
- egui is a SMALL percentage of total GPU work
- Total GPU time: High, egui time: Low percentage

**When rendering stops:**
- GPU doing: ONLY tonemap + egui rendering
- Same egui work, but now represents larger percentage of total
- GPU usage metrics may show higher percentage (even though absolute work is less)

**Alternative theory: GPU power/clock management**
- When rendering: GPU clocks stay high for sustained compute load
- When idle: GPU may throttle down, then boost for UI frames
- Power state transitions and clock changes show up as "higher usage"

**Alternative theory: Frame pacing/VSync**
- When rendering: 60-960 FPS, VSync may be disabled or ignored
- When idle: 60 FPS locked to VSync, GPU waiting for vertical blank
- VSync wait time may be counted as "GPU usage" by monitoring tools

## Next Investigation Steps

1. Check absolute GPU time (ms) vs percentage metrics
2. Monitor GPU clocks/power states during rendering vs idle
3. Check if VSync/present timing differs between states
4. Profile with GPU profiling tools (RenderDoc, Nsight, etc.)

## Notes

- Performance metrics already tracked in `PerformanceMetrics`
- Frame timing already measured (compute, accumulate, tonemap, submit, present)
- May need to add egui-specific metrics
