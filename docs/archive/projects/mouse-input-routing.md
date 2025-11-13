# Mouse Input Routing Analysis

## Problem Statement

Mouse dragging within egui panels sometimes incorrectly affects the fractal viewport, causing unwanted pan operations. The behavior is inconsistent and occurs when interacting with UI elements inside dockable panels.

## Current Architecture

### Rendering Order
1. **Fractal Rendering** (lines 327-392 in `src/app/mod.rs`):
   - Compute pass (generates fractal samples)
   - Accumulate pass (progressive refinement)
   - Tonemap pass (renders to `view` - the surface texture)

2. **UI Rendering** (lines 394-418):
   - egui panels rendered on top of fractal
   - Uses same `view` texture as render target
   - Returns `ui_response` with state changes

3. **Submit** (line 422):
   - Single command encoder submitted to GPU

### Input Flow

**Event Path:**
```
Window Events
    ↓
egui_layer.handle_event() [src/ui/mod.rs:71-84]
    ↓ (returns consumed: bool)
Event Loop [src/app/mod.rs:221-235]
    ↓ (checks !consumed)
App input handlers [src/app/input.rs]
    ↓
ConfigManager updates
```

**Current Logic (src/ui/mod.rs:71-84):**
```rust
pub fn handle_event(&mut self, event: &WindowEvent, window: &Window) -> bool {
    let response = self.state.on_window_event(window, event);

    match event {
        WindowEvent::MouseInput { .. }
        | WindowEvent::CursorMoved { .. }
        | WindowEvent::MouseWheel { .. } => {
            // Only consume if egui is actively using the pointer
            response.consumed && self.ctx.is_using_pointer()
        }
        _ => response.consumed
    }
}
```

**App Event Handling (src/app/mod.rs:225-235):**
```rust
WindowEvent::MouseInput { state, button, .. } => {
    // Always handle mouse releases to clear dragging state,
    // but only handle presses if egui didn't consume them
    app.handle_mouse_button(state, button, consumed);
}
WindowEvent::CursorMoved { position, .. } if !consumed => {
    app.handle_mouse_move(position.x as f32, position.y as f32);
}
WindowEvent::MouseWheel { delta, phase, .. } if !consumed => {
    app.handle_mouse_wheel(delta, phase);
}
```

### Mouse Drag Implementation

**State Tracking (src/app/mod.rs:36, 139):**
- `mouse_dragging: bool` - Tracks if user is currently dragging

**Press Handling (src/app/input.rs:132-159):**
```rust
ElementState::Pressed => {
    if !consumed {
        self.mouse_dragging = true;  // Only start if egui didn't consume
    }
}
ElementState::Released => {
    let was_dragging = self.mouse_dragging;
    self.mouse_dragging = false;
    self.last_mouse_pos = None;

    if was_dragging {
        // Commit pan preview
        self.config_manager.force_commit_preview(&ConfigPath::PanX);
    }
}
```

**Move Handling (src/app/input.rs:162-196):**
```rust
pub(super) fn handle_mouse_move(&mut self, x: f32, y: f32) {
    if self.mouse_dragging {
        if let Some((last_x, last_y)) = self.last_mouse_pos {
            // Calculate delta and update pan
            // Uses preview mode (lazy=true)
        }
    }
    self.last_mouse_pos = Some((x, y));
}
```

## Root Cause Analysis

### The Bug

**Symptom:** Mouse drag inside a panel sometimes pans the fractal.

**Likely Causes:**

1. **Race Condition in `is_using_pointer()`:**
   - `ctx.is_using_pointer()` may return false even when mouse is over a panel
   - Could be timing issue - pointer state not updated until next frame
   - DockArea panels may not properly report pointer usage

2. **`mouse_dragging` State Persistence:**
   - Press is consumed by egui → `mouse_dragging` stays false ✅
   - But if press is NOT consumed, `mouse_dragging = true`
   - Then user drags into a panel → moves still processed
   - Release inside panel might be consumed → state never cleared ❌

3. **Inconsistent Consumption:**
   - Press might not be consumed (background of panel)
   - Moves might not be consumed (dragging over panel content)
   - Creates window where drag starts "valid" then becomes "invalid"

### Evidence of Current Workarounds

**Special Release Handling (src/app/mod.rs:225-228):**
```rust
// Always handle mouse releases to clear dragging state,
// but only handle presses if egui didn't consume them
app.handle_mouse_button(state, button, consumed);
```

This comment suggests awareness that release events need special handling to prevent stuck states.

## Solution Approaches

### Option 1: Improve Input Routing (Incremental Fix)

**Scope:** Small to Medium
**Estimated Effort:** 1-3 days
**Risk:** Medium (may not fully solve the problem)

**Approach:**
1. **Continuous Pointer Tracking:**
   - Check `ctx.is_using_pointer()` during move events, not just press
   - Cancel drag if pointer becomes "in use" mid-drag
   - Commit preview when canceling

2. **Explicit Panel Hit Testing:**
   - Check if mouse position is inside any panel rect
   - Use egui's `ctx.layer_id_at()` or similar
   - More reliable than `is_using_pointer()`

3. **Drag Validation:**
   ```rust
   pub(super) fn handle_mouse_move(&mut self, x: f32, y: f32, consumed: bool) {
       if self.mouse_dragging {
           // NEW: Check if egui is now using pointer
           if consumed || self.egui_layer.ctx.is_using_pointer() {
               // Cancel drag, commit preview
               self.mouse_dragging = false;
               self.config_manager.force_commit_preview(&ConfigPath::PanX);
               return;
           }
           // ... existing drag logic
       }
   }
   ```

**Pros:**
- Minimal architectural change
- Preserves current rendering approach
- Can be implemented incrementally

**Cons:**
- May still have edge cases
- Relies on egui's pointer detection being accurate
- Doesn't address fundamental layering issue

### Option 2: Convert Fractal to Panel (Architectural Change)

**Scope:** Large
**Estimated Effort:** 3-7 days
**Risk:** High (significant refactoring)

**Approach:**
1. **Create Central Panel:**
   ```rust
   egui::CentralPanel::default()
       .frame(egui::Frame::none()) // No visible frame
       .show(ctx, |ui| {
           // Fractal rendering happens here
           self.render_fractal_to_ui(ui);
       });
   ```

2. **Custom Widget for Fractal:**
   - Create `FractalWidget` that implements `egui::Widget`
   - Handles its own mouse input via `ui.interact()`
   - Renders fractal texture as image

3. **Texture as Image:**
   - Fractal still rendered to offscreen texture
   - Display via `ui.image()` or custom paint callback
   - egui automatically handles input routing

**Implementation Sketch:**
```rust
// In src/ui/fractal_widget.rs (new file)
pub struct FractalWidget<'a> {
    renderer: &'a mut FlameRenderer,
    texture_id: egui::TextureId,
}

impl<'a> egui::Widget for FractalWidget<'a> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let desired_size = ui.available_size();
        let (rect, response) = ui.allocate_exact_size(
            desired_size,
            egui::Sense::click_and_drag()
        );

        // Handle drag for panning
        if response.dragged() {
            let delta = response.drag_delta();
            // Update pan via ConfigManager
        }

        // Handle scroll for zoom
        if let Some(hover_pos) = response.hover_pos() {
            ui.input(|i| {
                if i.scroll_delta.y != 0.0 {
                    // Update zoom via ConfigManager
                }
            });
        }

        // Render fractal texture
        ui.painter().image(
            self.texture_id,
            rect,
            egui::Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
            Color32::WHITE,
        );

        response
    }
}
```

**Changes Required:**

1. **Rendering Pipeline:**
   - Keep fractal rendering to offscreen texture
   - Register texture with egui: `ctx.load_texture()`
   - Display texture in CentralPanel

2. **Input Handling:**
   - Remove direct winit event handlers for mouse
   - Move all input logic into FractalWidget
   - Use egui's Response system

3. **Texture Management:**
   - Create persistent texture for fractal output
   - Update texture each frame
   - Handle resize via texture recreation

4. **Layout:**
   ```
   TopBottomPanel::top()    [Menu Bar]
   SidePanel::left()        [Transforms, Triangle Editor]
   SidePanel::right()       [Colors, View, etc.]
   CentralPanel::default()  [Fractal Widget] ← NEW
   ```

**Pros:**
- ✅ **Automatic Input Routing:** egui handles all click/drag detection
- ✅ **Consistent Behavior:** Same input system as all other UI
- ✅ **No Edge Cases:** Widget bounds are explicit and accurate
- ✅ **Future Features:** Easier to add minimap, grid overlay, etc.
- ✅ **Better Architecture:** Cleaner separation of concerns

**Cons:**
- ❌ **Major Refactoring:** Touches many files
- ❌ **Texture Indirection:** One extra copy per frame
- ❌ **Performance Concern:** May add latency (need to benchmark)
- ❌ **Breaking Change:** Input code completely rewritten
- ❌ **Testing Overhead:** All input behavior must be re-tested

### Performance Analysis (Option 2)

**Current Pipeline:**
```
Compute → Accumulate → Tonemap → [Surface]
                                     ↑
                                  Display
```

**Panel-Based Pipeline:**
```
Compute → Accumulate → Tonemap → [Offscreen Texture]
                                     ↓
                                  [Copy to egui texture]
                                     ↓
                                  [CentralPanel image display]
                                     ↓
                                  [Surface]
```

**Additional Costs:**
1. **Texture Copy:** One additional GPU texture copy per frame
   - Cost: ~0.1-0.5ms for 1920x1080 (depends on GPU)
   - Mitigated by: Already copying for tonemap pass

2. **egui Overhead:** Image widget rendering
   - Cost: Negligible (single textured quad)

3. **Input Processing:** Widget interaction
   - Cost: Negligible (replaces existing logic)

**Overall Impact:** Likely 1-3% performance decrease, acceptable for correctness.

## Recommendations

### Short Term: Option 1 (Improved Routing)

**Why:**
- Fast to implement
- Low risk
- Can be done incrementally
- May fully solve the problem

**Action Items:**
1. Add `consumed` parameter to `handle_mouse_move()`
2. Check pointer state during drag, not just on press
3. Cancel drag and commit preview if pointer enters UI

**Implementation Priority:**
- High - Addresses immediate bug

### Long Term: Option 2 (Panel Architecture)

**Why:**
- More robust solution
- Better architectural alignment
- Enables future features (overlays, minimap, etc.)
- Standard egui pattern

**When to Consider:**
- If Option 1 doesn't fully solve the problem
- When adding features that need viewport interaction
- During major refactoring

**Prerequisites:**
- Benchmark texture copy performance
- Prototype widget implementation
- Test with existing input scenarios

## Implementation Plan (Option 1)

### Phase 1: Add Drag Cancellation (1 day)
1. Modify `handle_mouse_move()` signature to include `consumed`
2. Check `consumed || is_using_pointer()` during drag
3. Cancel drag if pointer enters UI mid-drag
4. Test with all panel types

### Phase 2: Improve Hit Testing (1 day)
1. Add explicit panel bounds checking
2. Use `ctx.layer_id_at()` for more accurate detection
3. Log edge cases for debugging
4. Test extensively with various panel configurations

### Phase 3: Edge Case Handling (1 day)
1. Handle window focus changes during drag
2. Handle panels opening/closing mid-drag
3. Handle drag starting in panel background
4. Add comprehensive test cases

## Alternative: Hybrid Approach

**Idea:** Use Option 1 immediately, plan Option 2 for future.

**Rationale:**
- Get bug fix quickly
- Gather data on remaining edge cases
- Design panel architecture with full knowledge of requirements

**Timeline:**
- Week 1: Implement Option 1 improvements
- Week 2-3: Gather feedback, identify remaining issues
- Week 4+: Evaluate if Option 2 is needed

## Conclusion

**Recommended Path:**
1. **Immediate:** Implement Option 1 (improved input routing)
2. **Monitor:** Track if issues persist after Option 1
3. **Future:** Consider Option 2 if needed, or when adding viewport features

**Success Criteria:**
- No unintended fractal panning when interacting with panels
- Drag operations cancel cleanly when pointer enters UI
- No stuck drag states
- No performance regression

**Decision Point:**
If Option 1 fixes 95%+ of cases → keep it.
If issues persist → implement Option 2.
