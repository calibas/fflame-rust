# DragValue Keyboard Input and Preview Mode Issue

**Status**: Analysis Complete
**Created**: 2025-11-01
**Priority**: High
**Affected Areas**: Triangle Editor, Transform Editor, Settings panels

## Problem Statement

When using `egui::DragValue` widgets with keyboard text input (clicking the field and typing numbers), the ConfigManager gets stuck in preview mode. This occurs during typing and persists after pressing Enter to commit the value.

The user expectation is:
- **Mouse dragging**: Use preview mode (lazy undo) - capture undo point only when drag completes
- **Keyboard input**: No preview mode - capture undo point only when Enter is pressed or focus is lost

Currently, the code treats all `changed()` events with preview mode if `dragged()` is true, but keyboard editing requires different handling.

## Root Cause Analysis

### egui Response Behavior

From egui documentation and GitHub issues research:

1. **`response.changed()`**:
   - Returns `true` for **both** dragging and keyboard input
   - Fires on every character typed during text editing
   - Fires continuously during mouse drag

2. **`response.dragged()`**:
   - Returns `true` only during mouse drag operations
   - Returns `false` during keyboard text input
   - Only works if widget senses drags (DragValue does)

3. **`response.drag_stopped()`**:
   - Returns `true` for a single frame when drag ends
   - Does **not** fire when keyboard editing completes

4. **`response.lost_focus()`**:
   - Returns `true` when widget loses keyboard focus
   - Fires when Enter is pressed in text input
   - Fires when user tabs away or clicks elsewhere
   - This is the key method we need for keyboard input

### Current Implementation Pattern

```rust
// Triangle Editor Affine Coefficients (lines 733-818)
let a_resp = ui.add(egui::DragValue::new(&mut transform.a).speed(0.01).prefix("a: "));
if a_resp.changed() {
    if let Ok(update) = config_manager.update_param(
        ConfigPath::TransformAffine { index: selected_transform, param: AffineParam::A },
        transform.a.into(),
        a_resp.dragged()  // ← This is the problem
    ) {
        max_update = max_update.max(update);
    }
}
dragging |= a_resp.dragged();
drag_stopped |= a_resp.drag_stopped();

// Later: force commit on drag_stopped
if drag_stopped && config_manager.is_in_preview_mode() {
    config_manager.force_commit_preview(&path)?;
    config_manager.reset_lazy_undo();
}
```

**Problem with this pattern**:
- When keyboard editing starts, `changed()` returns `true`, `dragged()` returns `false`
- Sends update with `lazy=false`, which should work...
- BUT: Typing multiple characters sends multiple updates with `lazy=false`
- ConfigManager might enter preview mode from the rapid updates
- When Enter is pressed, `lost_focus()` fires but `drag_stopped()` does not
- Preview mode is never committed because the commit logic only checks `drag_stopped`

## Affected Code Locations

### High Priority (Many DragValue fields)

1. **Triangle Editor** (`src/ui/triangle_editor.rs`):
   - Lines 520-541: Triangle Coordinates (6 DragValue fields)
   - Lines 734-812: Affine Coefficients (6 DragValue fields)
   - **Total**: 12 DragValue fields affected

2. **Transform Editor** (`src/ui/transforms.rs`):
   - Line 50: Z offset (g parameter)
   - Lines 146-237: Affine parameters (a, b, c, d, e, f)
   - **Total**: 7 DragValue fields affected

3. **Settings Panel** (`src/ui/settings.rs`):
   - Line 199: iterations_per_thread
   - Line 228: histogram_color_scale
   - Line 254: low_density_smoothing
   - Line 303: blend_factor
   - Line 333: density_compression_strength
   - Line 380: target_iterations_per_pixel
   - **Total**: 6 DragValue fields affected

**Grand Total**: 25+ DragValue fields across codebase

### Medium Priority (Use Sliders, less affected)

- View panel: Uses `lazy_slider()` helper which may handle this better
- Tone Mapping: Uses `config_slider()` which has different logic

## Solution Design

### Option 1: Track `lost_focus()` for Keyboard Completion

Add `lost_focus` tracking alongside `dragging` and `drag_stopped`:

```rust
let mut dragging = false;
let mut drag_stopped = false;
let mut focus_lost = false;  // New tracking

let a_resp = ui.add(egui::DragValue::new(&mut transform.a).speed(0.01).prefix("a: "));
if a_resp.changed() {
    if let Ok(update) = config_manager.update_param(
        ConfigPath::TransformAffine { index: selected_transform, param: AffineParam::A },
        transform.a.into(),
        a_resp.dragged()  // lazy=true only during drag
    ) {
        max_update = max_update.max(update);
    }
}
dragging |= a_resp.dragged();
drag_stopped |= a_resp.drag_stopped();
focus_lost |= a_resp.lost_focus();  // Track focus loss

// Commit preview on drag stop OR focus lost
if (drag_stopped || focus_lost) && config_manager.is_in_preview_mode() {
    config_manager.force_commit_preview(&path)?;
    config_manager.reset_lazy_undo();
}
```

**Pros**:
- Minimal code change
- Handles both drag and keyboard input
- Matches user expectations

**Cons**:
- Still sends updates on every keystroke (though with `lazy=false`)
- Adds another tracking variable to every DragValue section

### Option 2: Only Send Update on Focus Lost (Keyboard) or Drag Stopped (Mouse)

More aggressive approach - only send updates when editing is complete:

```rust
let mut changed = false;
let mut dragging = false;
let mut drag_stopped = false;
let mut focus_lost = false;

let a_resp = ui.add(egui::DragValue::new(&mut transform.a).speed(0.01).prefix("a: "));
changed |= a_resp.changed();
dragging |= a_resp.dragged();
drag_stopped |= a_resp.drag_stopped();
focus_lost |= a_resp.lost_focus();

// Only send update when editing completes
if changed && (drag_stopped || focus_lost) {
    if let Ok(update) = config_manager.update_param(
        ConfigPath::TransformAffine { index: selected_transform, param: AffineParam::A },
        transform.a.into(),
        false  // Never lazy - always discrete
    ) {
        max_update = max_update.max(update);
    }
}
```

**Pros**:
- Clean undo points - only when user commits
- No preview mode needed for these fields
- Fewer ConfigManager calls
- Matches user's desired behavior exactly

**Cons**:
- No live preview during dragging (user wants this for drag)
- Breaks existing drag preview functionality

### Option 3: Separate Drag and Keyboard Logic

Handle the two input methods separately:

```rust
let mut dragging = false;
let mut drag_stopped = false;

let a_resp = ui.add(egui::DragValue::new(&mut transform.a).speed(0.01).prefix("a: "));

// Handle dragging with preview
if a_resp.dragged() {
    if let Ok(update) = config_manager.update_param(
        ConfigPath::TransformAffine { index: selected_transform, param: AffineParam::A },
        transform.a.into(),
        true  // lazy during drag
    ) {
        max_update = max_update.max(update);
    }
    dragging = true;
}

// Handle keyboard input completion (no preview)
if a_resp.lost_focus() && !a_resp.dragged() {
    if let Ok(update) = config_manager.update_param(
        ConfigPath::TransformAffine { index: selected_transform, param: AffineParam::A },
        transform.a.into(),
        false  // not lazy - discrete
    ) {
        max_update = max_update.max(update);
    }
}

// Commit preview when drag stops
if a_resp.drag_stopped() && config_manager.is_in_preview_mode() {
    config_manager.force_commit_preview(&path)?;
    config_manager.reset_lazy_undo();
}
```

**Pros**:
- Clear separation of concerns
- Preserves drag preview behavior
- Keyboard input creates single undo point
- Matches user expectations exactly

**Cons**:
- More complex logic
- Duplicated update calls
- Needs careful testing to avoid double-updates

### Option 4: Helper Function Pattern

Create a reusable helper for all DragValue fields:

```rust
fn handle_drag_value_update(
    response: &egui::Response,
    path: ConfigPath,
    value: ConfigValue,
    config_manager: &mut ConfigManager,
) -> Result<UpdateType, ConfigError> {
    if response.dragged() {
        // Preview mode during drag
        config_manager.update_param(path, value, true)
    } else if response.lost_focus() {
        // Discrete update on focus lost (keyboard entry complete)
        config_manager.update_param(path, value, false)
    } else {
        Ok(UpdateType::None)
    }
}

// Usage:
let a_resp = ui.add(egui::DragValue::new(&mut transform.a).speed(0.01).prefix("a: "));
if let Ok(update) = handle_drag_value_update(
    &a_resp,
    ConfigPath::TransformAffine { index: selected_transform, param: AffineParam::A },
    transform.a.into(),
    config_manager
) {
    max_update = max_update.max(update);
}
```

**Pros**:
- DRY principle - reusable across codebase
- Clear, testable logic in one place
- Easy to apply to all 25+ fields
- Maintains both drag preview and keyboard discrete behavior

**Cons**:
- Requires helper function in appropriate module
- Still need to handle `drag_stopped` for force commit

## Recommended Solution

**Option 3 (Separate Drag and Keyboard Logic)** with refinements:

1. **For single-parameter fields** (most cases):
   - Use `dragged()` for preview updates
   - Use `lost_focus() && !dragged()` for discrete keyboard updates
   - Use `drag_stopped()` for force commit

2. **For batch updates** (Triangle Coordinates with 6 params):
   - Track `dragging` state across multiple widgets
   - Use `lost_focus` to detect keyboard completion
   - Commit on either `drag_stopped` OR `focus_lost`

### Implementation Pattern

```rust
// Pattern for single parameter:
let a_resp = ui.add(egui::DragValue::new(&mut transform.a).speed(0.01).prefix("a: "));

if a_resp.dragged() {
    // Live preview during drag
    if let Ok(update) = config_manager.update_param(path, value, true) {
        max_update = max_update.max(update);
    }
}

if a_resp.lost_focus() && !a_resp.dragged() {
    // Discrete update on keyboard completion
    if let Ok(update) = config_manager.update_param(path, value, false) {
        max_update = max_update.max(update);
    }
}

if a_resp.drag_stopped() && config_manager.is_in_preview_mode() {
    // Commit drag preview
    config_manager.force_commit_preview(&path)?;
    config_manager.reset_lazy_undo();
}
```

### Why This Works

1. **During mouse drag**:
   - `dragged()` returns true → sends update with `lazy=true`
   - Creates preview mode
   - When drag ends, `drag_stopped()` fires → force commit
   - Result: Single undo point for entire drag

2. **During keyboard input**:
   - `dragged()` returns false → no preview updates
   - User types numbers, DragValue updates internally
   - When Enter pressed, `lost_focus()` returns true
   - Sends update with `lazy=false` → discrete undo point
   - Result: Single undo point when Enter pressed

3. **Edge case - drag then keyboard**:
   - Check `!dragged()` in `lost_focus` condition prevents double-update
   - If dragging, focus lost is handled by `drag_stopped` instead

## Testing Plan

1. **Mouse Drag Test**:
   - Drag affine parameter
   - Verify single undo point created
   - Verify undo restores original value

2. **Keyboard Input Test**:
   - Click field, type new value, press Enter
   - Verify single undo point created
   - Verify NOT stuck in preview mode
   - Verify undo restores original value

3. **Tab Navigation Test**:
   - Type value, press Tab (instead of Enter)
   - Verify undo point created on focus lost
   - Verify next field gains focus

4. **Rapid Edit Test**:
   - Click field, type "1", press Enter
   - Immediately type "2", press Enter
   - Verify two discrete undo points (not merged)

5. **Batch Update Test** (Triangle Coordinates):
   - Drag one coordinate point
   - Verify all 6 affine params updated in one undo point
   - Type in coordinate field, press Enter
   - Verify all 6 affine params updated in one undo point

## Implementation Checklist

- [ ] Update Triangle Editor affine coefficients (6 fields)
- [ ] Update Triangle Editor coordinates (6 fields)
- [ ] Update Transform Editor z offset (1 field)
- [ ] Update Transform Editor affine params (6 fields)
- [ ] Update Settings panel DragValues (6 fields)
- [ ] Test mouse drag behavior (should be unchanged)
- [ ] Test keyboard input completion (should fix preview mode issue)
- [ ] Test tab navigation
- [ ] Update any other DragValue fields found
- [ ] Document pattern in CLAUDE.md or architecture docs

## References

- egui Issue #2687: "During keyboard input, only update a DragValue value when focus is lost"
- egui Issue #2877: "Keyboard navigation: focus change loses new value in DragValue"
- egui Response docs: https://docs.rs/egui/latest/egui/response/struct.Response.html
- ConfigManager: `src/config/manager.rs`
- Triangle Editor: `src/ui/triangle_editor.rs` lines 733-827

## Related Issues

- Preview mode getting stuck is a symptom of not handling `lost_focus()`
- May affect other DragValue uses in the codebase not yet identified
- Consider creating a helper function/macro if pattern becomes repetitive
