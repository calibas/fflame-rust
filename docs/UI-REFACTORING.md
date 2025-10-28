# UI Code Refactoring Plan

**Date:** 2025-10-27
**Branch:** `refactor/ui-helpers`
**Status:** Planning → Implementation

---

## Executive Summary

Current UI code in `src/ui/` suffers from massive code duplication (~1000+ lines), brittle parameter passing (44+ parameters per function), and lack of reusable components. This document outlines a refactoring plan to improve maintainability, reduce bugs, and accelerate feature development.

**Key Metrics:**
- **Code Reduction:** ~1000 lines eliminated (40% reduction in transforms.rs)
- **Maintenance Burden:** 5+ edit locations → 1-2 locations per new feature
- **Bug Risk:** Significantly reduced through elimination of copy-paste errors

---

## Current Issues

### 1. Massive Code Duplication in Variation Parameter Rendering

**Location:** `src/ui/transforms.rs`
**Lines:** 118-162, 178-222, 240-284, 300-344, 360-404

The same ~50-line parameter rendering block is copy-pasted **4 times** for each variation category:
- Basic 2D Variations (lines 118-162)
- Advanced 2D Variations (lines 178-222)
- 3D Depth Variations (lines 240-284)
- 3D Rotation Variations (lines 300-344)
- Full 3D Variations (lines 360-404)

**Total Duplication:** ~200 lines × 4 copies = **800 lines of duplicated code**

**Impact:**
- Bug fixes require updating 4+ locations
- Adding new parameter types requires updating 12+ match arms
- High risk of copy-paste errors creating inconsistent behavior

### 2. Repetitive Change Tracking Pattern

**Every UI element follows this pattern:**
```rust
if ui.add(egui::Slider::new(value, range).text("Label")).changed() {
    *some_changed = true;
}
```

**Occurrences:** 200+ times across all UI files

**Impact:**
- Verbose and repetitive
- Easy to forget the `.changed()` check
- Inconsistent hover text and formatting across similar controls

### 3. Massive Function Signatures

**Example:** `render_settings_window` has **44 parameters** (settings.rs:6-44)

```rust
pub fn render_settings_window(
    ctx: &egui::Context,
    show_settings: &mut bool,
    show_config_window: &mut bool,
    _show_palette_editor: &mut bool,
    can_undo: bool,
    can_redo: bool,
    undo_requested: &mut bool,
    redo_requested: &mut bool,
    // ... 36 more parameters
)
```

**Impact:**
- Every new feature requires modifying 3+ locations:
  - Function signature
  - Call site in `mod.rs`
  - All intermediate function signatures
- Extremely brittle to changes
- IDE autocomplete becomes nearly useless

### 4. Response Struct Bloat

**Location:** `src/ui/response.rs`
**Fields:** 46 boolean/option fields

**Adding one new control requires modifying 4 locations:**
1. Declare field in struct (line ~42)
2. Initialize in `Default::impl` (line ~90)
3. Set in `mod.rs::render_ui()` (line ~140)
4. Return in struct constructor (line ~420)

**Impact:**
- High maintenance burden
- Easy to miss one location during updates
- No logical grouping makes code hard to navigate

### 5. No Helper Functions

Despite egui's builder pattern being designed for composition, there are **zero reusable UI helper functions** in the codebase. Every slider, button, and drag value is manually constructed inline.

**Impact:**
- No consistency guarantees across UI
- Reinventing the wheel for common patterns
- Missed opportunities for standardized behavior (hover text, formatting, validation)

---

## Proposed Solutions

### Solution 1: UI Helper Module

**File:** `src/ui/helpers.rs` (new)

**Components:**
- `TrackedSlider` - Builder pattern slider with automatic change tracking
- `tracked_button()` - Button with change tracking
- `angle_slider_radians()` - Angle slider (degrees UI, radians storage)
- `zoom_controls()` - Consistent zoom UI pattern
- `drag_value_tracked()` - DragValue with change tracking
- `checkbox_tracked()` - Checkbox with change tracking

**Benefits:**
- Consistent behavior across all UI controls
- Single source of truth for formatting, ranges, hover text
- Reduces 10-15 lines → 2-3 lines per control

**Example:**
```rust
// Before (10 lines)
ui.label("Exposure");
if ui.add(egui::Slider::new(exposure, 0.1..=5.0).text("Exposure")).changed() {
    *exposure_changed = true;
}

// After (2 lines)
ui.label("Exposure");
TrackedSlider::new(exposure, 0.1..=5.0).text("Exposure").show(ui, exposure_changed);
```

### Solution 2: Variation Parameter Rendering Module

**File:** `src/ui/variation_params.rs` (new)

**Components:**
- `render_variation_params()` - Unified parameter rendering for all variation types
- `render_float_param()` - Float parameter slider
- `render_integer_param()` - Integer parameter slider
- `render_angle_param()` - Angle parameter slider

**Benefits:**
- Eliminates 800 lines of duplication
- Bug fixes apply everywhere automatically
- Adding new parameter types requires updating 1 function, not 12+ match arms

### Solution 3: Variation Category Rendering

**File:** `src/ui/variation_controls.rs` (new)

**Components:**
- `render_variation_category()` - Generic category renderer
- Works with any `VariationCategory` enum value

**Benefits:**
- Reduces transforms.rs from ~400 lines → ~50 lines
- Adding new variation categories requires 1 line of code
- Guaranteed consistent behavior across all categories

**Example:**
```rust
// Before (100+ lines per category × 5 categories)
ui.separator();
ui.label("Basic 2D Variations");
let basic_2d = crate::variations::global_registry().by_category(VariationCategory::Basic2D);
for var_info in basic_2d {
    let mut value = transform.get_variation(&var_info.name);
    if ui.add(egui::Slider::new(&mut value, 0.0..=2.0).text(&var_info.display_name)).changed() {
        // ... 40+ more lines
    }
}
// Repeat 4 more times

// After (5 lines total)
render_variation_category(ui, transform, VariationCategory::Basic2D, "Basic 2D Variations", flame_changed);
render_variation_category(ui, transform, VariationCategory::Advanced2D, "Advanced 2D Variations", flame_changed);
if matches!(flame.render_mode, RenderMode::ThreeD) {
    render_variation_category(ui, transform, VariationCategory::Depth3D, "3D Depth Variations", flame_changed);
    render_variation_category(ui, transform, VariationCategory::Rotation3D, "3D Rotation Variations", flame_changed);
    render_variation_category(ui, transform, VariationCategory::Full3D, "Full 3D Variations", flame_changed);
}
```

### Solution 4: Parameter Grouping Structs

**Files:** `src/ui/params/*.rs` (new directory)

**Components:**
- `ViewParams` - Zoom, pan, rotation, camera rotation
- `RenderParams` - Iterations, histogram scale, density smoothing, blend settings
- `TonemapParams` - Tonemap mode, curve, exposure, gamma, background
- `FileOperations` - Export, import, undo, redo flags
- `StateChanges` - Preset changes, transform additions/deletions, mode changes

**Benefits:**
- Function signatures: 44 parameters → 6-8 parameter groups
- Logical grouping improves code readability
- Can pass `&mut params.render` to sub-functions
- Adding fields only requires updating struct definition and one call site

**Example:**
```rust
// Before
pub fn render_settings_window(
    ctx: &egui::Context,
    show_settings: &mut bool,
    show_config_window: &mut bool,
    // ... 41 more parameters
) { }

// After
pub fn render_settings_window(
    ctx: &egui::Context,
    show_settings: &mut bool,
    file_ops: &mut FileOperations,
    render_params: &mut RenderParams,
    tonemap_params: &mut TonemapParams,
    preset_library: &PresetLibrary,
    current_preset_index: &mut usize,
    flame: &mut Flame,
) { }
```

### Solution 5: Restructured Response Struct

**File:** `src/ui/response.rs` (modified)

**Components:**
```rust
pub struct UiResponse {
    pub view: ViewChanges,
    pub render: RenderChanges,
    pub color: ColorChanges,
    pub file: FileOperations,
    pub state: StateChanges,
}
```

**Benefits:**
- Logical grouping makes intent clear
- Adding fields requires updating 1-2 locations instead of 4+
- Can return sub-structs from sub-functions
- Easier to test individual change categories

---

## Implementation Plan

### Phase 1: Foundation (High Priority)
**Goal:** Eliminate the worst duplication, establish patterns

1. ✅ Create branch `refactor/ui-helpers`
2. ✅ Document findings and plan (this document)
3. ✅ Commit documentation
4. Create `src/ui/helpers.rs` with core helper functions
5. Create `src/ui/variation_params.rs` with parameter rendering
6. Create `src/ui/variation_controls.rs` with category rendering
7. Update `src/ui/mod.rs` to re-export helpers
8. **Test:** Ensure compilation succeeds, no functionality changes

### Phase 2: Migrate Transforms Window (High Priority)
**Goal:** Demonstrate maximum impact with minimal risk

1. Refactor `src/ui/transforms.rs` to use new helpers
2. **Expected:** 428 lines → ~100 lines (76% reduction)
3. **Test:** Verify all variation categories render correctly
4. **Test:** Verify parameter sliders work for all parameter types
5. **Test:** Verify 2D/3D mode visibility switching
6. Commit with detailed before/after metrics

### Phase 3: Migrate Settings Window (Medium Priority)
**Goal:** Improve maintainability of most complex window

1. Create `src/ui/params/` directory
2. Create parameter grouping structs:
   - `src/ui/params/view_params.rs`
   - `src/ui/params/render_params.rs`
   - `src/ui/params/tonemap_params.rs`
   - `src/ui/params/mod.rs`
3. Refactor `src/ui/settings.rs` to use helpers and param structs
4. Update call sites in `src/ui/mod.rs`
5. **Test:** Verify all settings controls work correctly
6. Commit with before/after metrics

### Phase 4: Migrate Other Windows (Medium Priority)
**Goal:** Apply helpers consistently across all UI

1. Refactor `src/ui/view.rs` to use helpers
2. Refactor `src/ui/tone_mapping.rs` to use helpers
3. Refactor `src/ui/palette_editor.rs` to use helpers
4. **Test:** Full regression test of all UI windows
5. Commit each file individually

### Phase 5: Restructure Response (Low Priority)
**Goal:** Complete the refactoring for long-term maintainability

1. Create sub-structs in `src/ui/response.rs`
2. Update all UI functions to return/set sub-structs
3. Update `src/app/mod.rs` to handle new response structure
4. **Test:** Full regression test
5. Commit with detailed migration notes

### Phase 6: Documentation & Cleanup (Low Priority)
**Goal:** Ensure future contributors understand the patterns

1. Add examples to `docs/UI-REFACTORING.md`
2. Add inline documentation to helper functions
3. Update CLAUDE.md with new UI patterns
4. Remove old commented-out code if any
5. Final commit and merge to main

---

## Testing Strategy

### Unit Testing
- Helper functions should be testable in isolation
- Parameter rendering functions should have unit tests

### Integration Testing
- Manual testing of each UI window after migration
- Verify all controls trigger correct change flags
- Test edge cases (min/max values, rapid clicking, etc.)

### Regression Testing
- Compare screenshots before/after refactoring
- Verify all presets load correctly
- Test all variation categories in 2D and 3D modes
- Test all parameter types (float, integer, angle)

### Performance Testing
- Ensure no measurable performance degradation
- UI should remain at 60 FPS during interaction

---

## Risk Mitigation

### Risk 1: Breaking Existing Functionality
**Mitigation:**
- Implement helpers without touching existing code initially
- Migrate one file at a time
- Test thoroughly after each migration
- Keep commits small and focused

### Risk 2: Performance Degradation
**Mitigation:**
- Helper functions should be zero-cost abstractions (inline where appropriate)
- Benchmark if any concerns arise
- egui's immediate mode design makes abstraction overhead negligible

### Risk 3: Incomplete Migration
**Mitigation:**
- Phase 1-2 deliver immediate value even if later phases are delayed
- Each phase is independently useful
- Document migration status clearly

### Risk 4: Breaking Change Detection
**Mitigation:**
- Helper functions encapsulate `.changed()` checks
- Add assertions to catch missing change flags during development
- Thorough testing ensures all change paths work

---

## Success Metrics

### Code Metrics
- [ ] **Line Count:** Reduce `src/ui/transforms.rs` from 428 → ~100 lines
- [ ] **Function Parameters:** Reduce `render_settings_window` from 44 → ~8 params
- [ ] **Duplication:** Eliminate 800+ lines of duplicated parameter rendering
- [ ] **Response Fields:** Group 46 flat fields → 5 categorized sub-structs

### Quality Metrics
- [ ] **Maintainability:** Adding new UI control requires 1-2 edits instead of 5+
- [ ] **Consistency:** All similar controls use identical patterns
- [ ] **Readability:** Intent is immediately clear from helper function names
- [ ] **Bug Reduction:** Zero regressions introduced during refactoring

### Developer Experience Metrics
- [ ] **New Feature Time:** Estimate 50% reduction in time to add new UI controls
- [ ] **Bug Fix Time:** Parameter rendering bugs fixed in 1 location, not 4+
- [ ] **Onboarding:** New contributors can understand UI patterns immediately

---

## Timeline

**Phase 1 (Foundation):** 2-3 hours
**Phase 2 (Transforms):** 1-2 hours
**Phase 3 (Settings):** 2-3 hours
**Phase 4 (Other Windows):** 2-3 hours
**Phase 5 (Response):** 1-2 hours
**Phase 6 (Documentation):** 1 hour

**Total Estimated Time:** 9-14 hours
**Expected Completion:** 1-2 development sessions

---

## References

### EGUI Documentation
- Builder Pattern: https://docs.rs/egui/latest/egui/
- Widget Customization: https://docs.rs/egui/latest/egui/struct.Slider.html
- Immediate Mode Patterns: https://github.com/emilk/egui/blob/master/ARCHITECTURE.md

### Internal Documentation
- `docs/ARCHITECTURE.md` - Overall system architecture
- `docs/STATUS.md` - Implementation status
- `CLAUDE.md` - Project context and guidelines

### Affected Files
- `src/ui/mod.rs` - Main UI orchestration (424 lines)
- `src/ui/transforms.rs` - Transform editor (428 lines) ← **Primary target**
- `src/ui/settings.rs` - Settings window (366 lines)
- `src/ui/tone_mapping.rs` - Tone mapping controls (389 lines)
- `src/ui/view.rs` - View controls (155 lines)
- `src/ui/response.rs` - Response struct (99 lines)

**Total Lines in src/ui/:** ~2500 lines
**Expected Reduction:** ~1000 lines (40% reduction)

---

## Next Steps

1. ✅ Commit this document
2. Begin Phase 1: Create helper modules
3. Begin Phase 2: Migrate transforms.rs
4. Iterate based on lessons learned

---

**Document Version:** 1.0
**Last Updated:** 2025-10-27
**Status:** Ready for Implementation
