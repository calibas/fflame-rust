# Undo/Redo System Testing Checklist

**Feature Branch:** `feature/undo-redo-improvements`
**Date:** 2025-11-16
**Status:** Ready for Testing

## Test Checklist

### 1. Basic Undo/Redo Functionality
- [ ] Make a parameter change (e.g., zoom)
- [ ] Verify it appears in History panel
- [ ] Click Undo button → change reverts
- [ ] Click Redo button → change reapplies
- [ ] Keyboard shortcuts (Ctrl+Z, Ctrl+Y) work

### 2. Coalescing (Palette Changes)
- [ ] Open Palette Editor panel
- [ ] Click color picker and drag RGB sliders rapidly
- [ ] Make ~10-20 color changes within 2 seconds
- [ ] Check History panel → should show **1 entry** (not 20!)
- [ ] Undo once → all color changes revert to original
- [ ] Redo once → all color changes reapply

### 3. Coalescing Does NOT Affect Other Parameters
- [ ] Change zoom slider rapidly (should still create multiple entries if > 5s apart)
- [ ] Change exposure slider (NOT whitelisted, each change = new entry)
- [ ] Verify only Palette creates coalesced entries

### 4. History Panel UI
- [ ] Open History panel
- [ ] Make several changes
- [ ] Verify current position marked with ▶
- [ ] Verify future states grayed out (after undo)
- [ ] Verify past states normal color
- [ ] Stats show correct "Position: X (Past: Y, Future: Z)"

### 5. Truncate Future on New Change
- [ ] Make 3 changes (A, B, C)
- [ ] Undo twice (back to A)
- [ ] Verify History shows: A (current), B (future), C (future)
- [ ] Make new change D
- [ ] Verify History shows: A, D (B and C deleted)
- [ ] Verify can't redo B or C anymore

### 6. Preview Mode with Longer Interval
- [ ] Drag a slider continuously
- [ ] Should NOT create undo point immediately
- [ ] Wait 5+ seconds
- [ ] Should create undo point
- [ ] Release drag → commits final value

### 7. Preview Mode Interaction with Coalescing
- [ ] Open Palette Editor
- [ ] Drag color picker slider
- [ ] Keep dragging (preview mode active)
- [ ] Release → commits
- [ ] Drag again within 2 seconds
- [ ] Release → should coalesce with previous

### 8. History Depth Limit
- [ ] Make 500+ changes (max_undo_depth)
- [ ] Verify oldest entries removed automatically
- [ ] Verify position adjusts correctly

### 9. Snapshot-Based Undo (Presets)
- [ ] Load a preset (full config replacement)
- [ ] Undo → reverts to previous full config
- [ ] Redo → reapplies preset

### 10. Edge Cases
- [ ] Empty history state (fresh start)
- [ ] Undo when position = 0 (should do nothing)
- [ ] Redo when position = history.len() (should do nothing)
- [ ] Rapid undo/redo (no crashes)

## Expected Results

### Coalescing Behavior
**Before (without coalescing):**
- Color picker: 50+ undo entries for single color adjustment
- History panel cluttered
- Hard to navigate

**After (with coalescing):**
- Color picker: 1 undo entry per color adjustment session
- History panel clean
- Easy to navigate

### Preview Interval
**Before (500ms):**
- Undo entries created while still adjusting
- Cluttered history

**After (5000ms):**
- Undo entries created after thoughtful pause
- Better grouping of related changes

## Known Issues / Limitations

- Clicking history entries only does single undo/redo (not jump-to-position yet)
- Coalescing only enabled for `ConfigPath::Palette`
- Preview mode commit on drag end might create extra entry if > 5s elapsed

## Success Criteria

- [ ] All basic undo/redo functionality works
- [ ] Coalescing reduces palette changes to 1 entry
- [ ] History panel clearly shows timeline and position
- [ ] No crashes or unexpected behavior
- [ ] Performance is acceptable (no lag)

## Notes

(Add any observations or issues found during testing here)
