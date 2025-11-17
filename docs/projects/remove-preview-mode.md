# Remove Preview Mode - Simplify Config Update System

## Status
**PLANNED** - Not yet started

## Overview

Remove the entire preview mode system and rely on coalescing + renderer accumulation strategy for smooth real-time updates.

## Current System (Overcomplicated)

**Preview Mode:**
- Creates shadow `preview: Option<FractalConfig>` during lazy updates
- Shows live updates without committing to history
- Commits on throttle (5 second intervals) or force commit
- Tracks `preview_needs_overwrite` flag
- Complex state management: current vs preview vs active_config()

**Lazy Undo:**
- Throttles history capture during continuous input (sliders, drags)
- 5 second delay before creating undo point
- Intended to reduce undo spam

**Problems:**
1. Stuck preview mode (fixed but still fragile)
2. Blank frame when exiting preview (accumulation reset)
3. Complex state synchronization (200+ lines of preview logic)
4. Difficult to reason about (which config is "real"?)
5. Lazy parameter everywhere (`update_param(..., lazy: bool)`)

## Proposed System (Simpler)

**No Preview Mode:**
- All changes apply to `current` immediately
- No shadow config, no preview tracking
- ConfigManager always has single source of truth

**Coalescing Handles Rapid Changes:**
- Already implemented (2 second window)
- Merges rapid changes to same parameter into single undo point
- Whitelist: Palette, future additions as needed
- Example: Moving slider creates 1 undo point, not 50

**Renderer Provides Visual Smoothness:**
- `UpdateType::IterationReset` → Reset accumulation (clear and restart)
- `UpdateType::ColorOnly` → Reset accumulation (recolor existing samples)
- `UpdateType::ToneMappingOnly` → Continue accumulation (post-processing)
- `UpdateType::ViewOnly` → Reset accumulation (camera moved)
- No blank frame - accumulation handles transition smoothly

**Modify Sessions (Unchanged):**
- Still needed for structural changes (triangle editor)
- Suppresses coalescing during session
- Creates snapshot on commit
- Already works without preview mode

## Benefits

1. **Simplicity**: Remove ~200 lines of preview logic
2. **Reliability**: No stuck preview mode issues
3. **Performance**: No blank frames, smooth accumulation transitions
4. **Clarity**: Single config, obvious state
5. **Real-time**: All changes visible immediately
6. **Maintainability**: Less code, fewer edge cases

## Technical Details

### What Gets Removed

**ConfigManager fields:**
```rust
preview: Option<FractalConfig>,           // DELETE
preview_needs_overwrite: bool,             // DELETE
```

**ConfigManager methods:**
```rust
fn set_value_in_preview()                  // DELETE
pub fn is_in_preview_mode()                // DELETE
pub fn force_commit_preview()              // DELETE
```

**update_param() complexity:**
- Remove entire lazy path (lines 267-337)
- Keep only immediate path (simplified)
- Remove `lazy: bool` parameter

**update_batch() complexity:**
- Remove entire lazy path (lines 375-461)
- Keep only immediate path (simplified)
- Remove `lazy: bool` parameter

### What Gets Simplified

**update_param() - After:**
```rust
pub fn update_param(
    &mut self,
    path: ConfigPath,
    new_value: ConfigValue,
) -> Result<UpdateType, ConfigError> {
    // Special case: modify session (skip history, commit immediately)
    if self.modify_session.is_some() {
        self.set_value(&path, new_value)?;
        let update_type = path.update_type();
        self.record_action(update_type);
        return Ok(update_type);
    }

    // Normal mode: update current and capture
    let old_value = self.get_value(&path)?;

    if old_value.approx_eq(&new_value) {
        return Ok(UpdateType::None);
    }

    let delta = ConfigDelta::new(path.clone(), old_value, new_value.clone());
    let change = ConfigChange::single(delta);
    let update_type = change.update_type();

    self.push_undo(change);  // Coalescing happens here
    self.set_value(&path, new_value)?;
    self.record_action(update_type);

    Ok(update_type)
}
```

**update_batch() - After:**
```rust
pub fn update_batch(
    &mut self,
    changes: Vec<(ConfigPath, ConfigValue)>,
    description: String,
) -> Result<UpdateType, ConfigError> {
    // Special case: modify session (skip history, commit immediately)
    if self.modify_session.is_some() {
        for (path, value) in changes {
            self.set_value(&path, value)?;
        }
        let update_type = UpdateType::IterationReset;  // Assume worst case
        self.record_action(update_type);
        return Ok(update_type);
    }

    // Normal mode: create deltas and capture
    let mut deltas = Vec::new();
    for (path, new_value) in changes {
        let old_value = self.get_value(&path)?;
        if !old_value.approx_eq(&new_value) {
            deltas.push(ConfigDelta::new(path, old_value, new_value));
        }
    }

    if deltas.is_empty() {
        return Ok(UpdateType::None);
    }

    let change = ConfigChange::batch(deltas, description);
    let update_type = change.update_type();

    self.push_undo(change.clone());  // Batch changes skip coalescing

    for delta in &change.deltas {
        self.set_value(&delta.path, delta.new_value.clone())?;
    }

    self.record_action(update_type);
    Ok(update_type)
}
```

**active_config() - After:**
```rust
pub fn active_config(&self) -> &FractalConfig {
    &self.current  // Always just current, no preview
}
```

### UI Call Site Changes

**Before:**
```rust
// Slider (continuous input)
if slider.changed() {
    config_manager.update_param(path, value.into(), slider.dragged())?;
}
if slider.drag_stopped() && config_manager.is_in_preview_mode() {
    config_manager.force_commit_preview(&path)?;
}
```

**After:**
```rust
// Slider (continuous input) - coalescing handles it
if slider.changed() {
    config_manager.update_param(path, value.into())?;
}
// No force_commit needed - coalescing merges rapid changes
```

**Before:**
```rust
// Button (discrete input)
if button.clicked() {
    config_manager.update_param(path, value.into(), false)?;
}
```

**After:**
```rust
// Button (discrete input) - same as slider now
if button.clicked() {
    config_manager.update_param(path, value.into())?;
}
```

## Migration Plan

### Phase 1: Remove Preview Plumbing ✅
**Files:** `src/config/manager.rs`

1. Remove `preview` and `preview_needs_overwrite` fields
2. Delete `set_value_in_preview()` method
3. Delete `is_in_preview_mode()` method
4. Delete `force_commit_preview()` method
5. Update `active_config()` to return `&self.current`
6. Update constructor to remove preview initialization

**Testing:** Should compile (UI won't work yet)

### Phase 2: Simplify update_param() ✅
**Files:** `src/config/manager.rs`

1. Remove `lazy: bool` parameter
2. Delete entire lazy update path (lines 267-337)
3. Keep only immediate update logic
4. Add modify session check at top
5. Simplify to: get old → create delta → push undo → set value

**Testing:** Should compile (UI calls still have wrong signature)

### Phase 3: Simplify update_batch() ✅
**Files:** `src/config/manager.rs`

1. Remove `lazy: bool` parameter
2. Delete entire lazy update path (lines 375-461)
3. Keep only immediate update logic
4. Add modify session check at top
5. Simplify to: create deltas → push undo → apply values

**Testing:** Should compile (UI calls still have wrong signature)

### Phase 4: Update All UI Call Sites ✅
**Files:** All UI modules

1. Remove `lazy` parameter from all `update_param()` calls
2. Remove `response.dragged()` logic
3. Delete all `force_commit_preview()` calls
4. Delete all `is_in_preview_mode()` checks
5. Delete all `drag_stopped` handlers related to preview

**Affected files:**
- `src/ui/settings.rs`
- `src/ui/transforms.rs`
- `src/ui/view.rs`
- `src/ui/tone_mapping.rs`
- `src/ui/palette_editor.rs`
- `src/ui/triangle_editor.rs` (already done for modify sessions)
- `src/ui/panel_viewer.rs` (pan drag)
- `src/app/input.rs` (keyboard pan)

**Testing:** Should compile and run

### Phase 5: Clean Up Documentation ✅
**Files:** Various docs

1. Update `CLAUDE.md` to remove preview mode references
2. Update `docs/main/CONFIG.md` to remove lazy undo docs
3. Update ConfigManager doc comments
4. Update UI patterns in code comments

**Testing:** Documentation review

### Phase 6: Verify Coalescing Works ✅
**Manual testing:**

1. Move slider rapidly → should create 1 undo point
2. Edit palette → should coalesce color changes
3. Click buttons repeatedly → should create multiple undo points
4. Triangle editor → should still create single snapshot
5. Undo/redo through all scenarios

**Success criteria:**
- Smooth real-time updates (no lag, no blank frames)
- Reasonable undo history (not spammed, but not missing changes)
- Coalescing works for continuous input
- Modify sessions still work correctly

## Risks and Mitigations

### Risk: Too many undo points for sliders
**Mitigation:** Expand coalescing whitelist to include common slider paths
- Exposure, Gamma, Zoom, Rotation, etc.
- Test and add as needed

### Risk: Coalescing window too short
**Mitigation:** Increase from 2s to 3s or 5s if needed
- Current 2s should be fine for continuous dragging
- Easy to adjust if users complain

### Risk: Performance impact from immediate updates
**Mitigation:** Already doing this in modify sessions, works fine
- GPU uploads are fast
- Accumulation handles visual smoothness

### Risk: Breaking existing workflows
**Mitigation:** Preserve behavior with coalescing
- Rapid changes still merge (like old lazy undo)
- Just happens automatically instead of manually

## Open Questions

1. **Coalescing whitelist**: Which paths should support coalescing?
   - Current: Palette
   - Candidates: Exposure, Gamma, Zoom, Rotation, Pan, all variation weights?
   - Strategy: Start conservative, add based on user feedback

2. **Coalescing window**: Keep 2 seconds or adjust?
   - 2s seems reasonable for continuous slider dragging
   - Can experiment with 3s or 5s

3. **Modify session throttling**: Should modify sessions also commit on throttle?
   - Current: Commit only on drag end
   - Consideration: Very long drags might benefit from intermediate commits
   - Decision: Keep current behavior (commit on end only)

## Success Metrics

1. **Code reduction**: Remove ~200 lines of preview logic
2. **Bug reduction**: Zero stuck preview mode issues
3. **Performance**: No blank frames, smooth 60 FPS
4. **UX**: Real-time updates feel responsive
5. **Undo quality**: History is clean and predictable

## References

- Original preview mode: Implemented for lazy undo throttling
- Coalescing: Already implemented in `should_coalesce()` (lines 700-738)
- Modify sessions: Proof that immediate updates work well
- UpdateType: Determines accumulation strategy (reset vs continue)
