# State Centralization: ConfigManager as Single Source of Truth

**Status:** In Progress (Branch: state-centralization)
**Created:** 2025-10-31
**Updated:** 2025-10-31
**Goal:** Eliminate state duplication between App and ConfigManager, centralize all update logic

---

## Current Architecture Problems

### Problem 1: Split Logic Across Multiple Layers

**Currently scattered across:**
1. **UI Layer** - Sets 30+ boolean flags in `UiResponse`
2. **App Layer** - Interprets flags to decide: reset? update palette? update tone curve?
3. **ConfigManager** - Has `UpdateType` enum but it's **not used** by app layer

**Example of current mess (app/mod.rs:1067):**
```rust
let should_reset = ui_response.reset_requested
    || view_changed
    || (ui_response.palette_changed && !in_preview_mode)
    || ui_response.color_mode_changed
    || ui_response.background_color_changed
    || ui_response.tonemap_mode_changed
    || ui_response.histogram_color_scale_changed
    || ui_response.low_density_smoothing_changed
    || ui_response.density_compression_changed
    || ui_response.blend_factor_changed
    || ui_response.use_dynamic_blend_changed
    || ui_response.target_iterations_changed
    || ui_response.preset_changed
    || (ui_response.flame_changed && !in_preview_mode);
```

**Problems:**
- Hard to maintain (30+ flags!)
- Easy to forget flags (palette bug we just fixed)
- Logic duplicated (preview mode checks in multiple places)
- No single source of truth

### Problem 2: UpdateType Exists But Isn't Used

**ConfigManager already has this:**
```rust
pub enum UpdateType {
    None,            // No update needed
    ViewOnly,        // Just update view transform (zoom, pan, rotation)
    ToneMappingOnly, // Re-run tonemap pass (exposure, gamma)
    ColorOnly,       // Re-run color accumulation (palette, color mode)
    IterationReset,  // Full reset - clear accumulation, restart iterations
}
```

**And each ConfigPath knows its update type:**
```rust
impl ConfigPath {
    pub fn update_type(&self) -> UpdateType {
        match self {
            ConfigPath::Zoom | ConfigPath::PanX => UpdateType::ViewOnly,
            ConfigPath::Exposure | ConfigPath::Gamma => UpdateType::ToneMappingOnly,
            ConfigPath::Palette(_) | ConfigPath::ColorMode => UpdateType::ColorOnly,
            ConfigPath::TransformWeight { .. } => UpdateType::IterationReset,
            // ... etc
        }
    }
}
```

**But the app layer ignores this and uses UiResponse flags instead!**

### Problem 3: Preview Mode Logic Scattered

**Preview mode checks appear in:**
- Line 311: `renderer.set_overwrite_mode(in_preview_mode)`
- Line 987: `if ui_response.flame_changed || in_preview_mode`
- Line 1067: `(ui_response.palette_changed && !in_preview_mode)`
- Line 1076: `(ui_response.flame_changed && !in_preview_mode)`

**Each one is slightly different, easy to miss one (palette bug)**

### Problem 4: State Duplication Between App and ConfigManager

**App struct (lines 28-68) duplicates ~30 fields from FractalConfig:**
- View: `zoom`, `pan_x`, `pan_y`, `rotation`, `camera_rotation_x/y`
- Rendering: `density_scale`, `speed_factor`, `iterations_per_thread`, `speed_multiplier`
- Color: `color_mode`, `current_palette_index`, `background_color`
- Tone mapping: `tonemap_mode`, `tonemap_curve`, `use_curve`, `exposure`, `gamma`
- Histogram/blend: `histogram_color_scale`, `low_density_smoothing`, etc.

**Consequences:**
- Manual synchronization required (line 964-972: preset loading syncs 8+ fields)
- Potential for inconsistent state (App fields vs ConfigManager)
- UI receives mutable references to App fields AND mutates ConfigManager
- Renderer reads from App fields instead of canonical state
- Cannot trust any single source as "ground truth"

---

## Proposed Architecture

### Vision: ConfigManager as Single Source of Truth

**Architecture Flow:**
```
UI Layer (egui controls)
  ↓
  Reads from ConfigManager (no mutable references to App fields)
  Updates via ConfigManager.update_param(), load_config()
  ↓
ConfigManager (ONLY state holder)
  • Owns FractalConfig (canonical state)
  • Tracks all changes with deltas
  • Knows UpdateType for each change
  • Knows if in preview mode
  • Provides UpdateAction (what needs updating)
  ↓
App Layer (thin orchestration)
  • NO duplicate fields (removed ~30 fields!)
  • Reads state from ConfigManager
  • Asks: "What needs updating?" → gets UpdateAction
  • Executes rendering actions
  • Manages GPU resources only
  ↓
Renderer (pure execution)
  • Receives parameters from App
  • No state, no decisions
```

**Key Changes:**
1. **App struct shrinks** - Remove all duplicate config fields
2. **Single source of truth** - ConfigManager.active_config() is THE state
3. **No synchronization** - Can't have inconsistent state when there's only one copy
4. **Centralized logic** - UpdateAction replaces 30+ boolean flags

### Proposed API

#### 1. New UpdateAction Struct

```rust
/// What actions need to be taken after config changes
pub struct UpdateAction {
    pub reset_accumulation: bool,      // Clear buffers, restart from scratch
    pub update_flame: bool,            // Update flame parameters on GPU
    pub update_palette: bool,          // Update palette texture
    pub update_tone_curve: bool,       // Update tone curve LUT
    pub update_view: bool,             // Update view transform (zoom/pan/rotation)
    pub rebuild_shader: bool,          // Recompile shader (variation changes)
    pub needs_import: bool,            // Full config import needed (preset load, etc)
}
```

#### 2. ConfigManager Provides Actions

```rust
impl ConfigManager {
    /// Get actions needed based on recent changes
    /// Call this once per frame after all UI updates
    pub fn get_pending_actions(&self) -> UpdateAction {
        // Look at changes since last frame
        // Consider preview mode state
        // Calculate optimal set of actions
        // Return consolidated UpdateAction
    }

    /// Clear pending actions (call after executing them)
    pub fn clear_pending_actions(&mut self) {
        self.pending_actions = UpdateAction::none();
    }
}
```

#### 3. Simplified App Logic

**Current (200+ lines of scattered logic):**
```rust
// Handle 30+ different flags
if ui_response.palette_changed { ... }
if ui_response.tonemap_curve_changed { ... }
if ui_response.flame_changed { ... }
// ... repeat for each flag
let should_reset = ui_response.reset_requested || view_changed || ...;
```

**Proposed (10 lines):**
```rust
let actions = self.config_manager.get_pending_actions();

if actions.reset_accumulation {
    renderer.reset(...);
}
if actions.update_flame {
    renderer.update_flame(...);
}
if actions.update_palette {
    renderer.update_palette(...);
}
// ... etc

self.config_manager.clear_pending_actions();
```

---

## Benefits

### 1. Single Source of Truth
- All "what needs updating" logic lives in ConfigManager
- UI just calls `update_param()`, doesn't decide consequences
- App just executes actions, doesn't decide what's needed

### 2. Automatic Preview Mode Handling
- ConfigManager knows if in preview mode
- Automatically adjusts actions (no reset during preview)
- No scattered `!in_preview_mode` checks

### 3. Optimal Action Consolidation
- Multiple palette changes in one frame → single update
- Conflicting updates merged intelligently
- Example: Palette + Transform change → just reset (includes palette update)

### 4. Easier to Maintain
- Add new parameter? Just set its UpdateType in ConfigPath
- ConfigManager automatically handles it correctly
- No hunting through app.rs for scattered flags

### 5. Fix Category of Bugs
- **Palette bug we just fixed** - Would never happen (ConfigManager handles preview)
- **Missing update flags** - Impossible, UpdateType is mandatory
- **Inconsistent behavior** - Single code path for all parameters

---

## Implementation Phases

### Phase 1: Add UpdateAction Infrastructure (Low Risk)
**Goal:** Build the mechanism without breaking existing code

- [ ] Create `UpdateAction` struct in `config/manager.rs`
- [ ] Add `pending_actions: UpdateAction` field to ConfigManager
- [ ] Implement `get_pending_actions()` method that analyzes recent changes
- [ ] Implement `clear_pending_actions()` method
- [ ] **Don't change app.rs or UI yet** - parallel infrastructure

**Testing:** Compiles, existing functionality unchanged

### Phase 2: Remove State Duplication from App (High Risk)
**Goal:** Eliminate duplicate fields, force all reads through ConfigManager

**2a. Identify Fields to Remove:**
- [ ] List all App fields that duplicate FractalConfig
- [ ] Identify fields that must stay (GPU resources, UI state, metrics)
- [ ] Document which fields go where

**2b. Remove Duplicate Fields:**
- [ ] Remove view fields: `zoom`, `pan_x`, `pan_y`, `rotation`, `camera_rotation_x/y`
- [ ] Remove rendering fields: `density_scale`, `speed_factor`, `iterations_per_thread`, `speed_multiplier`
- [ ] Remove color fields: `color_mode`, `current_palette_index`, `background_color`
- [ ] Remove tonemap fields: `tonemap_mode`, `tonemap_curve`, `use_curve`, `exposure`, `gamma`
- [ ] Remove histogram/blend fields: `histogram_color_scale`, `low_density_smoothing`, etc.
- [ ] Keep: `flame` (working copy for renderer), `flame_renderer`, `gpu`, `egui_layer`, `metrics`, libraries, UI state

**2c. Update App::run() initialization:**
- [ ] Remove field initialization for removed fields
- [ ] ConfigManager already initialized with FractalConfig

**Testing:** Won't compile yet - need Phase 3

### Phase 3: Refactor Rendering to Read from ConfigManager (High Risk)
**Goal:** All rendering code reads canonical state

**3a. Update render() method:**
- [ ] Replace `self.zoom` → `config.zoom` (read from ConfigManager)
- [ ] Replace `self.iterations_per_thread` → `config.iterations_per_thread`
- [ ] Replace all other field references throughout render()

**3b. Update helper methods:**
- [ ] Update `export_config()` to read from ConfigManager
- [ ] Update `load_config_with_undo()` to not sync App fields (ConfigManager is source)

**Testing:** Should compile and run, but UI won't work (still has mutable refs)

### Phase 4: Refactor UI Layer (Highest Risk)
**Goal:** Remove all mutable references to removed App fields

**4a. Update render_ui() signature:**
- [ ] Remove ~25 mutable reference parameters
- [ ] Keep only: `config_manager`, `flame_renderer`, `flame` (working copy), libraries, pause/max_iterations (UI state)
- [ ] UI reads from `config_manager.active_config()`
- [ ] UI writes via `config_manager.update_param()`

**4b. Update all UI panels:**
- [ ] Update view controls (zoom, pan, rotation)
- [ ] Update rendering controls (iterations, density, speed)
- [ ] Update color controls (color mode, palette, background)
- [ ] Update tonemap controls (mode, curve, exposure, gamma)
- [ ] Update histogram/blend controls

**Testing:** Full functionality should work

### Phase 5: Replace UiResponse Flags with UpdateAction (Medium Risk)
**Goal:** Centralize "what needs updating" logic

**5a. Implement get_pending_actions():**
- [ ] Track changes since last call
- [ ] Consider preview mode (suppress reset during preview)
- [ ] Merge multiple UpdateTypes intelligently
- [ ] Return consolidated UpdateAction

**5b. Replace App update logic:**
- [ ] Replace `if ui_response.palette_changed` → `if actions.update_palette`
- [ ] Replace massive `should_reset` calculation → `if actions.reset_accumulation`
- [ ] Remove all UiResponse config-related flags
- [ ] Keep non-config flags: export/import requests, add/delete transform

**Testing:** All functionality works, cleaner code

### Phase 6: Final Cleanup (Low Risk)

- [ ] Remove unused UiResponse fields
- [ ] Add comprehensive documentation to ConfigManager
- [ ] Update CLAUDE.md with new architecture
- [ ] Update docs/main/CONFIG.md if needed
- [ ] Run full test suite (unit + regression)
- [ ] Manual testing of all features

---

## Edge Cases to Handle

### 1. Multiple Changes in One Frame

**Example:** Drag transform slider while in preview mode

**Current behavior:** Each slider call creates lazy preview
**Needed behavior:** Consolidate into single update action

**Solution:**
```rust
impl ConfigManager {
    fn get_pending_actions(&self) -> UpdateAction {
        // Accumulate UpdateTypes from all changes since last frame
        // Merge them (IterationReset > ColorOnly > ToneMappingOnly > ViewOnly)
        // Consider preview mode (suppress reset if in preview)
        // Return consolidated actions
    }
}
```

### 2. Preview Mode Transitions

**Example:** User drags slider (preview starts) then releases (preview ends)

**Needed behavior:**
- During drag: Overwrite mode, no reset
- On release: Commit change, still no reset (overwrite handled it)

**Solution:**
```rust
impl ConfigManager {
    fn get_pending_actions(&self) -> UpdateAction {
        let in_preview = self.is_in_preview_mode();
        let mut actions = UpdateAction::from_update_type(self.pending_update_type);

        // Suppress reset during preview (overwrite mode handles it)
        if in_preview && actions.reset_accumulation {
            actions.reset_accumulation = false;
            actions.update_flame = true; // Still need to update GPU params
        }

        actions
    }
}
```

### 3. Preset Loading

**Example:** User loads preset (massive config change)

**Needed behavior:** Full reset + import, not individual updates

**Solution:**
```rust
// load_config() already creates snapshot, not individual deltas
impl ConfigManager {
    pub fn load_config(&mut self, config: FractalConfig) {
        // ... existing logic ...
        self.pending_import = true; // Flag for app to do full import
    }
}
```

### 4. Undo/Redo

**Example:** User undos palette change

**Needed behavior:** Same as if they made the change manually

**Solution:** Already works! Undo creates a ConfigChange, which has UpdateType

---

## Risks and Mitigations

### Risk 1: Breaking Existing Behavior

**Mitigation:** Gradual migration, one subsystem at a time
- Start with tone mapping (simplest)
- Validate each step before moving on
- Keep old flags working until migration complete

### Risk 2: Performance (Calculating Actions Every Frame)

**Mitigation:** Very cheap calculation
- No GPU work, just enum comparisons
- Only runs if ConfigManager changed this frame
- Can cache result until next change

### Risk 3: Preview Mode Complexity

**Mitigation:** All preview logic centralized in one place
- ConfigManager.get_pending_actions() handles it
- No scattered checks in app layer
- Easier to reason about and test

---

## Alternatives Considered

### Alternative 1: Keep UiResponse Flags, Just Clean Them Up

**Pros:** Less refactoring
**Cons:** Doesn't solve root problem (scattered logic)
**Verdict:** Band-aid, not a fix

### Alternative 2: Move Logic to Renderer

**Pros:** Renderer knows what it needs
**Cons:** Renderer shouldn't decide when to update (separation of concerns)
**Verdict:** Wrong layer

### Alternative 3: Keep Current System, Document Better

**Pros:** Zero work
**Cons:** Still hard to maintain, bugs will keep happening
**Verdict:** Doesn't solve the problem

---

## Questions to Resolve

### Q1: Should ConfigManager track "pending since last frame"?

**Option A:** Track changes since `get_pending_actions()` was last called
- Pro: Automatic consolidation of multiple changes
- Con: Need to track "last consumed" state

**Option B:** Calculate actions from current vs last committed state
- Pro: Simpler state management
- Con: Might miss transient states

**Recommendation:** Option A - better for multi-frame preview interactions

### Q2: What about non-config UI actions? (export, import, etc)

**Keep in UiResponse:**
- Export/import file dialogs
- Manual reset button
- Preset selection (triggers load_config)

**These aren't config parameters, just user actions**

**Result:** UiResponse shrinks from 30+ fields to ~10 action fields

### Q3: Should UpdateAction be per-frame or accumulated?

**Per-frame (recommended):**
- ConfigManager calculates actions based on changes this frame
- App executes actions immediately
- ConfigManager.clear_pending_actions() called after execution

**Accumulated:**
- ConfigManager accumulates actions until app asks
- More complex state management

**Recommendation:** Per-frame, simpler and sufficient

---

## Recommendation

**Status:** APPROVED - Implementation in progress (Branch: state-centralization)

**Why it's worth it:**
1. **Eliminates entire class of bugs** - State duplication, sync issues, palette preview, missing flags
2. **True single source of truth** - ConfigManager is THE state, no duplication
3. **Makes future changes trivial** - Add param to ConfigPath, automatic undo/update handling
4. **Reduces app.rs complexity** - ~30 duplicate fields removed, ~200 lines of logic simplified
5. **Centralizes all decision logic** - ConfigManager knows everything, App just executes

**Estimated effort:**
- Phase 1 (UpdateAction infrastructure): 2-3 hours
- Phase 2 (Remove duplicate App fields): 1-2 hours
- Phase 3 (Refactor rendering): 2-3 hours
- Phase 4 (Refactor UI layer): 4-6 hours (most complex)
- Phase 5 (Replace UiResponse flags): 2-3 hours
- Phase 6 (Cleanup + testing): 2-3 hours
- **Total: 13-20 hours** for major architectural improvement

**Risk level:** High (touching core state management), mitigated by:
- Incremental approach (6 phases, test each)
- Can rollback via git branch
- Comprehensive test suite to catch regressions

**Impact:** Transformational - this is the "right" architecture we should have had from the start

---

## Example: Before vs After

### Before (Current)

**UI Layer:**
```rust
// palette_editor.rs
if slider.changed() {
    config_manager.update_param(...);
    *palette_changed = true; // ← Manual flag
}
```

**App Layer:**
```rust
// app/mod.rs (lines 1048-1082)
if ui_response.palette_changed {
    let palette = /* complex logic */;
    renderer.update_palette(...);
}

let should_reset = ui_response.reset_requested
    || (ui_response.palette_changed && !in_preview_mode) // ← Easy to forget !in_preview_mode
    || ui_response.color_mode_changed
    || /* 10+ more conditions */;

if should_reset {
    renderer.reset(...);
}
```

### After (Proposed)

**UI Layer:**
```rust
// palette_editor.rs
if slider.changed() {
    config_manager.update_param(...);
    // That's it! No manual flags
}
```

**App Layer:**
```rust
// app/mod.rs
let actions = self.config_manager.get_pending_actions();

if actions.update_palette {
    let palette = self.config_manager.active_palette();
    renderer.update_palette(&self.gpu.device, &self.gpu.queue, palette);
}

if actions.reset_accumulation {
    renderer.reset(...); // Preview mode already handled by ConfigManager
}

self.config_manager.clear_pending_actions();
```

**Much cleaner, harder to get wrong!**
