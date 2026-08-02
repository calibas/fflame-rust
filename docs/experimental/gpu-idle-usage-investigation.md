# GPU Idle Usage Investigation

**Date:** 2025-10-30
**Status:** Investigated, Root Cause Identified
**Priority:** Low (Performance acceptable, optimization deferred)

## Problem Statement

GPU usage remains at ~20% when the fractal is finished rendering (paused or max iterations reached), even though no new samples are being computed. The app runs at 60 FPS continuously, which may be wasteful when idle.

## Investigation Summary

### What We Tested

We systematically disabled components to isolate the GPU usage source:

1. **Tonemap pass disabled** → GPU usage unchanged (~20%)
2. **UI rendering disabled** → GPU usage unchanged (~20%)
3. **Compute/accumulate passes disabled** → GPU usage unchanged (~20%)
4. **All GPU operations disabled** → GPU usage unchanged (~20%)
5. **Entire render() function disabled** → **GPU usage dropped to 0%** (but CPU usage spiked due to unbounded event loop)

### Key Findings

**The GPU usage is NOT from any specific shader or pass we can isolate.** Even with all of the following disabled:
- Compute pass (fractal iteration shader)
- Accumulate pass (sample blending shader)
- Tonemap pass (display tone mapping shader)
- UI rendering (egui)
- GPU queue submit
- Uniform buffer updates

...the GPU still showed ~20% usage when the window was open and event loop running.

**Root Cause:** The GPU usage appears to be baseline overhead from:
- Window surface presentation (swapchain)
- wgpu/winit event loop processing
- Driver-level frame management
- Background compositor/DWM on Windows

The only way to eliminate GPU usage was to disable `render()` entirely, which broke frame timing and caused 100% CPU usage in the event loop.

## Technical Details

### Current Rendering Architecture

The app runs a fixed 60 FPS loop (or 60 × speed_multiplier when actively rendering):

```rust
// In AboutToWait event
let target_fps = 60.0 * multiplier as f64;
window.request_redraw();  // Triggers render() at target FPS
```

Each frame, `render()` is called and performs:
1. **Compute pass** (only if `should_iterate`)
2. **Accumulate pass** (only if `should_iterate`)
3. **Tonemap pass** (always - reads accumulation buffer, outputs to surface)
4. **UI rendering** (always - egui overlay)
5. **GPU submit** (always - presents frame to window)

### Why We Can't Skip Rendering When Idle

- **UI needs continuous updates**: Mouse hover, frame time display, metrics
- **Surface presentation required**: Can't skip frames without breaking vsync/compositing
- **egui architecture**: Immediate-mode UI rebuilds each frame

### What We Ruled Out

✅ Not the compute shader (disabled, usage unchanged)
✅ Not the accumulate shader (disabled, usage unchanged)
✅ Not the tonemap shader (disabled, usage unchanged)
✅ Not the UI rendering (disabled, usage unchanged)
✅ Not queue submit (disabled, usage unchanged)
✅ Not buffer uploads (disabled, usage unchanged)

### What Actually Causes the Usage

❌ Baseline overhead from window/surface/event loop
❌ Cannot be eliminated without disabling rendering entirely

## Performance Context

### Is 20% GPU Usage a Problem?

**No, this is expected behavior for a real-time rendering application:**

1. **Modern GPUs idle at low power** - 20% usage is well within normal operating range
2. **Frame presentation has overhead** - Swapchain, compositing, vsync all consume GPU cycles
3. **Real-time UI needs continuous rendering** - Can't predict when user will interact
4. **Desktop apps commonly run at 60 FPS** - Browser tabs, video players, games all do this

### Comparison

- **Export mode (headless)**: Runs at same speed as before (no regression)
- **CPU usage**: Same or slightly lower than before delta system
- **RAM usage**: Same as before
- **Active rendering**: GPU usage appropriate for workload (60-100% when computing samples)

## Potential Optimizations (Deferred)

If we want to reduce idle GPU usage in the future, possible approaches:

### 1. Reactive Rendering Mode
Only redraw when something changes:
- Mouse movement
- Keyboard input
- Fractal still rendering
- UI interaction

**Pros**: Minimal GPU usage when truly idle
**Cons**: Complex dirty-tracking, may feel less responsive, vsync issues

### 2. Lower Idle FPS
Drop to 30 FPS or lower when fractal finished:
```rust
let target_fps = if is_rendering { 60.0 * multiplier } else { 30.0 };
```

**Pros**: Simple, reduces overhead by 50%
**Cons**: UI feels sluggish, metrics update slower

### 3. Cached Tonemap Frame
Render fractal to intermediate texture, cache result when idle:
```rust
if should_iterate || tonemap_params_changed {
    render_tonemap_to_cache();
}
composite_cache_with_ui();  // Cheap blit instead of full tonemap
```

**Pros**: Avoids re-running tonemap shader when fractal unchanged
**Cons**: Major refactor, extra VRAM, cache invalidation complexity

### 4. Suspend Rendering When Minimized
Detect window minimize/hide and pause render loop entirely:

**Pros**: Zero GPU usage when window hidden
**Cons**: Requires platform-specific window state tracking

## Conclusion

**The 20% idle GPU usage is baseline overhead from the window/surface/event loop, not a bug or regression from the delta system changes.**

This is expected behavior for a real-time rendering application running at 60 FPS with an interactive UI. The performance is acceptable and optimization can be deferred to future work if needed.

**Recommendation**: Close this investigation. Performance is within normal bounds for this type of application.

## References

- Delta-based state management: [delta-based-state-management.md](../archive/delta-migration/delta-based-state-management.md)
- Event loop: [src/app/mod.rs](../../src/app/mod.rs) lines 175-289
- Render function: [src/app/mod.rs](../../src/app/mod.rs) lines 298-1068
