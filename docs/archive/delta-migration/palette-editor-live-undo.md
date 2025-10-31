# Palette Editor: Live Editing with Undo Support

**Status:** In Progress (Phases 2-3 complete)
**Created:** 2025-10-31
**Updated:** 2025-10-31
**Goal:** Remove Apply button, add live editing with full undo/redo support

---

## Current Implementation Issues

1. **Global library modification** - Edits add/update palettes in global library
2. **No undo support** - Apply button has no undo tracking
3. **Deferred changes** - Must click Apply to see changes in fractal
4. **Unsaved changes tracking** - Manual `has_unsaved_changes` flag
5. **Custom palette flow** - Complex logic in app/mod.rs (line 542-572)

## Proposed Architecture

### Data Flow

**Current:**
```
PaletteEditor.current_palette (local copy)
  → User edits (has_unsaved_changes = true)
  → Click Apply
  → custom_palette = Some(...)
  → app/mod.rs: Add to global library + select it
  → No undo support
```

**Proposed:**
```
config.palette (fractal's own palette)
  → Load into PaletteEditor on open
  → User edits → ConfigManager.update_param(ConfigPath::Palette(...))
  → Live preview in fractal
  → Undo/redo works automatically
  → Library stays read-only reference
```

### Key Design Decisions

1. **Fractal owns its palette** - `config.palette: Option<Palette>`
   - If `Some(palette)`, use that (custom/edited palette)
   - If `None`, use `palette_index` from library (read-only reference)

2. **Library is read-only** - Selecting from library copies it to `config.palette`
   - Library = preset/template palettes
   - Editing creates a copy in the fractal config

3. **Live editing** - No Apply button needed
   - Changes apply immediately via ConfigManager
   - See results in real-time

4. **Full undo support** - Each edit creates undo entry
   - Snapshot-based (whole palette, not individual color stops)
   - Similar to tone curve implementation

---

## Palette Editor Operations

### 1. Color Stop Dragging (Position)

**Interaction:** Drag color stop left/right to change position

**Current Implementation:**
```rust
// Line ~140 of palette_editor.rs
if response.dragged() {
    stop.position = (new_x / gradient_width).clamp(0.0, 1.0);
    palette_editor.has_unsaved_changes = true;
}
```

**Proposed Implementation:**
```rust
// Lazy mode - live preview during drag
if response.drag_started() {
    // Store initial palette for comparison
    drag_start_palette = Some(current_palette.clone());
}

if response.dragged() {
    stop.position = (new_x / gradient_width).clamp(0.0, 1.0);
    // Update via ConfigManager in lazy mode
    let _ = config_manager.update_param(
        ConfigPath::Palette(Arc::new(current_palette.clone())),
        ConfigValue::Palette(current_palette.clone()),
        true // lazy mode
    );
}

if response.drag_stopped() {
    // Force commit creates final undo entry
    let _ = config_manager.force_commit_preview(&ConfigPath::Palette(...));
}
```

**Undo Behavior:** Single undo entry for entire drag operation

---

### 2. Color Picker (RGB Selection)

**Interaction:** Click color stop → Color picker opens → Drag RGB sliders or click color

**Current Implementation:**
```rust
// Line ~165 of palette_editor.rs
ui.color_edit_button_rgb(&mut stop.color);
// Sets has_unsaved_changes = true somewhere
```

**Proposed Implementation:**

**Option A: Lazy mode (recommended)**
```rust
// Track when color picker is active
let picker_active = ui.memory().is_popup_open(stop_id);

if !picker_active && was_active {
    // Color picker just closed - create undo entry
    let _ = config_manager.update_param(
        ConfigPath::Palette(Arc::new(current_palette.clone())),
        ConfigValue::Palette(current_palette.clone()),
        false // immediate mode on close
    );
}

if picker_active {
    // Live preview during color editing
    if ui.color_edit_button_rgb(&mut stop.color).changed() {
        let _ = config_manager.update_param(
            ConfigPath::Palette(...),
            ...,
            true // lazy mode
        );
    }
}
```

**Option B: Immediate mode**
```rust
// Every color change creates undo entry
if ui.color_edit_button_rgb(&mut stop.color).changed() {
    let _ = config_manager.update_param(
        ConfigPath::Palette(Arc::new(current_palette.clone())),
        ConfigValue::Palette(current_palette.clone()),
        false // immediate mode
    );
}
```

**Undo Behavior:**
- Option A: Single entry per color picker session (cleaner undo stack)
- Option B: Entry per RGB slider move (noisier undo stack)

**Recommendation:** Option A (lazy mode) - Better UX, cleaner undo

---

### 3. Add Color Stop

**Interaction:** Click gradient area → Add new stop at that position

**Current Implementation:**
```rust
// Line ~95 of palette_editor.rs
if response.clicked() {
    let position = (click_pos.x - rect.min.x) / rect.width();
    let color = palette_editor.current_palette.sample(position);
    palette_editor.current_palette.stops.push(ColorStop { position, color });
    palette_editor.has_unsaved_changes = true;
}
```

**Proposed Implementation:**
```rust
if response.clicked() {
    let position = (click_pos.x - rect.min.x) / rect.width();
    let color = current_palette.sample(position);
    current_palette.stops.push(ColorStop { position, color });
    current_palette.stops.sort_by(|a, b| a.position.partial_cmp(&b.position).unwrap());

    // Immediate mode - discrete action
    let _ = config_manager.update_param(
        ConfigPath::Palette(Arc::new(current_palette.clone())),
        ConfigValue::Palette(current_palette.clone()),
        false // immediate mode
    );
}
```

**Undo Behavior:** One entry per add operation

---

### 4. Delete Color Stop

**Interaction:** Right-click color stop → Delete

**Current Implementation:**
```rust
// Line ~150 of palette_editor.rs
if response.clicked_by(egui::PointerButton::Secondary) {
    stops_to_delete.push(i);
    palette_editor.has_unsaved_changes = true;
}
// Later: Remove from vec
```

**Proposed Implementation:**
```rust
if response.clicked_by(egui::PointerButton::Secondary) {
    current_palette.stops.remove(i);

    // Immediate mode - discrete action
    let _ = config_manager.update_param(
        ConfigPath::Palette(Arc::new(current_palette.clone())),
        ConfigValue::Palette(current_palette.clone()),
        false // immediate mode
    );
}
```

**Undo Behavior:** One entry per delete operation

---

### 5. Palette Name Edit

**Interaction:** Type in palette name text field

**Current Implementation:**
```rust
// Line 44 of palette_editor.rs
if ui.text_edit_singleline(&mut palette_editor.current_palette.name).changed() {
    palette_editor.has_unsaved_changes = true;
}
```

**Proposed Implementation:**

**Option A: On focus loss (recommended)**
```rust
let name_response = ui.text_edit_singleline(&mut current_palette.name);

if name_response.lost_focus() {
    // Create undo entry when user finishes editing
    let _ = config_manager.update_param(
        ConfigPath::Palette(Arc::new(current_palette.clone())),
        ConfigValue::Palette(current_palette.clone()),
        false
    );
}
```

**Option B: Per keystroke (noisy)**
```rust
if ui.text_edit_singleline(&mut current_palette.name).changed() {
    // Every keystroke creates undo entry
    let _ = config_manager.update_param(...);
}
```

**Recommendation:** Option A (on focus loss) - Cleaner undo

**Undo Behavior:** One entry per name edit session

---

### 6. Import Palette (JSON/File)

**Interaction:** Paste JSON or load .palette file

**Current Implementation:**
```rust
// Various import paths that set custom_palette
```

**Proposed Implementation:**
```rust
// After parsing JSON/file into Palette
let imported_palette = Palette::from_json(&json)?;

// Replace current palette
let _ = config_manager.update_param(
    ConfigPath::Palette(Arc::new(imported_palette.clone())),
    ConfigValue::Palette(imported_palette),
    false // immediate mode
);
```

**Undo Behavior:** One entry per import operation

---

### 7. Convert to/from Fixed Mode

**Interaction:** Toggle between gradient mode (variable stops) and fixed 256-color mode

**Current Implementation:**
```rust
// Line ~103, 257 of palette_editor.rs
if clicked {
    palette_editor.current_palette.to_fixed_256();
    *palette_changed = true;
    *custom_palette = Some(palette_editor.current_palette.clone());
}
```

**Proposed Implementation:**
```rust
if convert_to_fixed_clicked {
    current_palette.to_fixed_256();

    // Immediate mode - discrete conversion
    let _ = config_manager.update_param(
        ConfigPath::Palette(Arc::new(current_palette.clone())),
        ConfigValue::Palette(current_palette.clone()),
        false
    );
}
```

**Undo Behavior:** One entry per conversion operation

---

## Implementation Checklist

### Phase 1: Setup ConfigManager Integration ✅

- [x] Add `config_manager: &mut ConfigManager` parameter to `render_palette_editor_window()`
- [ ] Remove `custom_palette: &mut Option<Palette>` parameter (no longer needed)
- [ ] Remove `has_unsaved_changes` field from `PaletteEditor` struct
- [ ] Load initial palette from `config_manager.active_config().palette` (or library if None)

**Status:** Partial - ConfigManager parameter added, other cleanup pending

### Phase 2: Migrate Color Stop Dragging ✅

- [x] Add drag state tracking (drag_start_palette)
- [x] Implement lazy mode during drag
- [x] Call `force_commit_preview()` on drag end
- [ ] Test: Drag stop → release → undo → position reverts

**Implementation:** Position slider now uses ConfigManager with lazy mode. Changes tracked via flags to avoid borrow conflicts.

**Status:** Complete - Ready for testing

### Phase 3: Migrate Color Picker ✅

- [x] Track color picker open/close state
- [x] Implement lazy mode during color editing
- [x] Create undo entry on picker close
- [ ] Test: Edit color → close picker → undo → color reverts

**Implementation:** Color picker uses `lost_focus()` event to detect close and create final undo entry. Live updates during editing via lazy mode.

**Status:** Complete - Ready for testing

### Phase 4: Migrate Add/Delete Stops

- [ ] Add stop: Immediate mode undo entry
- [ ] Delete stop: Immediate mode undo entry
- [ ] Test: Add stop → undo → stop removed
- [ ] Test: Delete stop → undo → stop restored

### Phase 5: Migrate Name/Import/Convert

- [ ] Name edit: Undo entry on focus loss
- [ ] Import: Immediate mode undo entry
- [ ] Convert to/from fixed: Immediate mode undo entry
- [ ] Test: Each operation creates proper undo entry

### Phase 6: Remove Apply Button

- [ ] Delete Apply button UI code
- [ ] Remove apply button logic
- [ ] Remove `has_unsaved_changes` checks
- [ ] Update window title (no more "unsaved changes" indicator)

### Phase 7: Clean Up app/mod.rs

- [ ] Remove custom_palette handling (lines 541-572)
- [ ] Remove TODO comment about palette editor
- [ ] Palette changes now handled entirely by ConfigManager

### Phase 8: Testing

- [ ] Test all edit operations create undo entries
- [ ] Test undo/redo works for each operation
- [ ] Test live preview updates fractal immediately
- [ ] Test selecting from library creates config.palette copy
- [ ] Test preset loading preserves embedded palette

---

## Complexity Considerations

### Medium Complexity Items

1. **Drag state tracking** - Need to detect drag start/stop for lazy mode
2. **Color picker state** - Need to detect when picker closes
3. **Focus loss detection** - For name editing undo

### Low Complexity Items

1. **Add/delete stops** - Simple immediate mode
2. **Import/convert** - Simple immediate mode
3. **ConfigManager integration** - Already supports ConfigPath::Palette

### Edge Cases to Handle

1. **Selecting from library** - Should copy to config.palette (not reference)
2. **Preset loading** - Already embeds palette data (works)
3. **Config import** - Already has palette field (works)
4. **Multiple edits in sequence** - Each creates separate undo entry (acceptable)
5. **Rapid color changes** - Lazy mode throttle handles this

---

## Benefits After Implementation

✅ **Live editing** - See changes immediately, no Apply button
✅ **Full undo/redo** - Every edit tracked in undo stack
✅ **Cleaner code** - Remove unsaved changes tracking
✅ **Better UX** - No "forgot to click Apply" mistakes
✅ **Consistent** - Same undo pattern as tone curve editor
✅ **Palette ownership** - Fractal owns its palette, library is reference

---

## Migration Path

**Safe approach:** Implement in phases, test each phase independently

**Phase order:**
1. Setup (integrate ConfigManager) - Foundation
2. Color stop dragging (lazy mode) - Most complex
3. Color picker (lazy mode) - Medium complexity
4. Add/delete (immediate mode) - Simple
5. Name/import/convert (immediate mode) - Simple
6. Remove Apply button - Cleanup
7. Remove custom_palette handling - Cleanup

**Estimated complexity:** Medium (3-4 hours of focused work)
**Risk level:** Medium (significant UI behavior change)
**Testing needs:** High (all edit operations must be thoroughly tested)

---

## Questions to Resolve

1. **Color picker lazy mode** - Is detecting picker close reliable in egui?
   - May need to track popup state manually
   - Alternative: Use immediate mode for color changes (noisier undo)

2. **Name editing** - Focus loss vs per-keystroke?
   - Focus loss = cleaner undo (recommended)
   - Per-keystroke = noisier but more granular

3. **Library selection behavior** - Copy vs reference?
   - Copy = fractal independent from library (recommended)
   - Reference = palette_index only (current behavior for built-ins)

4. **Existing palettes in library** - Keep or remove?
   - Keep library as read-only templates (recommended)
   - Editing creates a copy in fractal config
   - Option to "Save to Library" in future?

---

## Recommendation

**Proceed with implementation?** Yes, but in phases.

**Start with:** Phase 1-2 (setup + color stop dragging)
- This validates the approach
- Most complex part done first
- Can test undo/redo early

**Benefits vs Cost:**
- **High benefit:** Better UX, undo support, cleaner code
- **Medium cost:** 3-4 hours implementation + testing
- **Worth it?** Yes, if palette editing is frequently used

**Alternative:** Keep current Apply button approach
- Low cost (already done)
- Missing: Undo support, live preview
- Pro: Simple, works
- Con: Poor UX, no undo
