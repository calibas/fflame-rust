# Palette System Redesign

## Current Issues (2025-11-01)

### Problems Identified

1. **Selection Gets "Stuck"**
   - Sometimes can't switch between palettes in dropdown
   - Possible race condition in UI state
   - Complex interaction between config.palette and palette_library

2. **Confusing UX**
   - Palette controls scattered across multiple windows
   - Dropdown in Tone Mapping window
   - Edit/Clone buttons in Tone Mapping window
   - Actual editor is separate window
   - Not intuitive which palette you're editing

3. **Complex State Management**
   - config.palette (working copy)
   - palette_library (persistent storage)
   - custom_palette mechanism (adds to library)
   - palette_index (fallback for rendering)
   - Multiple synchronization points

4. **Inconsistent Behavior**
   - Initial startup palette sometimes doesn't appear
   - Custom palettes may or may not show in dropdown
   - Built-in protection works but architecture is convoluted

## What We Tried (Session History)

### Attempt 1: Live View Model
- Palette editor reads from library each frame
- **Failed**: Two palette sources fighting each other
- **Issue**: palette_editor.current_palette vs config.palette

### Attempt 2: ConfigPath::Palette as Single Source
- All changes go through ConfigManager
- ConfigManager enforces built_in=false
- **Partial Success**: Undo/redo works
- **Issue**: Palettes don't appear in dropdown

### Attempt 3: Add to Library Without Clearing config.palette
- custom_palette adds to library but doesn't set palette_index
- Prevents clearing config.palette
- **Partial Success**: Custom palettes appear
- **Issue**: Selection still gets stuck sometimes

### Current State
- ConfigManager enforces built_in=false (works)
- Undo/redo creates restore points (works)
- Custom palettes added to library (mostly works)
- **Still Broken**: UI selection can get stuck

## Proposed Redesign

### Principles

1. **Single Window for All Palette Operations**
   - Move dropdown into Palette Editor window
   - All palette UI in one place
   - Clear which palette you're working on

2. **Simplified State Model**
   - ONE source of truth: palette_library
   - config.palette_index points to active palette
   - No separate config.palette (causes sync issues)
   - Edit operations modify library directly

3. **Explicit Edit/Save Model**
   - Select palette from library (read-only preview)
   - Click "Edit" → Creates editable copy
   - Edit operations modify the editable copy
   - Click "Save" → Replaces in library
   - Click "Cancel" → Discards changes

4. **Clear Built-in Protection**
   - Built-ins are truly immutable
   - "Edit" on built-in auto-creates named copy
   - Can't accidentally modify built-ins
   - Clear visual distinction

### Proposed Architecture

```
PaletteLibrary (single source of truth)
├── Built-in palettes (immutable)
└── Custom palettes (mutable)

PaletteEditor State
├── selected_index: usize           // Which palette from library
├── editing_palette: Option<Palette> // Working copy while editing
└── is_editing: bool                // Are we in edit mode?

Config
└── palette_index: usize            // Active palette for rendering
```

### UI Layout (All in Palette Editor Window)

```
┌─ Palette Editor ────────────────────────────────┐
│                                                  │
│ Library:                                         │
│ ┌────────────────────────────────────────────┐  │
│ │ ▼ Fire                                     │  │
│ │   Cool                                     │  │
│ │   Grayscale                                │  │
│ │   ─────────────────                        │  │
│ │   Fire (Custom)                            │  │
│ │   My Custom Palette                        │  │
│ └────────────────────────────────────────────┘  │
│                                                  │
│ [Edit] [Clone] [Delete]                         │
│                                                  │
│ ┌─ Editing: Fire (Custom) ───────────────────┐  │
│ │                                             │  │
│ │ Gradient Preview: [===============]        │  │
│ │                                             │  │
│ │ Color Stops:                                │  │
│ │   ● 0.0  [▓▓▓] [🗑]                        │  │
│ │   ● 0.5  [▓▓▓] [🗑]                        │  │
│ │   ● 1.0  [▓▓▓] [🗑]                        │  │
│ │                                             │  │
│ │ [➕ Add Color Stop]                         │  │
│ │                                             │  │
│ │ Mode: 🎨 Free Gradient  [Switch to Fixed]  │  │
│ │                                             │  │
│ │ [✓ Save] [✗ Cancel]                        │  │
│ └─────────────────────────────────────────────┘  │
│                                                  │
│ [Import] [Export]                               │
└──────────────────────────────────────────────────┘
```

### Workflow

**Selecting a Palette:**
1. Open Palette Editor
2. Select from Library dropdown
3. Preview shown immediately
4. Fractal renders with selected palette
5. Undo/redo navigates palette selections

**Editing a Custom Palette:**
1. Select custom palette from library
2. Click "Edit" → Enters edit mode
3. Modify color stops
4. Changes shown live in preview
5. Click "Save" → Updates library
6. Click "Cancel" → Discards changes

**Editing a Built-in:**
1. Select built-in from library
2. Click "Edit" → Auto-creates copy "Fire (Custom)"
3. Copy added to library
4. Enters edit mode on the copy
5. Built-in remains unchanged

**Cloning:**
1. Select any palette
2. Click "Clone" → Creates "Palette (Copy)"
3. Immediately enters edit mode
4. Can rename and modify

### Implementation Plan

#### Phase 1: Consolidate UI
- [ ] Move palette dropdown into Palette Editor window
- [ ] Remove palette controls from Tone Mapping window
- [ ] Single window for all palette operations

#### Phase 2: Simplify State Model
- [ ] Remove config.palette field
- [ ] Use only config.palette_index
- [ ] Remove custom_palette mechanism
- [ ] Direct library modifications

#### Phase 3: Edit/Save Pattern
- [ ] Add editing_palette: Option<Palette> to PaletteEditor
- [ ] Implement Save/Cancel buttons
- [ ] Clear visual distinction between viewing and editing
- [ ] Undo/redo on palette_index changes only

#### Phase 4: Built-in Protection
- [ ] Library method: can_edit(index) -> bool
- [ ] Auto-fork on edit of built-in
- [ ] Visual indicators (🔒 for built-ins)
- [ ] Delete button only for custom palettes

#### Phase 5: Testing & Polish
- [ ] Test all palette operations
- [ ] Verify no stuck selections
- [ ] Check undo/redo works correctly
- [ ] Clean up any leftover code

### Benefits of Redesign

1. **Simpler Mental Model**
   - One window, one library, clear edit mode
   - No hidden state synchronization

2. **Better UX**
   - All palette controls in one place
   - Clear when you're editing vs viewing
   - Explicit save/cancel

3. **Fewer Bugs**
   - Single source of truth
   - No race conditions between UI components
   - No stuck selections

4. **Easier Maintenance**
   - Centralized palette logic
   - Clear responsibility boundaries
   - Easier to reason about

## Technical Notes

### ConfigPath Changes

Remove:
- `ConfigPath::Palette` (no embedded palette)

Keep:
- `ConfigPath::PaletteIndex` (points to library)

### Undo/Redo Behavior

**Before Edit:**
- Undo/redo changes palette_index
- Switches between different library palettes
- Fast and simple

**During Edit:**
- Changes are in editing_palette (not committed)
- Undo/redo still works on palette_index
- Can switch away from palette being edited

**After Save:**
- Updates library entry
- No undo needed (library modification is separate)
- OR: Add undo support for library changes (future enhancement)

### Library Management

```rust
impl PaletteLibrary {
    fn is_built_in(&self, index: usize) -> bool
    fn can_edit(&self, index: usize) -> bool { !self.is_built_in(index) }
    fn can_delete(&self, index: usize) -> bool { !self.is_built_in(index) }

    fn update(&mut self, index: usize, palette: Palette)
    fn delete(&mut self, index: usize) -> Result<(), Error>

    // Returns index of new palette
    fn add(&mut self, palette: Palette) -> usize

    // For built-in auto-fork
    fn create_editable_copy(&self, index: usize) -> Palette {
        let mut pal = self.palettes[index].clone();
        pal.name = generate_unique_name(&pal.name, self);
        pal.built_in = false;
        pal
    }
}
```

## Migration Path

### Step 1: Document Current State
- [x] Write this document
- [ ] List all known issues
- [ ] Get user confirmation on redesign

### Step 2: Create Branch
- [ ] Branch: `palette-editor-redesign`
- [ ] Commit point before major changes
- [ ] Can revert if needed

### Step 3: Incremental Changes
- [ ] Move UI first (cosmetic, low risk)
- [ ] Then change state model (high risk)
- [ ] Test thoroughly between steps

### Step 4: Verify and Merge
- [ ] All operations work smoothly
- [ ] No stuck selections
- [ ] Undo/redo works correctly
- [ ] Merge to main

## Open Questions

1. **Undo/Redo for Library Changes?**
   - Currently only palette_index changes create undo points
   - Should editing a palette create undo points?
   - Adds complexity but more user-friendly

2. **Save Custom Palettes to Disk?**
   - Currently only in-memory
   - Could save to assets/palettes/ on disk
   - Load on startup

3. **Palette Import/Export?**
   - Currently has JSON import/export
   - Keep this feature in redesign?
   - Where should it live in new UI?

4. **Color Mode Selection?**
   - Currently in Tone Mapping window
   - Should it move to Palette Editor?
   - Or keep separate (not really palette-specific)

## References

- Original issue: "Can't edit built-ins, no restore points for color stops"
- Session: 2025-11-01 palette system overhaul
- Related: ConfigManager state centralization project
- See also: docs/main/CONFIG.md, docs/main/COLOR.md
