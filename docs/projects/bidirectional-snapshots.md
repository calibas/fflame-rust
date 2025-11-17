# Bidirectional Snapshot System

## Current State

The undo/redo system uses two strategies:

### 1. Delta-based (Efficient) ✅
Used for normal parameter changes (zoom, pan, exposure, variation weights, etc.)

```rust
pub struct ConfigDelta {
    pub path: ConfigPath,
    pub old_value: ConfigValue,
    pub new_value: ConfigValue,
    pub timestamp: Instant,
}
```

**Storage:** ~100 bytes per change
**Undo:** Apply inverted delta (old_value)
**Redo:** Apply forward delta (new_value)

### 2. Snapshot-based (Inefficient) ❌
Used for structural changes (Add Transform, Delete Transform, Load Preset)

```rust
pub struct ConfigChange {
    pub deltas: Vec<ConfigDelta>,
    pub snapshot: Option<Box<FractalConfig>>,  // Entire config (~many KB)
    // ...
}
```

**Current behavior:**
- `load_config()` creates **TWO** separate undo entries:
  1. "Before: Add Transform" with full config snapshot
  2. "After: Add Transform" with full config snapshot
- Each snapshot stores entire FractalConfig (all transforms, palette, tone mapping, view, etc.)

**Problems:**
- Wasteful: "Add Transform" stores 2× full configs when only 1 transform changed
- Cluttered: Two entries in history for single operation
- Inconsistent: "Before:" and "After:" naming is confusing

---

## Proposed Solution

### Bidirectional Snapshots

Store both before/after states in a **single** ConfigChange, with specialized types for different operations.

## Design

### 1. New SnapshotData Enum

```rust
/// Specialized snapshot data (stores only what changed)
#[derive(Debug, Clone)]
pub enum SnapshotData {
    /// Full config replacement (preset loading, file import)
    FullConfig {
        before: Box<FractalConfig>,
        after: Box<FractalConfig>,
    },

    /// Transform added
    AddTransform {
        index: usize,           // Where it was inserted
        transform: Transform,   // The transform that was added
    },

    /// Transform deleted
    DeleteTransform {
        index: usize,           // Where it was removed
        transform: Transform,   // What was deleted
    },

    // Future extensions:
    // AddFinalTransform { transform: FinalTransform },
    // DeleteFinalTransform { transform: FinalTransform },
    // PaletteChange { old: Palette, new: Palette },
}
```

### 2. Updated ConfigChange

```rust
pub struct ConfigChange {
    pub deltas: Vec<ConfigDelta>,
    pub timestamp: Instant,
    pub description: String,

    /// Snapshot data (bidirectional or specialized)
    /// If Some: use snapshot logic for undo/redo
    /// If None: use deltas for undo/redo
    pub snapshot: Option<SnapshotData>,
}
```

### 3. New Constructors

```rust
impl ConfigChange {
    /// Create full config snapshot (preset loading)
    pub fn full_config_snapshot(
        before: FractalConfig,
        after: FractalConfig,
        description: String,
    ) -> Self {
        Self {
            deltas: vec![],
            timestamp: Instant::now(),
            description,
            snapshot: Some(SnapshotData::FullConfig {
                before: Box::new(before),
                after: Box::new(after),
            }),
        }
    }

    /// Create add transform snapshot
    pub fn add_transform_snapshot(
        index: usize,
        transform: Transform,
        description: String,
    ) -> Self {
        Self {
            deltas: vec![],
            timestamp: Instant::now(),
            description,
            snapshot: Some(SnapshotData::AddTransform { index, transform }),
        }
    }

    /// Create delete transform snapshot
    pub fn delete_transform_snapshot(
        index: usize,
        transform: Transform,
        description: String,
    ) -> Self {
        Self {
            deltas: vec![],
            timestamp: Instant::now(),
            description,
            snapshot: Some(SnapshotData::DeleteTransform { index, transform }),
        }
    }
}
```

---

## Implementation Plan

### Phase 1: Add SnapshotData enum and constructors
**Files:** `src/config/delta.rs`

1. Add `SnapshotData` enum with variants:
   - `FullConfig { before, after }`
   - `AddTransform { index, transform }`
   - `DeleteTransform { index, transform }`

2. Change `ConfigChange::snapshot` field:
   ```rust
   // OLD:
   pub snapshot: Option<Box<FractalConfig>>,

   // NEW:
   pub snapshot: Option<SnapshotData>,
   ```

3. Add new constructor methods:
   - `full_config_snapshot()`
   - `add_transform_snapshot()`
   - `delete_transform_snapshot()`

4. Remove old `snapshot()` constructor (will break compilation - fix in Phase 2)

**Testing:** Compilation will fail - proceed to Phase 2

---

### Phase 2: Update undo/redo logic
**Files:** `src/config/manager.rs`

#### Update `undo()` method

```rust
pub fn undo(&mut self) -> Result<UpdateType, ConfigError> {
    // ... position checks ...

    let change = &self.history[self.position];

    // Check if this is a snapshot-based undo
    if let Some(snapshot) = &change.snapshot {
        match snapshot {
            SnapshotData::FullConfig { before, .. } => {
                log::debug!("  Restoring full config snapshot (before)");
                self.current = (**before).clone();
                return Ok(UpdateType::IterationReset);
            }

            SnapshotData::AddTransform { index, .. } => {
                log::debug!("  Undoing add transform at index {}", index);
                if *index < self.current.flame.transforms.len() {
                    self.current.flame.transforms.remove(*index);
                }
                return Ok(UpdateType::IterationReset);
            }

            SnapshotData::DeleteTransform { index, transform } => {
                log::debug!("  Undoing delete transform (re-insert at index {})", index);
                if *index <= self.current.flame.transforms.len() {
                    self.current.flame.transforms.insert(*index, transform.clone());
                }
                return Ok(UpdateType::IterationReset);
            }
        }
    }

    // Delta-based undo (existing logic)
    // ...
}
```

#### Update `redo()` method

```rust
pub fn redo(&mut self) -> Result<UpdateType, ConfigError> {
    // ... position checks ...

    let change = self.history[self.position].clone();

    // Check if this is a snapshot-based redo
    if let Some(snapshot) = &change.snapshot {
        match snapshot {
            SnapshotData::FullConfig { after, .. } => {
                log::debug!("  Restoring full config snapshot (after)");
                self.current = (**after).clone();
                self.position += 1;
                return Ok(UpdateType::IterationReset);
            }

            SnapshotData::AddTransform { index, transform } => {
                log::debug!("  Redoing add transform at index {}", index);
                if *index <= self.current.flame.transforms.len() {
                    self.current.flame.transforms.insert(*index, transform.clone());
                }
                self.position += 1;
                return Ok(UpdateType::IterationReset);
            }

            SnapshotData::DeleteTransform { index, .. } => {
                log::debug!("  Redoing delete transform (remove at index {})", index);
                if *index < self.current.flame.transforms.len() {
                    self.current.flame.transforms.remove(*index);
                }
                self.position += 1;
                return Ok(UpdateType::IterationReset);
            }
        }
    }

    // Delta-based redo (existing logic)
    // ...
}
```

**Testing:** Compilation will fail - proceed to Phase 3

---

### Phase 3: Update load_config() to use FullConfig snapshot
**Files:** `src/config/manager.rs`

Replace the two-snapshot approach with single bidirectional snapshot:

```rust
pub fn load_config(&mut self, new_config: FractalConfig, description: String) -> Result<(), ConfigError> {
    // Clear any preview state
    self.preview = None;
    self.preview_needs_overwrite = false;

    // Create single bidirectional snapshot
    let change = ConfigChange::full_config_snapshot(
        self.current.clone(),  // before
        new_config.clone(),    // after
        description,
    );

    self.push_undo(change);

    // Replace current config
    self.current = new_config;

    // Record full config import action
    let mut action = UpdateAction::none();
    action.update_flame = true;
    action.update_view = true;
    action.update_palette = true;
    action.update_tone_curve = true;
    action.reset_accumulation = true;
    self.pending_actions.merge(&action);

    Ok(())
}
```

**Testing:** Build should succeed - test preset loading undo/redo

---

### Phase 4: Update Add Transform to use specialized snapshot
**Files:** `src/app/mod.rs`

Replace `load_config()` call with specialized snapshot:

```rust
// In handle_add_transform (around line 500):
if ui_response.add_transform {
    let config = self.config_manager.active_config();

    // Create new transform
    let mut new_transform = Transform::new();
    new_transform.variations.insert("linear".to_string(), 1.0);
    new_transform.color = 0.5;
    new_transform.color_speed = 0.5;

    let insert_index = config.flame.transforms.len();

    // Create snapshot BEFORE modifying config
    let change = ConfigChange::add_transform_snapshot(
        insert_index,
        new_transform.clone(),
        "Add Transform".to_string(),
    );

    // Apply change to current config
    self.config_manager.config_mut().flame.transforms.push(new_transform);

    // Record the snapshot
    if let Err(e) = self.config_manager.push_undo_external(change) {
        eprintln!("Failed to record add transform: {}", e);
    }

    // Update app state
    self.flame = self.config_manager.active_config().flame.clone();

    // Record action for GPU updates
    self.config_manager.record_action_external(UpdateType::IterationReset);
}
```

**Note:** Need to add public methods:
- `config_manager.push_undo_external(change)`
- `config_manager.record_action_external(update_type)`

Or better: Add a new method to ConfigManager:

```rust
impl ConfigManager {
    /// Apply a structural change (transform add/delete)
    pub fn apply_structural_change(&mut self, change: ConfigChange) -> Result<(), ConfigError> {
        // Apply the change based on snapshot type
        if let Some(snapshot) = &change.snapshot {
            match snapshot {
                SnapshotData::AddTransform { index, transform } => {
                    if *index <= self.current.flame.transforms.len() {
                        self.current.flame.transforms.insert(*index, transform.clone());
                    }
                }
                SnapshotData::DeleteTransform { index, .. } => {
                    if *index < self.current.flame.transforms.len() {
                        self.current.flame.transforms.remove(*index);
                    }
                }
                SnapshotData::FullConfig { after, .. } => {
                    self.current = (**after).clone();
                }
            }
        }

        // Record in history
        self.push_undo(change);

        // Record GPU update action
        self.record_action(UpdateType::IterationReset);

        Ok(())
    }
}
```

Then usage becomes:

```rust
if ui_response.add_transform {
    let insert_index = self.config_manager.active_config().flame.transforms.len();

    let mut new_transform = Transform::new();
    new_transform.variations.insert("linear".to_string(), 1.0);
    new_transform.color = 0.5;
    new_transform.color_speed = 0.5;

    let change = ConfigChange::add_transform_snapshot(
        insert_index,
        new_transform,
        "Add Transform".to_string(),
    );

    if let Err(e) = self.config_manager.apply_structural_change(change) {
        eprintln!("Failed to add transform: {}", e);
    } else {
        self.flame = self.config_manager.active_config().flame.clone();
    }
}
```

**Testing:** Test add transform + undo/redo

---

### Phase 5: Update Delete Transform to use specialized snapshot
**Files:** `src/app/mod.rs`

Similar to Phase 4, but for delete:

```rust
if let Some(idx) = ui_response.delete_transform {
    let config = self.config_manager.active_config();

    if config.flame.transforms.len() > 1 && idx < config.flame.transforms.len() {
        // Get the transform before deleting
        let deleted_transform = config.flame.transforms[idx].clone();

        let change = ConfigChange::delete_transform_snapshot(
            idx,
            deleted_transform,
            format!("Delete Transform {}", idx + 1),
        );

        if let Err(e) = self.config_manager.apply_structural_change(change) {
            eprintln!("Failed to delete transform: {}", e);
        } else {
            self.flame = self.config_manager.active_config().flame.clone();
        }
    }
}
```

**Testing:** Test delete transform + undo/redo

---

### Phase 6: Update should_coalesce() check
**Files:** `src/config/manager.rs`

The current check for empty deltas will still work, but make it more explicit:

```rust
fn should_coalesce(&self, new_change: &ConfigChange) -> bool {
    // Never coalesce snapshots
    if new_change.snapshot.is_some() {
        return false;
    }

    // Never coalesce if no deltas (safety check)
    if new_change.deltas.is_empty() {
        return false;
    }

    // ... rest of coalescing logic ...
}
```

**Testing:** Verify palette coalescing still works

---

## Memory Impact Analysis

### Before (Current System)

**Add Transform:**
- 2 undo entries × 1 FractalConfig each = ~2× full config size
- FractalConfig size estimate: ~50 KB (32 transforms × ~1 KB each + other fields)
- **Total: ~100 KB per Add Transform**

**Delete Transform:**
- Same as Add Transform
- **Total: ~100 KB per Delete Transform**

### After (New System)

**Add Transform:**
- 1 undo entry with `AddTransform { index, transform }`
- Transform size: ~1 KB
- **Total: ~1 KB per Add Transform** (99% reduction!)

**Delete Transform:**
- 1 undo entry with `DeleteTransform { index, transform }`
- **Total: ~1 KB per Delete Transform** (99% reduction!)

**Load Preset:**
- 1 undo entry with `FullConfig { before, after }`
- Still stores 2× full configs (necessary for full replacement)
- **Total: ~100 KB per Load Preset** (same as before, but only 1 entry instead of 2)

---

## Benefits Summary

1. ✅ **Memory efficiency:** 99% reduction for Add/Delete Transform
2. ✅ **Cleaner history:** Single entry instead of "Before:" + "After:" pair
3. ✅ **Clear intent:** Snapshot variant shows exactly what happened
4. ✅ **Extensible:** Easy to add more specialized snapshots (FinalTransform, etc.)
5. ✅ **Backward compatible:** Delta-based changes unaffected
6. ✅ **Type safe:** Match arms ensure correct undo/redo logic

---

## Testing Checklist

### Basic Functionality
- [ ] Add Transform creates single history entry
- [ ] Delete Transform creates single history entry
- [ ] Load Preset creates single history entry

### Undo/Redo
- [ ] Undo Add Transform removes the transform
- [ ] Redo Add Transform restores the transform at same index
- [ ] Undo Delete Transform restores the transform at original index
- [ ] Redo Delete Transform removes it again
- [ ] Undo/Redo Load Preset swaps entire config correctly

### Edge Cases
- [ ] Add transform, modify it, undo → transform gone (not reverted to original)
- [ ] Delete transform at index 0 (first)
- [ ] Delete transform at last index
- [ ] Undo/redo chain: Add → Delete → Add → Undo × 3
- [ ] History panel shows correct single entries

### Performance
- [ ] Adding 10 transforms uses ~10 KB instead of ~1 MB
- [ ] History with 50 entries stays responsive

---

## Future Extensions

Potential additional specialized snapshots:

```rust
pub enum SnapshotData {
    // ... existing variants ...

    /// Final transform added
    AddFinalTransform {
        transform: FinalTransform,
    },

    /// Final transform deleted
    DeleteFinalTransform {
        transform: FinalTransform,
    },

    /// Palette replaced (can't represent as ConfigPath)
    ReplacePalette {
        old_palette: Palette,
        new_palette: Palette,
    },

    /// Multiple transforms reordered
    ReorderTransforms {
        old_order: Vec<usize>,  // Original indices
        new_order: Vec<usize>,  // New indices
    },
}
```

---

## Migration Notes

### Breaking Changes
None - this is purely internal refactoring. Existing delta-based changes continue to work unchanged.

### Deprecations
- Old `ConfigChange::snapshot()` constructor replaced by specialized constructors
- `load_config()` behavior changes from 2 entries to 1 entry

### Testing Strategy
1. Implement Phase 1-3 (core infrastructure)
2. Test with Load Preset first (full config snapshot)
3. Implement Phase 4-5 (specialized snapshots)
4. Run full regression test suite
5. Manual testing with complex undo/redo sequences
