# Complete Delta-Based State Management Migration

**Status:** Planning
**Created:** 2025-10-31
**Goal:** Complete migration from old capture_state() system to ConfigManager-only approach

---

## Background

The delta-based state management system (ConfigManager) is mostly implemented and working well. However, there's still dual undo system cruft:
- Old `capture_state()` method still exists for transform structure changes
- Old `undo_history` field still exists alongside ConfigManager's undo stack
- Some controls still use flag-based change tracking instead of ConfigManager

**Current Status (as of 2025-10-31):**
- ✅ Phase 1-8: Core delta system implemented (view, color, tone mapping, presets)
- ✅ Phase 9: Removed dual undo entries for tone mapping controls
- ✅ Phase 10: Fixed lazy undo force commit bug
- ⚠️ Transform window still uses old system (add/delete/modify transforms)
- ⚠️ Old undo_history field and capture_state() method still exist

---

## Recent Fixes to Remember

### Lazy Undo Force Commit Bug (Phase 10)

**Problem:** Quick slider drags < 500ms didn't create final undo entry on mouse release.

**Root Causes:**
1. `force_commit_preview()` didn't create undo entries (just committed preview→current)
2. No helper to extract values from arbitrary FractalConfig instances
3. Input handler called `force_commit_preview(&PanX)` on ANY mouse release (wrong path!)

**Solution:**
1. Added `get_value_from_config(config, path)` helper to extract values from any config
2. Modified `force_commit_preview()` to:
   - Extract current and preview values using helper
   - Compare them with PartialEq
   - Create final undo entry if they differ
3. Added `PartialEq` to `ConfigValue`, `ToneCurve`, `Palette`, `ColorStop`
4. Removed interfering hardcoded call from `input.rs` (let UI controls handle their own force_commit)

**Key Learning:** When adding lazy undo support to new controls:
- UI control calls `force_commit_preview(&correct_path)` on drag end
- Don't call `force_commit_preview()` from global input handlers (interference!)
- Ensure ConfigValue has PartialEq for value comparison

**Files Modified:**
- `src/config/manager.rs` - Added helper, fixed force_commit
- `src/config/delta.rs` - Added PartialEq to ConfigValue
- `src/app/input.rs` - Removed interfering call
- `src/scene/palette.rs` - Added PartialEq to Palette/ColorStop
- `src/scene/tonemap.rs` - Added PartialEq to ToneCurve/CurvePoint

---

## Migration Phases

### Phase 11: Migrate Transform Add/Delete/Modify ✅ COMPLETE (2025-10-31)

**What Was Migrated:**
- Add transform → snapshot-based undo via `config_manager.load_config()`
- Delete transform → snapshot-based undo via `config_manager.load_config()`
- Config import (JSON paste) → snapshot-based undo
- Config load from file → snapshot-based undo
- Apophysis XML import → snapshot-based undo

**Implementation Pattern:**
```rust
// Create new config with modification
let mut new_config = self.config_manager.active_config().clone();
new_config.flame.transforms.push(new_transform); // or remove, etc.

// Load via ConfigManager (creates before/after snapshots internally)
if let Err(e) = self.config_manager.load_config(new_config, "Add Transform".to_string()) {
    eprintln!("Failed: {}", e);
} else {
    // Update app state from config
    self.flame = self.config_manager.active_config().flame.clone();
}
```

**Key Design Decision:**
Used `load_config()` instead of creating new ConfigPath variants because:
- Transform structure changes affect entire flame (not single parameters)
- Snapshot approach is simpler and more robust
- Same pattern as preset loading (consistent API)
- Avoids complex delta tracking for transform arrays

**Files Modified:**
- `src/app/mod.rs` - Replaced 7 `capture_state()` calls with `load_config()`
  - Line 481: Config import (JSON paste)
  - Line 522: Add transform
  - Line 539: Delete transform
  - Line 584: Load config from file
  - Line 637, 648: Apophysis XML import (single/multiple flames)
  - Line 979: Removed generic flame_changed capture

**Remaining capture_state() Call:**
- Line 550: Custom palette from editor (deferred - low frequency, separate subsystem)

**Testing:** ✅ Build successful

### Phase 12: Remove Old Undo System ✅ COMPLETE (2025-10-31)

**What Was Removed:**
- `undo_history` field from App struct (line 49)
- `undo_history` initialization (line 152)
- `capture_state()` method from App (config.rs line 131)
- `use crate::undo::UndoHistory` import
- `mod undo` declaration from lib.rs
- Last remaining `capture_state()` call (custom palette)

**Files Modified:**
- `src/app/mod.rs` - Removed field, import, initialization
- `src/app/config.rs` - Removed capture_state() method definition
- `src/lib.rs` - Removed mod undo declaration

**Orphaned Code:**
- `src/undo.rs` still exists but no longer imported
- Safe to delete in Phase 14 cleanup
- Keeping for now as historical reference

**Custom Palette Note:**
Removed capture_state() call and replaced with TODO comment.
Custom palette modifies library (not config), so needs different
approach. Low frequency operation, undo support deferred.

**Testing:** ✅ Build successful

**Result:** Old undo system completely removed! All undo/redo now
handled exclusively by ConfigManager.

### Phase 13: Final Transform Migration ✅ NOT NEEDED

**Status:** No migration needed - it's a future feature placeholder!

**Current State:**
- `final_transform` field exists in Flame struct
- Always initialized to `None`
- No UI controls exist
- Not used in rendering

**Conclusion:**
Phase 13 is not a migration task. When/if final transform UI is
added in the future, it will use ConfigManager from day one.
No old system code to migrate.

### Phase 14: Cleanup and Documentation (TODO)

**Goal:**
- Update CLAUDE.md with final state management guidelines
- Archive delta-based-state-management.md as historical reference
- Create concise guide for adding new undo-tracked controls

**Complexity:** Low
- Documentation only

---

## Control Migration Checklist

When migrating a control from old system to ConfigManager:

### For Simple Value Changes (Sliders, Dropdowns)
1. ✅ Define ConfigPath variant (if new parameter)
2. ✅ Add to ConfigManager get_value() / set_value() / set_value_in_preview()
3. ✅ Add to ConfigValue enum (if new type)
4. ✅ Add to ConfigDelta display formatting
5. ✅ Determine UpdateType for the path
6. ✅ Replace widget with lazy_slider() or use update_param()
7. ✅ Remove capture_state() call
8. ✅ Remove *_changed flag assignment (keep flag if needed for GPU side effects)

### For Lazy Undo Controls (Sliders, Triangle Editor)
1. ✅ Follow simple value checklist above
2. ✅ Use `lazy_slider()` or call `update_param(lazy=true)` during drag
3. ✅ Call `force_commit_preview(&path)` on drag end with CORRECT path
4. ✅ Don't call force_commit from global handlers (input.rs, etc.)
5. ✅ Ensure ConfigValue has PartialEq for comparison
6. ✅ Test quick drags < 500ms to ensure final undo entry created

### For Batch Operations (Multiple Related Changes)
1. ✅ Define ConfigPath variants for all affected parameters
2. ✅ Use update_params_batch() with description
3. ✅ All changes captured in single undo entry
4. ✅ Example: Transform color RGB (3 values, 1 undo entry)

### For Discrete Actions (Buttons, Checkboxes)
1. ✅ Use non-lazy mode (update_param with lazy=false)
2. ✅ Creates immediate undo entry
3. ✅ No throttling needed

---

## Common Pitfalls to Avoid

### ❌ Calling force_commit with wrong path
**Example:** Input handler calls `force_commit_preview(&PanX)` for all mouse releases
**Problem:** Interferes with other controls, compares wrong values
**Solution:** Let UI controls handle their own force_commit with correct path

### ❌ Missing PartialEq on ConfigValue types
**Example:** Adding new ConfigValue variant without PartialEq
**Problem:** force_commit_preview() can't compare values
**Solution:** Ensure all ConfigValue variants and nested types have PartialEq

### ❌ Calling capture_state() AND ConfigManager
**Example:** Slider uses ConfigManager but also sets *_changed flag for capture_state()
**Problem:** Creates duplicate undo entries
**Solution:** Remove capture_state() call and redundant flag assignment

### ❌ Not calling force_commit on drag end
**Example:** Triangle editor updates preview but doesn't call force_commit
**Problem:** Quick drags < 500ms create no undo entry
**Solution:** Always call force_commit_preview(&path) when drag ends

### ❌ Forgetting to add UpdateType
**Example:** New ConfigPath added but returns UpdateType::None
**Problem:** GPU state not updated, visual glitches
**Solution:** Determine correct UpdateType (ViewOnly, ToneMappingOnly, etc.)

---

## Testing Checklist

For each migrated control:
- [ ] Change value via UI → undo with Ctrl+Z → value reverts
- [ ] Change value → redo with Ctrl+Shift+Z → value reapplies
- [ ] Quick drag < 500ms → release → undo → entire drag captured
- [ ] Long drag > 500ms → see intermediate lazy captures in undo history
- [ ] Check undo history window shows single entries (no duplicates)
- [ ] Verify GPU state updates correctly (no visual glitches)

---

## Progress Tracking

### Completed (Phases 1-10)
- ✅ Core ConfigManager implementation
- ✅ View controls (zoom, pan, rotation, camera)
- ✅ Color controls (mode, palette, speed, background)
- ✅ Tone mapping controls (exposure, gamma, curve, etc.)
- ✅ Rendering settings (density, blend, iterations, etc.)
- ✅ Preset loading (snapshot-based undo)
- ✅ Lazy undo with throttling
- ✅ Dual undo cleanup (tone mapping)
- ✅ Lazy undo force commit bug fix

### Completed Work
- [x] Phase 11: Transform structure operations (add/delete/modify) ✅ COMPLETE (2025-10-31)
- [x] Phase 12: Remove old undo system entirely ✅ COMPLETE (2025-10-31)
- [x] Phase 13: Final transform migration ✅ NOT NEEDED (future feature, no old code)
- [x] Phase 14a: Fix ALL remaining lazy undo bugs ✅ COMPLETE (2025-10-31)
- [x] Phase 14b: Fix shader recompilation performance ✅ COMPLETE (2025-10-31)
- [x] Phase 14c: Palette editor live undo (partial) ✅ COMPLETE (2025-10-31)

### Remaining Work
- [ ] Phase 15: Palette editor complete (add/delete, import/export)
- [ ] Phase 16: Final documentation cleanup (delete undo.rs, update CLAUDE.md)

---

## Today's Major Fixes (2025-10-31 Afternoon)

### Phase 14a: Fix ALL Remaining Lazy Undo Bugs ✅

**Problem:** 17 more sliders were stuck in preview mode - never called `force_commit_preview()`

**Affected Controls:**

**settings.rs (6 sliders):**
- iterations_per_thread
- histogram_color_scale
- low_density_smoothing
- blend_factor
- density_compression_strength
- target_iterations_per_pixel

**transforms.rs (11 controls):**
- Transform weight (1 slider)
- Affine parameters a,b,c,d,e,f (6 drag values)
- Color RGB (3 sliders)
- Color speed (1 slider)

**variation_controls.rs + variation_params.rs:**
- Variation weight sliders (all 26+ variations)
- Variation parameter sliders (Float, Integer, Angle types)

**Solution:**
- Sliders: Store response, check `drag_stopped()`, call `force_commit_preview()`
- DragValues: Store response, check `lost_focus()`, call `force_commit_preview()`
- Variation params: Changed render functions to return `(changed, drag_stopped)` tuple

**Result:** ALL lazy undo sliders in the entire codebase now work correctly!

**Files Modified:**
- `src/ui/settings.rs` - Fixed 6 sliders
- `src/ui/transforms.rs` - Fixed 11 controls
- `src/ui/variation_controls.rs` - Fixed variation weight sliders
- `src/ui/variation_params.rs` - Fixed parameter sliders

### Phase 14b: Fix Shader Recompilation Performance ✅

**Problem:** Every variation weight adjustment (0.5 → 0.6) triggered full shader rebuild (~100ms stutter)

**Root Cause:**
- `ShaderCache.ensure_current()` compared `HashMap<String, f32>` for equality
- HashMap equality includes values (weights), not just keys (variation names)
- Weights don't affect shader code at all - only which variations exist

**Solution:**
- Compare only keys as HashSets: `needed.keys() == self.active_variations.keys()`
- Now only rebuilds when variations actually added/removed

**Result:**
- ✅ No shader rebuild when adjusting existing variation weights
- ✅ Still rebuilds when adding/removing variations (correct behavior)
- ✅ Smooth real-time editing without compilation stutters

**Files Modified:**
- `src/shader_cache.rs` - Changed comparison logic (lines 62-67)

### Phase 14c: Palette Editor Live Undo (Partial) ✅

**Problem:** Palette editor had Apply button, no live updates, no undo support

**Implemented:**
- Color stop position slider: Live updates with lazy undo
- Color picker: Live updates with lazy undo
- Both use preview mode and create single undo entry on release
- Fixed black frames during preview mode (palette changes now respect preview mode)

**Still TODO:**
- Add/delete color stops
- Palette name editing
- Import/export operations
- Remove Apply button (deferred to Phase 15)

**Files Modified:**
- `src/ui/palette_editor.rs` - Added lazy undo for position/color
- `src/app/mod.rs` - Fixed palette preview mode logic (line 1067)

---

## References

- **Implementation Details:** `docs/projects/delta-based-state-management.md` (2,600 lines, historical)
- **Architecture Overview:** `docs/main/CONFIG.md`
- **Lazy Undo Implementation:** `docs/projects/lazy-undo-implementation.md`
- **Code Locations:**
  - ConfigManager: `src/config/manager.rs`
  - ConfigPath/ConfigValue: `src/config/delta.rs`
  - Lazy slider helper: `src/config/slider.rs`
  - Triangle editor: `src/ui/triangle_editor.rs`

---

## Next Steps

1. Review this plan with user
2. Start Phase 11 when ready (transform operations)
3. Keep this doc updated as migration progresses
4. Archive delta-based-state-management.md when complete
