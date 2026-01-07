# Palette System Improvements

**Branch:** `feature/palette-improvements`
**Created:** 2025-01-06
**Status:** Phase 1, 2, 4, 5 Complete

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

### Phase 1: Core Refactor ✅ COMPLETE
**Goal:** Simplify palette architecture
**Completed:** 2025-01-06 (commit `7c80ed8`)

- [x] Make `Palette` required in FractalConfig (not `Option<Palette>`)
- [x] Remove `palette_index` field from FractalConfig
- [x] Deprecate `ConfigPath::PaletteIndex` (returns 0, logs warning)
- [x] Update Palette Editor to edit `config.palette` directly
- [x] Update Palette Library - already had "(Custom)" suffix on selection
- [x] Update existing presets to ensure all have palettes
- [x] Backward compatibility - old configs with `palette: null` still load
- [x] Undo/redo works for palette changes via ConfigManager

**Files modified (17 total):**
- `src/config/fractal_config.rs` - Made palette required, added deserializer
- `src/config/manager.rs` - Updated Palette/PaletteIndex handling
- `src/ui/palette_editor.rs` - Removed Option handling
- `src/ui/tone_mapping.rs` - Simplified palette access
- `src/ui/transforms.rs` - Direct palette access for color preview
- `src/app/mod.rs`, `config.rs`, `gpu_updates.rs`, `ui_handlers.rs`
- `src/renderer/render.rs` - Simplified palette access
- `src/animation/export.rs` - Removed palette_library dependency
- `src/export/renderer.rs`, `high_res.rs` - Direct palette access
- `src/scene/presets.rs` - All presets now have required palettes
- `src/apophysis_xml.rs` - Uses default palette if missing
- `examples/export_presets.rs`, `test_apophysis_import.rs`

### Phase 2: Save Custom Palettes ✅ COMPLETE
**Goal:** Allow users to save modified palettes to registry
**Status:** Complete
**Completed:** 2025-01-06

#### Implementation Summary
- Created `src/storage/custom_palettes.rs` with `CustomPaletteLibrary` struct
- Uses existing storage backend (filesystem on desktop, localStorage on WASM)
- Custom palettes stored as "Custom" pack (appears first in Palette Library)
- "Save to Library" button in Palette Editor → Import/Export section
- Palettes persist across sessions in `custom_palettes.json`
- Duplicate names allowed (palettes identified by index, not name)

#### Files Modified (8 files)
- `src/storage/custom_palettes.rs` - New file: CustomPaletteLibrary with load/save
- `src/storage/mod.rs` - Export CustomPaletteLibrary
- `src/scene/palette.rs` - PaletteLibrary: load_custom_pack(), save_to_custom_library()
- `src/ui/palette_editor.rs` - "Save to Library" button
- `src/ui/panel_viewer.rs` - palette_save_to_library parameter
- `src/ui/response.rs` - palette_save_to_library field in UiResponse
- `src/ui/mod.rs` - Wire up palette_save_to_library
- `src/app/ui_handlers.rs` - Handle palette_save_to_library response
- `locales/en.yml` - Translations for save_to_library

#### Usage
1. Edit a palette in the Palette Editor
2. Expand "Import/Export Palette" section
3. Click "📚 Save to Library"
4. Palette appears in Custom pack at top of Palette Library
5. Persists across app restarts

#### Future Enhancements (Not in Minimal)
- Delete palette from Custom pack
- Rename palette in library
- Reorder palettes
- Export/import entire Custom pack as file

### Phase 3: Library Organization (Future)
**Goal:** Better palette browsing and WASM improvements

- [ ] Improve pack organization UI
- [ ] Add search/filter functionality
- [ ] Better WASM palette loading (currently limited)
- [ ] Consider palette categories/tags

### Phase 4: Adjustable Palette Length ✅ COMPLETE
**Goal:** Allow palettes with different color counts (256 to 4096+)
**Status:** Complete
**Completed:** 2025-01-06

#### Implementation Summary
- Added `palette_size: u32` field to `FlameBuffers`, `TonemapParams`, and `FractalConfig`
- Updated `FlameBuffers::with_palette_size()` to create texture at specified size
- Updated `update_palette()` to use dynamic size for rotation loop and texture upload
- Updated `shaders/tonemap.wgsl` to use `palette_size` uniform for PathMap index calculations
- Added UI dropdown in Tone Mapping panel (256, 512, 1024, 2048, 4096)
- Added `ConfigPath::PaletteSize` to delta system for undo/redo support
- `FlameRenderer::with_palette_size()` accepts palette size parameter
- Unified render API uses config's palette_size for headless exports

#### Files Modified (10 files)
- `src/gpu/buffers.rs` - FlameBuffers palette_size field, dynamic texture creation
- `src/renderer/compute_kernel.rs` - FlameRenderer::with_palette_size()
- `src/renderer/render.rs` - Use config.palette_size in unified render API
- `src/config/fractal_config.rs` - palette_size field with serde defaults
- `src/config/defaults.rs` - DEFAULT_PALETTE_SIZE constant
- `src/config/delta.rs` - ConfigPath::PaletteSize variant
- `src/config/manager.rs` - PaletteSize getter/setter
- `src/ui/tone_mapping.rs` - Palette Size dropdown UI
- `shaders/tonemap.wgsl` - palette_size uniform in TonemapParams
- `locales/en.yml` - Translations for palette_size

#### Usage Notes
- **Interactive App:** Palette size changes take effect on app restart or config reload
  (texture must be recreated with new size)
- **Headless Export:** Uses config.palette_size directly when creating renderer
- **Backward Compatibility:** Defaults to 256, old configs without palette_size work correctly

#### Previous Architecture Notes (for reference)
The palette system uses a **gradient-based approach**:
- `Palette` struct stores variable-length color stops (`Vec<ColorStop>`)
- On render, `generate_texture_data(size)` samples the gradient at N positions
- GPU receives a **dynamic Nx1 Rgba8Unorm texture** (N = palette_size)
- Shaders sample using normalized coordinates (0.0-1.0)

#### Memory & Performance (Negligible Impact)

| Size | Texture Memory | Upload Time | Notes |
|------|----------------|-------------|-------|
| 256 | 4 KB | <0.1 ms | Current default |
| 1024 | 16 KB | <0.2 ms | High detail |
| 4096 | 64 KB | ~1 ms | Max recommended |

GPU texture sampling cost is negligible - caching handles it efficiently.

#### Compatibility Notes
- **Apophysis:** Fixed 256-color palettes. Import always uses 256, export resamples to 256.
- **Old configs:** Palettes without `palette_size` default to 256.
- **Gradients:** Already variable-length, just need to sample at correct size.

### Phase 5: Palette Squeeze ✅ COMPLETE
**Goal:** Transform palette color distribution
**Status:** Complete
**Completed:** 2025-01-06

#### Implementation Summary
- Added `palette_squeeze: f32` field to `FractalConfig` (default 1.0)
- Squeeze transformation applied in `update_palette()` before GPU upload
- **Squeeze > 1** (e.g., 16): Palette repeats N times across the texture
- **Squeeze < 1** (e.g., 0.1): Only 10% of palette shown, stretched to fill
- Formula: `src_t = (dst_t * squeeze) % 1.0`
- Order of operations: Squeeze first, then rotation
- No shader changes required - all processing done on CPU before texture upload

#### Files Modified (12 files)
- `src/config/defaults.rs` - DEFAULT_PALETTE_SQUEEZE constant
- `src/config/fractal_config.rs` - palette_squeeze field with serde defaults
- `src/config/delta.rs` - ConfigPath::PaletteSqueeze variant
- `src/config/manager.rs` - PaletteSqueeze getter/setter (clamped 0.1-16.0)
- `src/gpu/buffers.rs` - update_palette() applies squeeze transformation
- `src/renderer/compute_kernel.rs` - FlameRenderer::update_palette() wrapper
- `src/app/gpu_updates.rs` - Pass palette_squeeze parameter
- `src/app/ui_handlers.rs` - Pass palette_squeeze parameter (3 locations)
- `src/app/mod.rs` - Pass palette_squeeze on resize
- `src/ui/tone_mapping.rs` - Palette Squeeze slider (0.1 to 16.0)
- `src/scene/presets.rs` - Added palette_squeeze to FractalConfig constructors
- `src/apophysis_xml.rs` - Added palette_squeeze to FractalConfig constructor
- `locales/en.yml` - Translations for palette_squeeze

#### Usage
- **Slider range:** 0.1 to 16.0
- **Default:** 1.0 (no change)
- **Example:** Squeeze = 4.0 with 1024 palette size = 256-color pattern repeated 4 times
- **Benefit for video:** Combined with palette rotation for smoother color cycling

### Phase 6: Randomize Palette → Moved to Random Generator Panel
**Status:** Deferred - will be implemented as part of the Random Generator Panel project

See [random-generator-panel.md](random-generator-panel.md) for the comprehensive randomization system that includes palette generation.

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
