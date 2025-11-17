# Undo/Redo System Improvements

**Status:** Planning
**Created:** 2025-11-16
**Priority:** High

## Overview

Improve the undo/redo system to reduce undo point spam and provide better UX with a single unified history stack.

## Current Issues

1. **Too many undo points** - Color picker (and potentially other controls) create one undo point per change
   - Example: Dragging color picker RGB sliders creates dozens of undo entries
   - Makes undo history cluttered and hard to navigate
   - Degrades performance with excessive history entries

2. **Separate undo/redo stacks** - Current implementation uses two separate stacks
   - More complex to reason about
   - Harder to visualize in UI
   - Standard approach is single stack with position pointer

3. **Preview mode interval too short** - Currently 500ms
   - Too aggressive for some use cases
   - Should be 5000ms (5 seconds) to group related changes better

4. **History UI not optimal** - Shows separate undo/redo lists
   - Should show unified timeline with current position highlighted
   - Harder to see full history context

## Proposed Solution

### 1. Automatic Undo Point Coalescing (Opt-In)

**Problem:** Rapid succession of same-type changes create too many undo points

**Solution:** When multiple identical config path changes occur within a time window, merge them into a single undo point. **Only enabled for specific config paths** to avoid unexpected behavior.

**Algorithm:**
```rust
// Whitelist of paths that support coalescing
fn supports_coalescing(path: &ConfigPath) -> bool {
    match path {
        ConfigPath::Palette => true,  // Color picker changes
        // Add more paths here as needed:
        // ConfigPath::Exposure => true,
        // ConfigPath::Gamma => true,
        _ => false,
    }
}

// When adding new undo point:
if let Some(last_delta) = history.last() {
    let time_since_last = now - last_delta.timestamp;
    let same_path = new_delta.path == last_delta.path;
    let coalescing_enabled = supports_coalescing(&new_delta.path);

    if same_path && coalescing_enabled && time_since_last < COALESCE_WINDOW {
        // Replace last delta instead of adding new one
        history[position] = new_delta;
    } else {
        // Add as new undo point
        history.push(new_delta);
        position += 1;
    }
}
```

**Parameters:**
- `COALESCE_WINDOW` = 2000ms (2 seconds) - merge changes within this window
- Only coalesce if same `ConfigPath` (don't merge different parameters)
- Only coalesce if at head of history (position == history.len() - 1)
- **Only coalesce if path is in whitelist** (currently only `ConfigPath::Palette`)

**Initial Whitelist:**
- `ConfigPath::Palette` - Fixes color picker spam (50+ undo points → 1)

**Future Additions (as needed):**
- Tone mapping parameters (exposure, gamma, etc.)
- Camera controls (zoom, pan, rotation)
- Transform affine parameters (if needed)

**Benefits:**
- Color picker RGB changes → 1 undo point instead of 50+
- Other parameters unaffected (explicit opt-in)
- Easy to expand to more paths as needed
- Still respects preview mode (coalescing happens on commit, not during preview)

### 2. Single History Stack with Position

**Current Structure:**
```rust
pub struct ConfigManager {
    undo_stack: Vec<ConfigDelta>,  // Past states
    redo_stack: Vec<ConfigDelta>,  // Future states (if we went back)
    active_config: FractalConfig,  // Current state
}
```

**New Structure:**
```rust
pub struct ConfigManager {
    history: Vec<ConfigDelta>,      // Full history (past + future)
    position: usize,                 // Current position in history
    active_config: FractalConfig,   // Current state at position
}
```

**Operations:**
- **Undo:** `position -= 1` (apply inverse of history[position])
- **Redo:** `position += 1` (apply history[position])
- **New change:**
  - If `position < history.len() - 1`: truncate history at position (clear future)
  - Add new delta at position
  - `position += 1`

**Benefits:**
- Simpler mental model (single timeline)
- Easier to visualize in UI (one list with position marker)
- Standard approach used by most applications
- Can easily jump to arbitrary position in history (future feature)

### 3. Increase Preview Mode Interval

**Change:**
```rust
// src/config/manager.rs
const LAZY_UNDO_INTERVAL: Duration = Duration::from_millis(5000); // was 500
```

**Rationale:**
- 500ms is too aggressive - creates undo points while user is still adjusting
- 5000ms (5 seconds) groups related adjustments better
- Still responsive enough for typical workflows

### 4. Improved History Panel UI

**Current UI:**
- Shows "Undo History" with list of past changes
- Separate redo indication (if applicable)

**New UI:**
```
┌─────────────────────────────────────┐
│ History                             │
├─────────────────────────────────────┤
│ Transform 0 → Affine (a)            │
│ Transform 1 → Linear Weight         │
│ ▶ Palette → Color Stop 0            │ ← Current position
│ Palette → Color Stop 1              │
│ View → Zoom                         │
└─────────────────────────────────────┘
```

**Features:**
- Single unified list showing full history
- Current position marked with `▶` or highlight
- Items above position = can redo (go forward)
- Items below position = can undo (go back)
- Grayed out items above position (future states)
- Click any item to jump to that position (future enhancement)

## Implementation Plan

### Phase 1: Single Stack Refactor (Core Logic)
**Files:** `src/config/manager.rs`

1. Add new fields to ConfigManager:
   ```rust
   history: Vec<ConfigDelta>,
   position: usize,
   ```

2. Update `can_undo()` / `can_redo()`:
   ```rust
   pub fn can_undo(&self) -> bool { self.position > 0 }
   pub fn can_redo(&self) -> bool { self.position < self.history.len() }
   ```

3. Rewrite `undo()`:
   ```rust
   pub fn undo(&mut self) -> Result<UpdateType> {
       if self.position == 0 { return Err(...); }
       self.position -= 1;
       let delta = &self.history[self.position];
       self.apply_delta_inverse(delta)
   }
   ```

4. Rewrite `redo()`:
   ```rust
   pub fn redo(&mut self) -> Result<UpdateType> {
       if self.position >= self.history.len() { return Err(...); }
       let delta = &self.history[self.position];
       self.position += 1;
       self.apply_delta(delta)
   }
   ```

5. Update `update_param()` to truncate future on new changes:
   ```rust
   // When adding new change (not in preview mode):
   if self.position < self.history.len() {
       self.history.truncate(self.position); // Clear future
   }
   self.history.push(delta);
   self.position = self.history.len();
   ```

6. Remove old `undo_stack` and `redo_stack` fields

### Phase 2: Undo Point Coalescing
**Files:** `src/config/manager.rs`

1. Add coalescing constants:
   ```rust
   const COALESCE_WINDOW: Duration = Duration::from_millis(2000);
   ```

2. Add timestamp to ConfigDelta:
   ```rust
   pub struct ConfigDelta {
       pub path: ConfigPath,
       pub old_value: ConfigValue,
       pub new_value: ConfigValue,
       pub timestamp: std::time::Instant, // NEW
       pub description: String,
   }
   ```

3. Add coalescing whitelist function:
   ```rust
   fn supports_coalescing(path: &ConfigPath) -> bool {
       match path {
           ConfigPath::Palette => true,  // Initial use case
           _ => false,
       }
   }
   ```

4. Implement coalescing logic in `update_param()`:
   ```rust
   fn should_coalesce(&self, new_path: &ConfigPath) -> bool {
       if self.position == 0 { return false; }
       if self.position != self.history.len() { return false; } // Only at head
       if !supports_coalescing(new_path) { return false; } // Check whitelist

       let last = &self.history[self.position - 1];
       let time_since = Instant::now() - last.timestamp;

       last.path == *new_path && time_since < COALESCE_WINDOW
   }

   // In update_param (non-preview):
   if self.should_coalesce(&path) {
       // Replace last delta's new_value instead of adding new delta
       let last_idx = self.history.len() - 1;
       self.history[last_idx].new_value = new_value.clone();
       self.history[last_idx].timestamp = Instant::now();
   } else {
       // Add new delta as usual
       self.history.push(delta);
       self.position = self.history.len();
   }
   ```

### Phase 3: Increase Preview Interval
**Files:** `src/config/manager.rs`

1. Change constant:
   ```rust
   const LAZY_UNDO_INTERVAL: Duration = Duration::from_millis(5000); // was 500
   ```

### Phase 4: Update History Panel UI
**Files:** `src/ui/undo_history.rs`

1. Update `render_history_content()` to show unified list:
   ```rust
   for (i, delta) in config_manager.history().iter().enumerate() {
       let is_current = i == config_manager.position() - 1;
       let is_future = i >= config_manager.position();

       ui.horizontal(|ui| {
           if is_current {
               ui.label("▶");
           } else {
               ui.label(" ");
           }

           let text = format!("{}", delta.description);
           let color = if is_future {
               ui.style().visuals.weak_text_color() // Grayed out
           } else {
               ui.style().visuals.text_color()
           };

           ui.colored_label(color, text);
       });
   }
   ```

2. Add public accessor methods to ConfigManager:
   ```rust
   pub fn history(&self) -> &[ConfigDelta] { &self.history }
   pub fn position(&self) -> usize { self.position }
   ```

### Phase 5: Testing & Validation

1. **Unit tests** - Test new history structure:
   - Single change creates 1 entry
   - Undo/redo with position tracking
   - Truncate future on new change
   - Coalescing with same path
   - No coalescing with different paths

2. **Integration tests** - Test with actual UI:
   - Color picker creates single undo point
   - Slider drag creates single undo point
   - Multiple parameter changes create separate points
   - Undo/redo navigation works correctly

3. **Manual testing** - Verify UX:
   - History panel shows correct state
   - Undo/redo keyboard shortcuts work
   - Preview mode still functions correctly

## Migration Strategy

**Backward Compatibility:**
- No file format changes (history is runtime-only, not serialized)
- ConfigDelta structure change (add timestamp) is internal only
- All existing code using ConfigManager continues to work

**Rollout:**
1. Implement Phase 1 (single stack) first - fundamental change
2. Verify all tests pass and UI still works
3. Add Phase 2 (coalescing) - incremental improvement
4. Phase 3 (interval) - simple constant change
5. Phase 4 (UI) - polish

## Success Criteria

- [ ] Color picker RGB changes create 1 undo point (not 50+)
- [ ] Slider drags create 1 undo point per drag operation
- [ ] History panel shows unified timeline with current position
- [ ] Can undo/redo through full history
- [ ] New changes truncate future history
- [ ] All existing tests pass
- [ ] Preview mode still works correctly

## Open Questions

1. **Should we allow jumping to arbitrary history position?**
   - Click any item in history to jump there
   - More complex but more powerful
   - Defer to future enhancement?

2. **Should coalescing be configurable?**
   - User preference for coalescing window duration
   - Per-control-type coalescing rules
   - Start with fixed defaults, add preferences later if needed

3. **How to handle preview mode during coalescing?**
   - Preview changes don't create undo points (current behavior)
   - Preview commit checks if should coalesce with last committed change
   - Seems correct - preview happens between coalesced commits

4. **Should we persist history across sessions?**
   - Currently history is runtime-only (cleared on app restart)
   - Could serialize history with config files
   - Adds complexity, defer for now

## Related Issues

- Color picker preview mode (temporary workaround in place)
- Palette editor undo spam
- History panel UX improvements

## Future Enhancements

1. **Undo branching** - Track alternate timelines when making changes after undo
2. **Named snapshots** - Manually mark important states
3. **Undo groups** - Batch multiple related changes into named groups
4. **History search** - Filter/search through history
5. **Visual diff preview** - Show what will change before undo/redo
6. **Persistent history** - Save/load history with config files
