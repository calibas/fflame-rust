# Undo/Redo System Issues

## Date: 2025-11-01

## Current State

The ConfigManager implements delta-based undo/redo with a 50-state history. However, there are bugs in how it records or applies changes.

UPDATE: This may have been related to preview mode issues.

## Observed Issues

**Symptoms:**
- Undo/redo not recording all changes
- Undo not applying changes correctly
- Changes may be applied in wrong order
- History may be incomplete

**Context:**
- Issue noticed during palette system testing
- Part of broader state centralization project
- ConfigManager is in: `src/config/manager.rs`
- Delta system in: `src/config/delta.rs`

## Architecture Overview

### Current Design

```
ConfigManager
├── current: FractalConfig           // Active state
├── preview: Option<FractalConfig>   // During drag/live preview
├── history: Vec<ConfigChange>       // Undo stack (max 50)
├── history_index: usize             // Current position
└── pending_actions: UpdateAction    // What GPU updates needed
```

### Delta System

```rust
pub struct ConfigChange {
    pub deltas: Vec<ConfigDelta>,      // Batch of parameter changes
    pub description: String,            // Human-readable
    pub timestamp: Instant,             // When created
}

pub struct ConfigDelta {
    pub path: ConfigPath,               // Which parameter
    pub old_value: ConfigValue,         // Before
    pub new_value: ConfigValue,         // After
}
```

### Undo/Redo Flow

**Creating Undo Point:**
1. User changes parameter via `update_param(path, value, lazy)`
2. If lazy=false (immediate): Create undo point immediately
3. If lazy=true (preview): Throttle undo creation (500ms)
4. Record `ConfigDelta { path, old_value, new_value }`
5. Add to history, truncate redo future

**Applying Undo:**
1. User presses Ctrl+Z
2. Get current change from `history[history_index]`
3. Apply deltas in reverse: `old_value <- new_value`
4. Decrement `history_index`
5. Update GPU state

**Applying Redo:**
1. User presses Ctrl+Y
2. Increment `history_index`
3. Get change from `history[history_index]`
4. Apply deltas forward: `new_value -> current`
5. Update GPU state

## Potential Bug Areas

### 1. Delta Application Order

**Issue:** Deltas within a ConfigChange may need specific ordering
- Example: Setting palette_index clears config.palette
- If undo applies in wrong order, state may be inconsistent

**Investigation:**
- Check `apply_change()` method in manager.rs
- Verify delta application is correct order
- May need dependency tracking

### 2. Preview Mode Commits

**Issue:** Lazy updates may not commit properly
- Drag ends but undo point never created
- Preview mode stays active too long
- Multiple preview commits create duplicate undo points

**Investigation:**
- Check `force_commit_preview()` calls
- Verify throttle timer works correctly
- Check preview cleanup on window close

### 3. Batch vs Individual Changes

**Issue:** Multiple related changes may be split incorrectly
- Should be one undo point: transform add + set variations
- Instead creates multiple undo points
- Redo doesn't restore complete state

**Investigation:**
- Check batch update mechanism
- Verify `begin_batch()`/`commit_batch()` usage
- May need explicit batching API

### 4. Value Capture Timing

**Issue:** Old value may be captured incorrectly
- Value captured after change instead of before
- Preview mode changes old_value
- Race condition between UI and state update

**Investigation:**
- Check `get_value()` timing in update_param()
- Verify old_value is captured before applying change
- Check preview vs current value capture

### 5. History Truncation

**Issue:** Redo history may not truncate correctly
- Make change, undo, make different change
- Old redo future should be discarded
- May still have stale future

**Investigation:**
- Check history truncation in update_param()
- Verify `history.truncate(history_index + 1)`
- May have off-by-one error

## Testing Approach

### Reproduce Issues

**Test Case 1: Simple Parameter Change**
```
1. Note initial zoom value
2. Change zoom slider
3. Undo
4. Verify zoom restored to initial value
5. Redo
6. Verify zoom restored to changed value
```

**Test Case 2: Multiple Changes**
```
1. Change zoom
2. Change exposure
3. Undo twice
4. Verify both restored in reverse order
5. Redo twice
6. Verify both restored in forward order
```

**Test Case 3: Preview Mode**
```
1. Drag zoom slider (don't release)
2. Verify preview updates live
3. Release mouse
4. Undo
5. Verify zoom restored to value before drag
```

**Test Case 4: Complex State**
```
1. Select palette from dropdown
2. Edit color stop
3. Add color stop
4. Undo three times
5. Verify: removed stop, restored stop, restored palette selection
```

### Debug Logging

Add logging to track undo/redo operations:

```rust
// In update_param()
log::debug!("Capturing old value: {:?} = {:?}", path, old_value);
log::debug!("Setting new value: {:?} = {:?}", path, new_value);
log::debug!("Created undo point: {}", description);

// In undo()
log::debug!("Undoing: {}", change.description);
for delta in &change.deltas {
    log::debug!("  {:?}: {:?} <- {:?}", delta.path, delta.old_value, delta.new_value);
}

// In redo()
log::debug!("Redoing: {}", change.description);
```

### Assertions

Add assertions to catch bugs:

```rust
// Verify old_value matches current before applying change
debug_assert_eq!(
    self.get_value(&path)?,
    old_value,
    "Old value doesn't match current before change"
);

// Verify history_index in bounds
debug_assert!(
    self.history_index < self.history.len(),
    "History index out of bounds"
);
```

## Known Working vs Broken

**Known to Work:**
- Basic parameter changes (zoom, exposure, gamma)
- Single undo/redo cycle
- ConfigManager.capture_state() for external changes

**Known to Be Broken (or suspect):**
- Palette changes (multiple sources of confusion)
- Complex state with dependencies (palette_index + config.palette)
- Preview mode commit timing
- Batch operations

## Related Code

**Core Files:**
- `src/config/manager.rs` - ConfigManager implementation (1,237 lines)
- `src/config/delta.rs` - Delta types and update logic (568 lines)
- `src/config/slider.rs` - UI helpers with lazy undo (299 lines)

**Key Methods:**
- `update_param()` - Create undo point
- `undo()` - Apply undo
- `redo()` - Apply redo
- `force_commit_preview()` - Commit lazy changes
- `apply_to_current()` - Apply delta to current config
- `get_value()` - Read current value for delta

## Investigation Plan

### Phase 1: Add Debug Logging
- [ ] Add logging to update_param, undo, redo
- [ ] Run test cases and capture logs
- [ ] Identify which operations fail

### Phase 2: Reproduce Minimal Case
- [ ] Find smallest test case that reproduces bug
- [ ] Document exact steps
- [ ] Verify bug is consistent

### Phase 3: Fix Root Cause
- [ ] Identify which bug area from list above
- [ ] Implement fix
- [ ] Add regression test

### Phase 4: Comprehensive Testing
- [ ] Test all parameter types
- [ ] Test preview mode
- [ ] Test batch operations
- [ ] Verify no regressions

## Questions to Answer

1. **Does undo create the right deltas?**
   - Are old_value and new_value correct?
   - Are all changed parameters captured?

2. **Does undo apply deltas correctly?**
   - Are deltas applied in right order?
   - Does apply_to_current() work correctly?
   - Are GPU updates triggered?

3. **Does preview mode work?**
   - Are preview changes committed on drag end?
   - Is throttling working correctly?
   - Does force_commit_preview() get called?

4. **Are there race conditions?**
   - UI reading stale values?
   - Multiple updates happening simultaneously?
   - Preview and immediate updates conflicting?

## Success Criteria

**Undo/Redo is fixed when:**
- All test cases pass consistently
- Debug logging shows correct delta capture and application
- History accurately reflects user actions
- Preview mode commits properly on drag end
- No stuck states or inconsistencies

## References

- Original state centralization project: docs/projects/centralized-update-logic.md
- Delta migration docs: docs/archive/delta-migration/
- ConfigManager documentation: docs/main/CONFIG.md
- Related issue: Palette system redesign (docs/projects/palette-system-redesign.md)
