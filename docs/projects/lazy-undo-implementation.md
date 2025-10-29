# Lazy Undo System Implementation

**Status:** Implementation started
**Created:** 2025-10-28
**Category:** UI/UX Improvement

## Problem

The current undo system captures state every time `flame_changed = true` is set. During continuous slider drag or triangle editor interactions, this happens **every frame** (60 FPS), filling the 50-state undo buffer in less than 1 second.

**Example:**
```rust
// Current pattern in UI code (PROBLEMATIC):
if ui.add(egui::Slider::new(&mut value, 0.0..=1.0)).changed() {
    *flame_changed = true;  // Triggers undo capture EVERY FRAME while dragging
}
```

**Result:**
- User drags slider for 2 seconds = 120 undo states
- Undo buffer holds only 50 states
- 70 states lost, only last 50 frames preserved
- No useful undo history before the drag

## Solution

Implement **lazy undo capture** with throttling:
- **During drag:** Capture undo state **once per second**
- **On drag end:** **Always** capture final state
- **On drag start:** **Always** capture initial state

**Benefits:**
- 2-second drag = 3 undo states (start, 1-second mark, end)
- Undo buffer preserves meaningful states
- Works across all UI interactions (sliders, drag widgets, triangle editor)

## Implementation

### 1. Core Helper (`src/ui/lazy_undo.rs`)

**Status:** ✅ Complete

Created `LazyUndoHelper` struct that tracks:
- Last capture time (for throttling)
- Current drag state (for detecting transitions)
- Throttle duration (default: 1 second)

**API:**
```rust
use crate::ui::LazyUndoHelper;

// Create helper (typically stored in EguiLayer or UI function)
let mut lazy = LazyUndoHelper::new();

// For egui widgets with Response:
let response = ui.add(egui::Slider::new(&mut value, 0.0..=1.0));
if lazy.should_capture_for_widget(&response) {
    *flame_changed = true;  // Only triggers at start, every 1s, and end
}

// For custom drag detection:
if lazy.should_capture_for_drag_state(is_mouse_down) {
    *flame_changed = true;
}
```

**Test Coverage:**
- ✅ Drag start capture
- ✅ Throttling during continuous drag
- ✅ Drag end capture
- ✅ Reset state

### 2. Integration Points

**Status:** 🔨 In Progress

Need to update all UI locations where continuous interactions trigger `flame_changed`:

#### A. Settings Window Sliders

**File:** `src/ui/settings.rs`
**Lines:** 6 locations where `*flame_changed = true`

**Sliders to update:**
- Iterations per thread
- Speed multiplier
- Histogram color scale
- Low-density smoothing
- Density compression strength
- Blend factor
- Target iterations per pixel

**Pattern:**
```rust
// BEFORE (captures every frame):
if ui.add(egui::Slider::new(iterations_per_thread, 1..=1024)).changed() {
    *iterations_changed = true;
}

// AFTER (captures throttled):
let response = ui.add(egui::Slider::new(iterations_per_thread, 1..=1024));
if lazy_undo.should_capture_for_widget(&response) {
    *iterations_changed = true;
}
```

#### B. Transform Editor

**File:** `src/ui/transforms.rs`
**Lines:** 14 locations where `*flame_changed = true`

**Controls to update:**
- Affine transformation sliders (a, b, c, d, e, f, g)
- Transform weight
- Color RGB sliders
- Color speed slider
- Variation weight sliders

#### C. Triangle Editor

**File:** `src/ui/triangle_editor.rs`
**Lines:** 13 locations where `*flame_changed = true`

**Status:** ⚠️ Special Case - Already has partial solution

The triangle editor already has `triangle_drag_started`, `triangle_dragging`, and `triangle_drag_ended` flags. These work correctly for undo:

```rust
// From src/app/mod.rs line 946:
let should_capture = ui_response.triangle_drag_started || view_changed
    || ui_response.color_mode_changed || ui_response.density_changed
    || ui_response.background_color_changed
    || ui_response.tonemap_mode_changed || ui_response.tonemap_curve_changed
    || (ui_response.flame_changed && !ui_response.triangle_drag_started);
```

**No changes needed** - triangle editor already only captures on drag start, not during continuous drag.

#### D. Variation Controls

**File:** `src/ui/variation_controls.rs`
**Lines:** 1 location where `*flame_changed = true`

**Controls to update:**
- Variation weight sliders (in list of active variations)

#### E. Variation Parameters

**File:** `src/ui/variation_params.rs`
**Lines:** 1 location where `*flame_changed = true`

**Controls to update:**
- Parameter value sliders (power, distance, waves, etc.)

#### F. Tone Mapping Window

**File:** `src/ui/tone_mapping.rs`
**Lines:** Not in original search, but has many sliders

**Controls to update:**
- Exposure slider
- Gamma slider
- Density scale slider
- Speed factor slider (if in this window)

#### G. View Window

**File:** `src/ui/view.rs`
**Lines:** Not in original search, but has sliders

**Controls to update:**
- Zoom slider
- Pan X/Y sliders
- Rotation slider
- Camera rotation X/Y sliders

### 3. Storage Location for LazyUndoHelper

**Option A: Per-Window Helpers (Recommended)**

Store separate `LazyUndoHelper` instances in `EguiLayer` for each UI section:

```rust
// In src/ui/mod.rs:
pub struct EguiLayer {
    // ... existing fields ...

    // Lazy undo helpers for throttling
    lazy_undo_settings: LazyUndoHelper,
    lazy_undo_transforms: LazyUndoHelper,
    lazy_undo_variations: LazyUndoHelper,
    lazy_undo_tone_mapping: LazyUndoHelper,
    lazy_undo_view: LazyUndoHelper,
}
```

**Pros:**
- Each UI section tracks its own drag state independently
- Multiple sliders can be dragged simultaneously without conflicts
- Clean separation of concerns

**Cons:**
- More fields in EguiLayer
- Slightly more initialization code

**Option B: Single Shared Helper**

Store one `LazyUndoHelper` in `EguiLayer` and pass it to all UI functions:

```rust
pub struct EguiLayer {
    // ... existing fields ...
    lazy_undo: LazyUndoHelper,
}
```

**Pros:**
- Single field
- Simpler initialization

**Cons:**
- Could have conflicts if multiple sliders dragged at once (unlikely in practice)
- Less granular control

**Recommendation:** Use **Option A** for maximum correctness, especially for transforms window which has many sliders.

### 4. Migration Strategy

**Phase 1: Settings Window** (Low Risk)
- Add `lazy_undo_settings` to EguiLayer
- Update settings.rs to use helper for all sliders
- Test with iterations/speed/blend sliders
- Verify undo behavior during drag

**Phase 2: Tone Mapping & View** (Low Risk)
- Add `lazy_undo_tone_mapping` and `lazy_undo_view` to EguiLayer
- Update tone_mapping.rs and view.rs
- Test exposure/gamma/zoom sliders

**Phase 3: Transforms** (Medium Risk)
- Add `lazy_undo_transforms` to EguiLayer
- Update transforms.rs for all affine/color/weight sliders
- Test with multiple transforms and sliders

**Phase 4: Variations** (Low Risk)
- Add `lazy_undo_variations` to EguiLayer
- Update variation_controls.rs and variation_params.rs
- Test variation weight and parameter sliders

**Phase 5: Verification** (Critical)
- Test all UI interactions with undo
- Verify undo buffer contains meaningful states after long drag
- Check edge cases (rapid slider movements, window switching during drag)

## Testing Plan

### Manual Testing

1. **Basic Slider Drag:**
   - Drag iterations slider for 3 seconds
   - Release
   - Press Undo multiple times
   - Verify: See state at end, 2-second mark, 1-second mark, start

2. **Multiple Sliders:**
   - Drag exposure slider
   - Drag gamma slider
   - Drag density slider
   - Press Undo multiple times
   - Verify: Each slider change preserved independently

3. **Rapid Interactions:**
   - Quickly drag slider back and forth
   - Release
   - Press Undo
   - Verify: Captures at 1-second intervals plus final state

4. **Triangle Editor:**
   - Drag triangle vertex for 2 seconds
   - Release
   - Press Undo
   - Verify: Returns to pre-drag state (not intermediate positions)

5. **Mixed Interactions:**
   - Drag slider
   - Click button (add transform)
   - Drag another slider
   - Press Undo repeatedly
   - Verify: All actions preserved in order

### Edge Cases

- **Window close during drag:** Helper should reset on window close
- **App pause during drag:** Time should still advance correctly (uses `web_time::Instant`)
- **Very long drag (10+ seconds):** Should capture every second, plus final
- **Drag without movement:** Still captures start and end
- **Multiple windows open:** Each helper tracks its own state independently

## Performance Impact

**Minimal:**
- `LazyUndoHelper::should_capture_for_widget()` is ~10 instructions
- Called once per slider per frame (only when sliders exist)
- No allocations, just time comparison
- Same cost as current system, but fewer undo captures = faster `capture_state()` calls

**Measurement:**
- Current: 60 undo captures/second during drag = ~1-5ms overhead
- Lazy: 1 undo capture/second during drag = ~0.02ms overhead
- **Net improvement:** ~1-5ms saved per second of dragging

## API Documentation

### `LazyUndoHelper::new()`

Creates helper with 1-second throttle (default).

### `LazyUndoHelper::with_throttle(secs: u64)`

Creates helper with custom throttle duration.

**Example:**
```rust
// Capture every 2 seconds during drag:
let lazy = LazyUndoHelper::with_throttle(2);
```

### `LazyUndoHelper::should_capture_for_widget(&mut self, response: &egui::Response) -> bool`

**Primary API for egui widgets.**

Returns `true` when undo state should be captured:
- First frame of drag (drag started)
- During drag, if 1+ seconds elapsed since last capture
- Frame when drag ends

**Usage:**
```rust
let response = ui.add(egui::Slider::new(&mut value, 0.0..=1.0));
if lazy.should_capture_for_widget(&response) {
    *flame_changed = true;
}
```

### `LazyUndoHelper::should_capture_for_drag_state(&mut self, dragging: bool) -> bool`

**Manual API for custom drag detection.**

Use when not working with egui::Response (e.g., custom input handling).

**Usage:**
```rust
if lazy.should_capture_for_drag_state(is_mouse_down) {
    *flame_changed = true;
}
```

### `LazyUndoHelper::reset(&mut self)`

Resets helper state. Call when switching UI contexts or closing windows.

## Future Enhancements

### 1. Configurable Throttle

Add UI setting for throttle duration:
- Default: 1 second
- Range: 0.1 - 5.0 seconds
- Stored in app settings

### 2. Per-Control Throttle

Different controls could have different throttle rates:
- Fast controls (zoom, pan): 0.5 seconds
- Slow controls (gamma, exposure): 2 seconds
- Heavy controls (transforms): 1 second

### 3. Adaptive Throttle

Adjust throttle based on undo buffer fullness:
- Buffer < 50% full: 2 second throttle
- Buffer 50-80% full: 1 second throttle
- Buffer > 80% full: 0.5 second throttle

### 4. Undo Buffer Statistics

Add UI to show:
- Current buffer fullness (e.g., "32 / 50 states")
- Time range covered (e.g., "Last 2 minutes")
- Memory usage (each state ~2-10KB depending on transforms)

## Related Files

**Implementation:**
- [src/ui/lazy_undo.rs](../../src/ui/lazy_undo.rs) - Core helper implementation
- [src/ui/mod.rs](../../src/ui/mod.rs) - Module definition and EguiLayer storage
- [src/ui/response.rs](../../src/ui/response.rs) - UiResponse flags

**Usage:**
- [src/ui/settings.rs](../../src/ui/settings.rs) - Settings sliders
- [src/ui/transforms.rs](../../src/ui/transforms.rs) - Transform editor
- [src/ui/variation_controls.rs](../../src/ui/variation_controls.rs) - Variation weights
- [src/ui/variation_params.rs](../../src/ui/variation_params.rs) - Variation parameters
- [src/ui/tone_mapping.rs](../../src/ui/tone_mapping.rs) - Tone mapping sliders
- [src/ui/view.rs](../../src/ui/view.rs) - View controls

**Undo System:**
- [src/app/config.rs](../../src/app/config.rs) - `capture_state()`, undo/redo logic
- [src/app/mod.rs](../../src/app/mod.rs) - Main render loop, undo trigger handling

## References

**egui Response API:**
- `response.dragged()` - Returns true while mouse button held after drag threshold
- `response.drag_started()` - Returns true only on first frame of drag
- `response.drag_stopped()` - Returns true only on frame when drag ends
- `response.changed()` - Returns true when widget value changed

**web_time::Instant:**
- Platform-independent time measurement (works on WASM)
- `Instant::now()` - Current time
- `instant.duration_since(earlier)` - Elapsed time since earlier instant

## Conclusion

The lazy undo system solves a critical UX problem with minimal implementation complexity. By throttling undo captures to meaningful intervals, it preserves useful undo history while reducing CPU overhead.

**Next Steps:**
1. Implement Phase 1 (Settings window)
2. Test thoroughly
3. Roll out to remaining UI sections
4. Document in user-facing help
