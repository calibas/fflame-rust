# Palette System Improvements

**Branch:** `feature/palette-improvements`
**Created:** 2025-01-06
**Status:** Planning

## Overview

Refactor the palette system to simplify the architecture and add new features. The core change is making the FractalConfig's palette the single source of truth, eliminating concepts like "selected palette index" and palette cloning.

## Current Architecture (Problems)

1. **Fixed palette indexes** - Palettes identified by numeric index in a global list
2. **Selected palette as global state** - `selected_palette_index` separate from FractalConfig
3. **Optional palette in config** - `config.palette: Option<Palette>` allows missing palette
4. **Clone-on-select behavior** - Selecting from library creates a copy with "(Custom)" suffix
5. **Confusion about "active" palette** - Unclear which palette is canonical

## New Architecture

### Core Principle
The palette stored in `FractalConfig.flame.palette` (or `FractalConfig.palette`) is THE active palette. There is no other "selected" palette.

### Key Changes

1. **Palette is required** - `Palette` field is no longer `Option`, always present
2. **No fixed indexes** - Registry uses names for identification, not numeric IDs
3. **Registry is read-only** - Selecting from registry copies data to FractalConfig
4. **Simple selection** - Clicking a palette in the library overwrites the active palette
5. **UI shows config palette name** - Display name comes from `config.palette.name`
6. **Modified indicator** - When user edits palette, name becomes "OriginalName (Custom)"

### Data Flow

```
┌─────────────────┐     select      ┌──────────────────┐
│ Palette Library │ ───────────────>│  FractalConfig   │
│   (Registry)    │   (copy data)   │     .palette     │
│   [read-only]   │                 │  [active/edited] │
└─────────────────┘                 └──────────────────┘
                                            │
                                            │ edits via
                                            │ Palette Editor
                                            ▼
                                    ┌──────────────────┐
                                    │  GPU Palette     │
                                    │    Texture       │
                                    └──────────────────┘
```

## Implementation Phases

### Phase 1: Core Refactor
**Goal:** Simplify palette architecture

- [ ] Make `Palette` required in FractalConfig (not `Option<Palette>`)
- [ ] Remove `selected_palette_index` from app state
- [ ] Update Palette Editor to edit `config.palette` directly
- [ ] Update Palette Library to copy palette data on selection
- [ ] Add "(Custom)" suffix when palette is modified from original
- [ ] Remove palette cloning logic
- [ ] Update existing presets to ensure all have palettes
- [ ] Ensure undo/redo works for palette changes via ConfigManager

**Files to modify:**
- `src/config/fractal_config.rs` - Make palette required
- `src/config/delta.rs` - Palette-related ConfigPath variants
- `src/ui/palette_editor.rs` - Edit config palette directly
- `src/ui/palette_library.rs` - Copy on select, no cloning
- `src/ui/tone_mapping.rs` - Remove palette selection dropdown if present
- `src/app/mod.rs` - Remove selected_palette_index state
- `src/scene/presets.rs` - Ensure all presets have palettes

### Phase 2: Save Custom Palettes (Future)
**Goal:** Allow users to save modified palettes to registry

- [ ] Design palette save format (JSON in user data directory)
- [ ] Implement desktop save/load (filesystem)
- [ ] Implement WASM save/load (localStorage)
- [ ] Add "Save to Library" button in Palette Editor
- [ ] Handle name conflicts (prompt for new name or overwrite)

**Deferred:** Requires platform-specific storage code

### Phase 3: Library Organization (Future)
**Goal:** Better palette browsing and WASM improvements

- [ ] Improve pack organization UI
- [ ] Add search/filter functionality
- [ ] Better WASM palette loading (currently limited)
- [ ] Consider palette categories/tags

### Phase 4: Adjustable Palette Length
**Goal:** Allow palettes with different color counts

- [ ] Support palette lengths from 256 to 4096+ colors
- [ ] Update GPU texture creation for variable sizes
- [ ] Add UI control for palette length
- [ ] Handle interpolation for different lengths
- [ ] Consider memory/performance implications

### Phase 5: Palette Stretching/Squeezing
**Goal:** Transform palette color distribution

- [ ] Add stretch/squeeze controls to Palette Editor
- [ ] Implement non-linear remapping of color positions
- [ ] Preview changes in real-time
- [ ] Apply changes to palette stops

### Phase 6: Randomize Palette
**Goal:** Generate random palettes for exploration

- [ ] Random palette generation algorithms
- [ ] Options: completely random, harmonious colors, variations on current
- [ ] Seed-based generation for reproducibility
- [ ] "Shuffle" existing colors option

## Technical Details

### Palette Struct Changes

```rust
// Before
pub struct FractalConfig {
    pub palette: Option<Palette>,
    // ...
}

// After
pub struct FractalConfig {
    pub palette: Palette,  // Required, not optional
    // ...
}
```

### Modified Indicator Logic

```rust
impl Palette {
    /// Check if palette has been modified from a library palette
    pub fn is_custom(&self) -> bool {
        self.name.ends_with(" (Custom)")
    }

    /// Mark palette as custom (modified from original)
    pub fn mark_as_custom(&mut self) {
        if !self.is_custom() {
            self.name = format!("{} (Custom)", self.name);
        }
    }
}
```

### Selection Behavior

```rust
// In palette_library.rs
fn on_palette_selected(palette: &Palette, config_manager: &mut ConfigManager) {
    // Clone the palette data (not a reference)
    let new_palette = palette.clone();

    // Update config via ConfigManager (enables undo/redo)
    config_manager.update_param(
        ConfigPath::Palette,
        ConfigValue::Palette(new_palette)
    );
}

// In palette_editor.rs - when user makes any edit
fn on_palette_edited(config_manager: &mut ConfigManager) {
    // Mark as custom if not already
    let mut palette = config_manager.active_config().palette.clone();
    palette.mark_as_custom();
    config_manager.update_param(ConfigPath::Palette, ConfigValue::Palette(palette));
}
```

## Migration Notes

### Preset Compatibility
- Presets without palettes will need a default palette added
- Existing presets with `palette: None` must be updated

### Config File Compatibility
- Old .fflame files with `palette: null` need migration
- Add default palette during deserialization if missing

## Testing Plan

### Phase 1 Tests
- [ ] Load preset - verify palette appears in editor
- [ ] Select from library - verify palette changes
- [ ] Edit palette - verify "(Custom)" suffix appears
- [ ] Undo/redo palette changes
- [ ] Save/load .fflame file - verify palette persists
- [ ] All existing presets load without errors

## References

- Current palette code: `src/scene/palette.rs`
- Palette library: `src/ui/palette_library.rs`
- Palette editor: `src/ui/palette_editor.rs`
- Config system: `src/config/`
