# Live Mode Accumulation Problem

**Status:** ✅ RESOLVED - Minimal shader fix (3 files, ~30 lines)
**Date:** 2025-10-30
**Related:** Phase 4 - Triangle Editor migration to delta-based state management

---

## Problem Statement

During lazy drag operations (Triangle Editor, sliders), the preview system exhibits progressive brightness buildup in dense areas of the fractal. The brightness persists even after the drag ends, requiring a manual reset or undo/redo to restore correct rendering.

**User observation:**
> "When I'm in 'live mode', the really dense parts get brighter and brighter. Much brighter than it should be in the normal render. When it exits 'live mode', the brightness remains."

---

## Background: How Rendering Works

### Render Pipeline (Every Frame)

The fractal renderer uses a **3-pass progressive refinement pipeline**:

```
┌─ FRAME N ─────────────────────────────────────────┐
│ 1. render() - Main render loop entry              │
│                                                    │
│ 2. compute_pass()                                  │
│    • Generate 32,768 fractal samples (128 WGs)    │
│    • Write samples to histogram buffer            │
│    • Histogram format: [R, G, B, Density] as u32  │
│    • Runtime: ~1-2ms                               │
│                                                    │
│ 3. accumulate_pass()                               │
│    • Read histogram buffer                         │
│    • Blend with previous accumulation texture     │
│    • Formula: new = old * (1 - α) + histogram * α │
│    • Clear histogram for next frame               │
│    • Swap ping-pong buffers                       │
│    • Runtime: ~0.1ms                               │
│                                                    │
│ 4. tonemap_pass()                                  │
│    • Read accumulation texture                     │
│    • Apply tone mapping, gamma, exposure          │
│    • Render to screen                             │
│    • Runtime: ~0.1ms                               │
│                                                    │
│ 5. UI rendering                                    │
│ 6. frame.present() - Show on screen               │
└───────────────────────────────────────────────────┘
```

**Key insight:** The pipeline is **sequential within a single frame**. All passes run before the frame is presented to the screen.

### Progressive Refinement (Normal Mode)

Over many frames, samples accumulate:

```
Frame 1:  accumulation = 0 + samples₁ * blend
Frame 2:  accumulation = (Frame 1 result) * (1 - blend) + samples₂ * blend
Frame 3:  accumulation = (Frame 2 result) * (1 - blend) + samples₃ * blend
...
Frame N:  High quality, converged image
```

**Blend factor modes:**
- **Dynamic (default)**: `blend = samples_this_frame / total_accumulated`
  - Starts high (e.g., 0.1 = 10%), decreases over time
  - Exponential convergence to stable image
- **Fixed**: `blend = 0.1` (constant 10% per frame)
  - Used for testing, not recommended for production

**Normal rendering behavior:**
- Accumulation buffer grows over time
- Dense areas get brighter as more samples hit them
- `reset()` called on flame changes, view changes, etc.
- Reset clears accumulation buffers to zero, starts fresh

---

## The Live Mode Problem

### What is "Live Mode"?

When a user drags a slider or Triangle Editor point:
- ConfigManager enters **preview mode** (`is_in_preview_mode() == true`)
- Every frame, the fractal updates to reflect the current drag position
- This gives **live visual feedback** during drag
- When drag ends, preview commits to current state

**Expected behavior:**
- During drag: Fast, noisy preview (intentionally lower quality)
- After drag: Return to high quality, progressive refinement

### What Goes Wrong

**Symptom:** Dense areas get progressively brighter during drag and stay overbright after drag ends.

**Root cause:** Alpha (density) channel accumulates additively even in overwrite mode.

### Investigation Timeline

**Attempt 1: Reset every frame during preview mode**
- **Hypothesis**: Accumulation buffer needs to be cleared during drag
- **Result**: Black/disappearing screen during drag
- **Why it failed**: Sequential pipeline (compute → accumulate → tonemap in same frame) means reset clears the buffer that's immediately used for blending, resulting in very faint output

**Attempt 2: Overwrite mode (blend_factor = 1.0)**
- **Hypothesis**: Replace accumulation buffer instead of blending during preview
- **Implementation**: Set `blend_factor = 1.0` in accumulate_pass() when in preview mode
- **Result**: Still has progressive brightness buildup
- **Why it failed**: Only fixed RGB blending, alpha still accumulates additively

### The Real Problem: Additive Alpha Accumulation

Found in [shaders/accumulate.wgsl](../../shaders/accumulate.wgsl):

```wgsl
// Line 88: RGB blending (correctly uses blend_factor)
let rgb_accumulated = prev.rgb * (1.0 - adjusted_blend) + new_color * adjusted_blend;

// Line 92: Alpha accumulation (ADDITIVE, ignores blend_factor!)
let alpha_accumulated = prev.a + (density * 0.01 * params.blend_factor * convergence_gate);
```

**The bug:**
- **RGB** is blended using `blend_factor`: when `blend_factor = 1.0`, RGB is correctly overwritten
- **Alpha (density)** is **additively accumulated** regardless of blend_factor
- Even with `blend_factor = 1.0`, alpha keeps growing: `prev.a + new_density`
- Tonemap shader uses alpha for brightness calculation
- Progressive brightness is caused by alpha growing frame after frame

**Why this matters:**
- During preview mode with overwrite enabled (`blend_factor = 1.0`):
  - RGB changes correctly (shows current fractal state)
  - Alpha keeps accumulating (grows every frame)
  - Tonemap sees growing alpha → increasing brightness
  - Dense areas hit more → alpha grows faster → get much brighter

**Why this happens:**

1. **Frame 1 of drag:**
   - `compute_pass()` generates samples for fractal state A
   - `accumulate_pass()` blends with previous buffer (shows state A)
   - Buffer now contains: old samples + new samples for state A

2. **Frame 2 of drag (fractal changes to state B):**
   - `compute_pass()` generates samples for fractal state B
   - `accumulate_pass()` blends with previous buffer
   - **Problem:** Previous buffer contains samples for state A!
   - Buffer now contains: (old + A) + B = **mixed states**

3. **Frame 3 of drag (fractal changes to state C):**
   - Buffer now contains: (old + A + B) + C = **even more mixed**

4. **Result:** Dense areas accumulate samples from **multiple different fractal states**, causing progressive brightness buildup.

### Why Reset Doesn't Work During Live Mode

**Attempted fix #1:** Call `reset()` every frame during preview mode

```rust
if in_preview_mode {
    renderer.reset(...);  // Clear accumulation buffers
}
// Later in same frame:
renderer.compute_pass(...);   // Generate samples
renderer.accumulate_pass(...); // Blend with empty buffer
renderer.tonemap_pass(...);    // Render to screen
```

**What happens:**

```
┌─ FRAME N (during drag) ───────────────────────────┐
│ 1. reset() - Clear accumulation to ZERO           │
│ 2. compute_pass() - Generate 32,768 samples       │
│ 3. accumulate_pass()                               │
│    • old_accumulation = 0 (just cleared!)         │
│    • blend_factor = 0.1 (10%)                     │
│    • new = 0 * 0.9 + samples * 0.1                │
│    • Result: 10% brightness                       │
│ 4. tonemap_pass() - Render very faint image       │
│ 5. frame.present() - NEARLY BLACK SCREEN          │
└───────────────────────────────────────────────────┘
```

**User observation:**
> "Complete regression in behavior. Now dragging makes everything disappear when it's in 'live mode'."

**Why this fails:**
- Reset clears accumulation to zero
- Same frame generates only 32,768 samples (one batch)
- Blend factor of 10% makes result very faint
- Not enough samples in one frame to produce visible image
- Results in flickering or black screen during drag

**Fundamental problem:** The pipeline is sequential. You cannot clear the buffer and show a full-brightness result in the same frame with exponential blending.

---

## Timeline of Attempts

### Attempt #1: Add flag tracking and transition detection
**Changes:**
- Added `preview_just_created`, `preview_just_committed` timestamps
- Added stale detection logic
- Added 5 new fields, 3 new methods

**Result:** Failed - Still had brightness buildup
**User feedback:** "That still didn't fix it. git diff and clean up the code. Has any of it improved anything?"

### Attempt #2: Reset every frame during live mode
**Changes:**
- Call `renderer.reset()` every frame when `is_in_preview_mode()`
- Call `renderer.reset()` when exiting preview mode

**Result:** Regression - Black/disappearing screen during drag
**User feedback:** "Complete regression in behavior. Now dragging makes everything disappear when it's in 'live mode'."

### Lessons Learned

1. **Cannot reset during live mode** - Timing problem makes it produce black frames
2. **Complex state tracking doesn't help** - The problem is architectural, not a missing flag
3. **Need different approach** - Working against the pipeline architecture

---

## Proposed Solution: Overwrite Mode

### Core Idea

Instead of **blending** new samples with old accumulation during live mode, **replace** the accumulation buffer entirely.

**Normal mode (progressive refinement):**
```
new_pixel = old_pixel * (1 - blend_factor) + new_samples * blend_factor
```

**Overwrite mode (live preview):**
```
new_pixel = new_samples * 1.0  // Ignore old_pixel entirely
```

### Why This Works

**During drag (overwrite mode):**
- Each frame completely replaces the previous frame's samples
- No mixing of different fractal states
- No progressive brightness buildup
- Shows current fractal state immediately (noisy but correct)

**After drag (normal mode):**
- Overwrite mode disabled
- Reset triggered once on exit
- Progressive refinement resumes
- Converges to high quality image

### Implementation Plan

#### 1. Add overwrite_mode field to FlameRenderer

```rust
// src/renderer/compute_kernel.rs
pub struct FlameRenderer {
    // ... existing fields ...
    overwrite_mode: bool,  // When true, replace accumulation instead of blending
}

impl FlameRenderer {
    pub fn new(...) -> Self {
        Self {
            // ... existing initialization ...
            overwrite_mode: false,
        }
    }

    pub fn set_overwrite_mode(&mut self, overwrite: bool) {
        self.overwrite_mode = overwrite;
    }
}
```

#### 2. Modify accumulate_pass() to use overwrite mode

```rust
// src/renderer/compute_kernel.rs - accumulate_pass()
pub fn accumulate_pass(&mut self, ...) {
    self.samples_accumulated += samples_this_frame;

    // Calculate blend_factor based on mode
    let blend_factor = if self.overwrite_mode {
        // Overwrite mode (live preview): Replace old buffer entirely
        1.0
    } else if self.use_dynamic_blend {
        // Exponential convergence (normal mode): blend_factor decreases over time
        samples_this_frame as f32 / self.samples_accumulated as f32
    } else {
        // Fixed blend rate (testing mode): constant blend per frame
        self.blend_factor
    };

    let params = AccumulateParams {
        // ... existing params ...
        blend_factor,
        // ...
    };

    // ... rest of accumulate_pass ...
}
```

**Key change:** When `overwrite_mode = true`, `blend_factor = 1.0`, which makes the shader formula:
```
new_pixel = old_pixel * 0.0 + new_samples * 1.0 = new_samples
```

#### 3. Control overwrite mode from App

```rust
// src/app/mod.rs - Add field to App struct
pub struct App {
    // ... existing fields ...
    was_in_preview_mode_last_frame: bool,  // Track preview mode transitions
}

// src/app/mod.rs - Initialize in App::run()
let mut app = App {
    // ... existing initialization ...
    was_in_preview_mode_last_frame: false,
};

// src/app/mod.rs - In render() method, before compute pass
if let Some(ref mut renderer) = self.flame_renderer {
    let in_preview_mode = self.config_manager.is_in_preview_mode();

    // Enable overwrite mode during preview (live mode)
    renderer.set_overwrite_mode(in_preview_mode);

    // Detect transition out of preview mode (drag ended)
    let exiting_preview_mode = self.was_in_preview_mode_last_frame && !in_preview_mode;
    if exiting_preview_mode {
        // Reset once when exiting to start fresh high-quality render
        renderer.reset(&mut update_encoder, &self.gpu.queue,
                      self.iterations_per_thread, self.zoom, self.pan_x, self.pan_y,
                      self.rotation, self.camera_rotation_x, self.camera_rotation_y,
                      self.speed_factor);
    }

    self.was_in_preview_mode_last_frame = in_preview_mode;

    // ... continue with compute/accumulate/tonemap passes ...
}
```

**Execution flow:**

**During drag (in_preview_mode = true):**
1. Set `overwrite_mode = true`
2. compute_pass() generates samples for current fractal state
3. accumulate_pass() uses `blend_factor = 1.0`, **replaces** previous buffer
4. tonemap_pass() renders current state (noisy but correct)
5. No progressive brightness buildup

**On drag release (exiting_preview_mode = true):**
1. Set `overwrite_mode = false` (back to normal blending)
2. Call `reset()` once to clear accumulation
3. Next frame starts fresh progressive refinement
4. Converges to high quality image

#### 4. No shader changes needed!

The accumulate shader already uses `blend_factor` from the uniform buffer:

```wgsl
// shaders/accumulate.wgsl (existing code, no changes needed)
let old_color = textureLoad(previous_texture, pixel_coords, 0).rgb;
let new_color = old_color * (1.0 - blend_factor) + histogram_color * blend_factor;
```

When `blend_factor = 1.0`:
```
new_color = old_color * 0.0 + histogram_color * 1.0 = histogram_color
```

Perfect overwrite behavior with no shader modifications!

---

## Advantages of Overwrite Mode

1. **Solves progressive brightness:** Each frame shows only current fractal state
2. **No timing issues:** Works within existing pipeline architecture
3. **No blank frames:** Always shows visible image (one frame of samples)
4. **Simple implementation:**
   - One bool field in FlameRenderer
   - One setter method
   - One conditional in blend_factor calculation
   - One transition detection in App
5. **No shader changes:** Existing shader already supports it via blend_factor
6. **Intentional quality tradeoff:** Fast noisy preview during drag, high quality after
7. **Clean separation:** Preview mode has different accumulation behavior by design

---

## Expected User Experience

### Before Fix
- Drag Triangle Editor point
- Dense areas get progressively brighter
- Drag ends, brightness remains
- Must undo/redo or change setting to fix

### After Fix
- Drag Triangle Editor point
- See live preview (noisy but correct brightness)
- Each frame shows current position
- Drag ends
- Brief reset (one frame)
- Progressive refinement resumes
- Converges to high quality in ~1 second

---

## Testing Plan

1. **Triangle Editor drag:**
   - Drag point across fractal
   - Verify no progressive brightness in dense areas
   - Verify preview updates every frame
   - Release drag
   - Verify reset occurs
   - Verify high quality render after ~1 second

2. **ConfigSlider drag (lazy mode):**
   - Drag rotation slider
   - Verify smooth live preview
   - Verify correct brightness throughout
   - Release drag
   - Verify clean return to high quality

3. **Multiple rapid drags:**
   - Drag, release, drag, release quickly
   - Verify reset triggers between drags
   - Verify no accumulated brightness

4. **Long drag:**
   - Hold drag for 5+ seconds while moving
   - Verify brightness stays consistent
   - Verify no buildup over time

---

## Alternative Solutions Considered

### Alternative 1: Multi-frame reset with forced blank frame
**Approach:** Reset on frame N, skip rendering, show previous frame again, render on frame N+1

**Rejected because:**
- Complex state machine (needs tracking of "reset in progress")
- Intentional frame skipping feels wrong
- Would cause visible stutter/pause

### Alternative 2: Double-buffered preview accumulation
**Approach:** Maintain separate accumulation buffers for preview vs. normal mode

**Rejected because:**
- Doubles memory usage (2 more textures at viewport resolution)
- Complex buffer management
- Overwrite mode achieves same result with zero memory overhead

### Alternative 3: Synchronous reset after drag
**Approach:** Force GPU sync after drag ends, reset on CPU side before next frame

**Rejected because:**
- GPU sync causes major performance hit (stalls pipeline)
- Introduces latency after drag release
- Not compatible with async rendering

### Alternative 4: Adjust blend factor dynamically
**Approach:** Use higher blend factor (e.g., 0.5 = 50%) during preview mode

**Rejected because:**
- Doesn't solve the mixing problem (still blends old + new states)
- Still causes progressive brightness, just slower
- No clean cutoff between states

---

## Final Resolution

**The Minimal Fix:** Only 3 files changed, ~30 lines total. No reset-on-exit needed!

### What Was Implemented

1. **shaders/accumulate.wgsl** - Alpha handling with `select()`:
   ```wgsl
   let new_alpha = density * 0.01 * params.blend_factor * convergence_gate;
   let alpha_accumulated = select(
       prev.a + new_alpha,        // Normal: accumulate
       new_alpha,                 // Overwrite: replace
       params.blend_factor >= 0.99  // Check for overwrite mode
   );
   ```

2. **src/renderer/compute_kernel.rs** - Overwrite mode infrastructure:
   - Added `overwrite_mode: bool` field
   - Added `set_overwrite_mode()` method
   - Set `blend_factor = 1.0` when overwrite mode active

3. **src/app/mod.rs** - Enable overwrite during preview:
   ```rust
   let in_preview_mode = self.config_manager.is_in_preview_mode();
   renderer.set_overwrite_mode(in_preview_mode);
   ```

### Why Reset-On-Exit Wasn't Needed

**Key insight:** The shader fix creates valid single-frame data during preview. When preview ends, the accumulation buffer contains correct RGB and alpha values (just noisy due to low sample count). Progressive refinement can continue directly from this valid starting point - no reset required!

**Before fix:**
- Preview ends with corrupted buffer (overbright alpha)
- Reset required to clear corruption
- Jarring visual flash

**After fix:**
- Preview ends with valid buffer (correct single-frame snapshot)
- No reset needed - just continue accumulating
- Smooth transition from noisy preview to refined result

### Test Results

✅ Triangle Editor drag - No brightness buildup, smooth preview
✅ Slider drag - Correct brightness throughout
✅ Multiple rapid drags - Clean transitions
✅ Long continuous drag - Brightness stays consistent
✅ Drag release - Smooth convergence to high quality

### Benefits for Animation System

This fix is critical for the planned animation system:

**Animation requires:**
- Rendering many frames with varying parameter values
- Each frame must have correct, independent brightness
- No accumulation of density across different parameter states

**Overwrite mode provides:**
- Single-frame snapshots at any parameter value
- No cross-contamination between frames
- Consistent brightness regardless of parameter changes

**Use cases:**
- Parameter interpolation (morph transforms over time)
- Keyframe rendering (jump between discrete states)
- Real-time parameter scrubbing (drag timeline slider)
- Batch rendering (export frame sequence)

The animation system can use `set_overwrite_mode(true)` to render each frame independently, then optionally use progressive refinement (overwrite mode off) for final quality passes.

---

## Related Documentation

- [delta-based-state-management.md](delta-based-state-management.md) - Phase 4 implementation
- [lazy-undo-implementation.md](lazy-undo-implementation.md) - Lazy undo system
- [docs/main/RENDERER.md](../main/RENDERER.md) - 3-pass pipeline architecture
- [docs/main/BUFFERS.md](../main/BUFFERS.md) - GPU buffer layouts
