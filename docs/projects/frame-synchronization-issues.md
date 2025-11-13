# Frame Synchronization Issues - Analysis and Fix

**Date:** 2025-11-13
**Status:** Analysis Complete - Implementation Pending
**Priority:** High (affects undo/redo, preview mode, UI consistency)

---

## Problem Statement

Race conditions and timing issues between UI updates, ConfigManager state, and GPU rendering cause:
- Undo/redo applying wrong values
- Preview mode flickering or stuck states
- UI controls getting "stuck"
- Inconsistent rendering between what UI shows and what's displayed

**Root Cause:** Frame processing order allows stale state to be read/written, creating temporal coupling bugs.

---

## Current Frame Order (BUGGY)

```
Frame N:
1. Get config from ConfigManager (line 309)
2. Compute pass (uses config from step 1)
3. Accumulate pass
4. Tonemap pass (updates some GPU params from config)
5. Render UI (line 394)
   - UI reads from ConfigManager
   - UI modifies ConfigManager via update_param()
   - ConfigManager sets pending_actions flags
6. Process pending_actions (line 1006-1083)
   - Get config AGAIN from ConfigManager (line 1020)
   - Update GPU buffers based on actions
   - Submit GPU commands
7. Clear pending_actions (line 1083)
8. Present frame (line 1089)
```

### The Race Conditions

**Problem 1: Config Read Twice**
```
Line 309:  config = get_config()  // Read at START of frame
Line 1020: config = get_config()  // Read at END of frame (after UI changes)
```
- GPU compute/accumulate use OLD config (from line 309)
- GPU update pass uses NEW config (from line 1020)
- Result: One frame lag between UI change and visible effect

**Problem 2: UI Modifies State Mid-Frame**
```
Frame N:
  309: config = old_value
  394: UI runs, user drags slider
       -> ConfigManager.update_param() changes config
       -> Pending actions set
  1020: config = new_value
  1083: Clear pending actions
```
- Compute pass at top of frame uses old_value
- Update pass at bottom uses new_value
- But both happen in SAME frame submit
- Result: Temporal inconsistency

**Problem 3: Preview Mode Confusion**
```
Frame N:
  325: in_preview = is_in_preview_mode()
  326: set_overwrite_mode(in_preview)  // GPU uses this for compute
  394: UI runs, user releases slider
       -> force_commit_preview() called
       -> Preview mode EXITS
  1020: Get config (preview now committed)
```
- GPU thinks it's in preview mode (overwrite enabled)
- But config is already committed (preview exited)
- Result: Next frame incorrectly overwrites committed changes

**Problem 4: Undo/Redo Timing**
```
User presses Ctrl+Z:
  Frame N input: Undo key detected
  Frame N render:
    309: config = current state
    Compute uses current state
    394: UI calls undo()
         -> ConfigManager reverts to old state
    1020: config = old state
    GPU update pass applies old state
  Frame N+1:
    309: config = old state
    Compute FINALLY uses old state
```
- Visual feedback delayed by 1 frame
- During that frame, stale state is computed

---

## Desired Frame Order (SYNCHRONIZED)

```
Frame N:
1. Poll input events (keyboard, mouse)
2. Process input BEFORE UI
   - Handle keyboard shortcuts (undo/redo)
   - Handle view changes (arrow keys, mouse drag)
3. Render UI
   - Read current config
   - User interactions update ConfigManager
   - ConfigManager sets pending_actions
4. Process ALL pending actions
   - Force commit any preview mode changes
   - Apply undo/redo
   - Update flame working copy from ConfigManager
5. Get FINAL config for this frame (after all changes)
6. Update ALL GPU state to match final config
   - Update buffers
   - Update uniforms
   - Reset accumulation if needed
7. GPU Render Pipeline
   - Compute pass (uses final config)
   - Accumulate pass
   - Tonemap pass
8. Present frame
9. Clear temporary flags (view_changed_by_keyboard, etc.)
```

### Key Principles

**Principle 1: Single Config Read**
- Read config ONCE per frame, after all updates
- No stale config references

**Principle 2: UI Changes Complete Before Rendering**
- All ConfigManager updates happen before GPU pipeline
- Preview commits happen before compute pass

**Principle 3: Synchronous Updates**
- No deferred GPU updates
- Everything ready before render pipeline starts

**Principle 4: Frame Boundaries**
- Clear frame: Start → Input → UI → Updates → Render → Present
- No state carries over between frames

---

## Implementation Plan

### Phase 1: Restructure render() Function

**Current Structure:**
```rust
fn render() {
    config = get_config();  // ❌ TOO EARLY
    compute_pass(config);
    ui = render_ui();
    process_ui_response();
    config2 = get_config();  // ❌ DUPLICATE READ
    apply_gpu_updates(config2);
    present();
}
```

**New Structure:**
```rust
fn render() {
    // 1. Render UI first (reads current state, applies changes)
    ui_response = render_ui();

    // 2. Process ALL state changes
    process_ui_response();
    force_commit_preview_if_needed();
    sync_flame_from_config();

    // 3. Get FINAL config (after all updates)
    config = get_config();

    // 4. Prepare GPU state (before any rendering)
    update_all_gpu_state(config);

    // 5. Render pipeline (uses consistent config)
    compute_pass(config);
    accumulate_pass();
    tonemap_pass(config);

    // 6. Present
    present();
}
```

### Phase 2: Separate Preparation from Rendering

**New Helper Method:**
```rust
fn prepare_frame_state(&mut self, ui_response: UiResponse) {
    // Force commit preview mode if drag ended
    if ui_response.preview_drag_ended {
        self.config_manager.force_commit_preview_all();
    }

    // Sync working flame copy with ConfigManager
    self.flame = self.config_manager.active_config().flame.clone();

    // Clear frame-local flags
    self.view_changed_by_keyboard = false;
}
```

**New GPU Update Method:**
```rust
fn update_gpu_for_frame(&mut self, encoder: &mut CommandEncoder) {
    let config = self.config_manager.active_config();
    let actions = self.config_manager.get_pending_actions();

    if let Some(renderer) = &mut self.flame_renderer {
        // Apply ALL pending updates atomically
        if actions.update_flame {
            renderer.update_flame(..., &config);
        }
        if actions.update_view {
            renderer.update_iterations(..., &config);
        }
        if actions.update_palette {
            renderer.update_palette(..., &config);
        }
        if actions.reset_accumulation {
            renderer.reset(encoder, ...);
        }
    }

    self.config_manager.clear_pending_actions();
}
```

### Phase 3: Fix Preview Mode Lifecycle

**Current Problem:**
```rust
// At frame start
let in_preview = is_in_preview_mode();  // ❌ Can change during frame

// Later in frame
render_ui();  // User releases slider -> exits preview
// But GPU already set overwrite mode!
```

**Solution:**
```rust
fn render() {
    // 1. Capture preview state BEFORE UI
    let preview_at_frame_start = self.config_manager.is_in_preview_mode();

    // 2. Render UI (may exit preview)
    ui_response = render_ui();

    // 3. Force commit if preview ended
    if preview_at_frame_start && !self.config_manager.is_in_preview_mode() {
        // Preview was active, now ended - changes already committed
    }

    // 4. Check preview state AFTER all updates
    let preview_for_this_frame = self.config_manager.is_in_preview_mode();

    // 5. Set GPU mode based on CURRENT state
    renderer.set_overwrite_mode(preview_for_this_frame);
}
```

### Phase 4: Synchronize Undo/Redo

**Current Problem:**
```rust
// Undo happens mid-frame in UI
ui.undo() -> ConfigManager.undo()
// But compute already used old config!
```

**Solution:**
```rust
// Process undo BEFORE rendering
fn handle_input_events() {
    if undo_key_pressed {
        self.config_manager.undo();
        self.sync_flame_from_config();
    }
}

fn render() {
    // Config already rolled back before frame starts
    config = get_config();
    render_pipeline(config);  // Uses correct state
}
```

---

## Code Changes Required

### File: `src/app/mod.rs`

**Change 1: Move config read to after UI**
```rust
// REMOVE this early read (line 309)
- let config = self.config_manager.active_config();

// ADD single read after all updates (before render pipeline)
+ let config = self.config_manager.active_config();
```

**Change 2: Reorder render() sections**
```rust
fn render() {
    // OLD ORDER:
    // 1. compute, 2. UI, 3. process updates

    // NEW ORDER:
    // 1. UI, 2. process updates, 3. sync state, 4. compute
}
```

**Change 3: Add sync helper**
```rust
fn sync_flame_from_config(&mut self) {
    self.flame = self.config_manager.active_config().flame.clone();
}
```

### File: `src/config/manager.rs`

**Change 1: Add force_commit_all()**
```rust
pub fn force_commit_preview_all(&mut self) {
    if self.is_in_preview_mode() {
        // Commit all pending preview changes
        self.commit_preview();
    }
}
```

**Change 2: Add frame boundary marker**
```rust
pub fn end_frame(&mut self) {
    // Called at end of frame to ensure preview commits
    if self.preview_uncommitted_for > Duration::from_millis(16) {
        self.force_commit_preview_all();
    }
}
```

---

## Testing Plan

### Test 1: Undo/Redo Consistency
```
1. Change zoom from 1.0 to 2.0
2. Undo
3. VERIFY: Zoom shows 1.0 in UI AND fractal displays at 1.0
4. Redo
5. VERIFY: Zoom shows 2.0 in UI AND fractal displays at 2.0
```

**Expected:** No 1-frame lag, immediate visual update

### Test 2: Preview Mode Quality
```
1. Drag zoom slider slowly
2. VERIFY: No flickering, smooth updates
3. Release slider
4. VERIFY: Quality improves (accumulation resumes)
5. Undo
6. VERIFY: Returns to pre-drag state
```

**Expected:** Consistent behavior, no stuck states

### Test 3: Rapid Parameter Changes
```
1. Rapidly drag exposure slider back and forth
2. VERIFY: No tearing, glitching, or stuck values
3. Release slider
4. VERIFY: Settles to final value smoothly
```

**Expected:** GPU and UI stay synchronized

### Test 4: Multiple Changes in One Frame
```
1. Press Ctrl+Z (undo)
2. Immediately press arrow key (view change)
3. VERIFY: Both changes apply correctly
4. No stuck states or race conditions
```

**Expected:** All changes process atomically

---

## Success Criteria

**Frame Synchronization Fixed When:**
- [ ] Config read ONCE per frame (after all updates)
- [ ] UI changes apply before render pipeline
- [ ] Preview mode commits properly before compute
- [ ] Undo/redo shows immediate visual effect (no lag)
- [ ] No flickering or stuck states
- [ ] GPU state matches UI state every frame
- [ ] All test cases pass consistently

---

## Risks and Mitigations

### Risk 1: Performance Impact
**Risk:** Moving UI before compute may change frame timing
**Mitigation:** UI already runs every frame, just reordering. Measure with metrics.

### Risk 2: Breaking Existing Behavior
**Risk:** Code assumes certain order
**Mitigation:** Comprehensive testing, git bisect if regressions found

### Risk 3: Preview Mode Edge Cases
**Risk:** Complex preview commit logic may have bugs
**Mitigation:** Add extensive debug logging, test thoroughly

---

## Related Issues

- `docs/projects/undo-redo-issues.md` - Original bug report
- `docs/projects/centralized-update-logic.md` - State centralization
- `docs/projects/dragvalue-keyboard-preview-mode.md` - Preview mode analysis

---

## Implementation Status

- [x] Analysis complete
- [x] Frame order documented
- [x] Race conditions identified
- [ ] Code changes implemented
- [ ] Tests passing
- [ ] Performance verified
- [ ] Ready to merge

---

**Next Steps:**
1. Implement new render() order
2. Add sync helpers
3. Test all scenarios
4. Measure performance impact
5. Document changes
