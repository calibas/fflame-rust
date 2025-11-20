# Palette Library System - Project Spec

**Status:** Planning
**Branch:** `feature/palette-library`
**Created:** 2025-11-17

## Overview

Add support for 700+ palettes organized into packs with visual previews and pack management.

## Goals

1. Support large palette library (~700 palettes) without memory concerns
2. Visual gradient previews in selection UI
3. Pack-based organization with enable/disable toggles
4. Load from JSON files (not hardcoded)
5. New "Palette Library" panel for management

## Non-Goals (Future Projects)

- ❌ Pack preference persistence (save enabled/disabled state)
- ❌ User editing of packs via UI
- ❌ User-created custom packs
- ❌ Import/export individual palettes to packs

## Design Decisions

### 1. Memory Strategy: **Eager Loading (Option 2)**

Load all palettes on startup:
- **Memory cost:** 700 palettes × 256 colors × 3 × 4 bytes = ~2.1 MB (negligible)
- **Preview cost:** 700 × (width × 10px × 4 bytes) ≈ variable based on width
- **Benefit:** Instant palette switching, no I/O during use

### 2. File Organization

```
assets/palettes/
  packs/
    starter_pack.json    # Initial pack with ~50 curated palettes
    (future: nature.json, fire.json, etc.)
```

**JSON Format:**
```json
{
  "pack_name": "Starter Pack",
  "description": "Curated selection of versatile palettes",
  "enabled_by_default": true,
  "palettes": [
    {
      "name": "Fire",
      "stops": [
        {"position": 0.0, "color": [0.0, 0.0, 0.0]},
        {"position": 0.5, "color": [1.0, 0.5, 0.0]},
        {"position": 1.0, "color": [1.0, 1.0, 0.0]}
      ]
    }
  ]
}
```

### 3. UI Design

#### New Panel: **Palette Library**

**Layout:**
```
┌─ Palette Library ────────────────────┐
│ Search: [____________]               │
│                                      │
│ ☑ Starter Pack (50)                 │
│   [████████] Fire                    │
│   [████████] Ocean                   │
│   [████████] Forest                  │
│   ...                                │
│                                      │
│ ☐ Nature Pack (100)   [disabled]    │
│ ☐ Plasma Pack (150)   [disabled]    │
└──────────────────────────────────────┘
```

**Components:**
1. **Search box** at top (filter by name across enabled packs)
2. **Pack sections** with collapsible headers
   - Checkbox to enable/disable pack
   - Pack name + palette count
3. **Palette entries** (only shown for enabled packs)
   - Horizontal gradient preview (10px height, full width)
   - Palette name below or overlay
   - Click to select palette

**Preview Rendering:**
- **Size:** Full width of panel × 10px height
- **Format:** Render to `egui::ColorImage` on load
- **Storage:** Static image data in `PaletteLibrary` struct
- **Generation:** Sample palette at even intervals across width

### 4. Code Architecture

#### New Types

```rust
// src/scene/palette.rs (additions)

/// A pack of related palettes loaded from JSON
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PalettePack {
    pub pack_name: String,
    pub description: String,
    pub enabled_by_default: bool,
    pub palettes: Vec<Palette>,
}

/// Runtime state for palette library
pub struct PaletteLibrary {
    /// All loaded packs
    packs: Vec<PalettePack>,
    /// Runtime enabled state (index into packs vec)
    enabled_packs: Vec<bool>,
    /// Cached preview images (one per palette across all packs)
    previews: Vec<egui::ColorImage>,
}

impl PaletteLibrary {
    /// Load all packs from assets/palettes/packs/
    pub fn load_from_disk() -> Self;

    /// Get all palettes from enabled packs (for main palette dropdown)
    pub fn get_enabled_palettes(&self) -> Vec<&Palette>;

    /// Toggle pack enabled state
    pub fn set_pack_enabled(&mut self, pack_index: usize, enabled: bool);

    /// Generate preview image for a palette
    fn generate_preview(palette: &Palette, width: u32, height: u32) -> egui::ColorImage;
}
```

#### UI Module

```rust
// src/ui/palette_library.rs (new file)

/// Render the Palette Library panel
pub fn render_palette_library(
    ui: &mut egui::Ui,
    library: &mut PaletteLibrary,
    selected_palette_index: &mut usize,
) -> bool {
    // Returns true if palette selection changed
}
```

#### Integration

```rust
// src/app.rs
struct App {
    palette_library: PaletteLibrary,  // New field
    // existing fields...
}

// src/ui/workspace.rs
enum PanelId {
    Settings,
    Transforms,
    TriangleEditor,
    View,
    ToneMapping,
    History,
    PaletteLibrary,  // New panel
}
```

### 5. Preview Generation Strategy

**Generate on Load:**
```rust
fn generate_preview(palette: &Palette, width: u32, height: u32) -> egui::ColorImage {
    let mut pixels = vec![egui::Color32::BLACK; (width * height) as usize];

    for x in 0..width {
        let t = x as f32 / (width - 1) as f32;
        let color = palette.sample(t);
        let color32 = egui::Color32::from_rgb(
            (color[0] * 255.0) as u8,
            (color[1] * 255.0) as u8,
            (color[2] * 255.0) as u8,
        );

        // Fill vertical column
        for y in 0..height {
            pixels[(y * width + x) as usize] = color32;
        }
    }

    egui::ColorImage {
        size: [width as usize, height as usize],
        pixels,
    }
}
```

**Memory cost:** If panel width = 300px, 700 palettes × 300 × 10 × 4 bytes = 8.4 MB

**Alternative (on-demand):** Generate preview each frame
- Negligible CPU cost for 10px height
- Zero memory for cached images
- May reconsider if performance is an issue

## Implementation Plan

### Phase 1: Data Structures & Loading
1. Add `PalettePack` struct to `src/scene/palette.rs`
2. Add `PaletteLibrary` struct with load logic
3. Create `assets/palettes/packs/starter_pack.json` with 10-20 test palettes
4. Add tests for JSON loading

### Phase 2: Preview Generation
1. Add `generate_preview()` function
2. Generate previews on library load
3. Store as `Vec<ColorImage>` in `PaletteLibrary`

### Phase 3: UI Panel
1. Create `src/ui/palette_library.rs`
2. Add to workspace panel system
3. Render pack sections with checkboxes
4. Show gradient previews with palette names
5. Handle palette selection (return selected index)

### Phase 4: Integration
1. Add `PaletteLibrary` to `App` struct
2. Replace old palette dropdown with library-sourced palettes
3. Wire up palette selection to update active palette
4. Test with Starter Pack

### Phase 5: Expand Library
1. Add more packs (nature, fire, plasma, etc.)
2. Reach ~700 total palettes
3. Test performance with full library

## Testing Strategy

- **Unit tests:** JSON loading, palette pack parsing
- **Manual testing:**
  - Load library with 700 palettes (check memory usage)
  - Toggle packs on/off
  - Select palettes from different packs
  - Visual preview quality
  - Search functionality

## Open Questions

1. **Preview width:** Dynamic (panel width) or fixed (256px)?
   - **Decision:** Dynamic (use available panel width)

2. **Preview caching:** Pre-generate or render on-demand?
   - **Decision:** Start with on-demand, cache if performance issues

3. **Search implementation:** Filter by name only or also by pack?
   - **Decision:** Name only for now

4. **Default enabled packs:** Just Starter Pack or all?
   - **Decision:** Just Starter Pack initially

## Future Enhancements (Not This Project)

- Save enabled/disabled pack state to user preferences
- User can create custom packs via UI
- Import/export palette packs
- Palette categories/tags for better filtering
- Thumbnail grid view (in addition to list)
- Drag-and-drop palette organization
- Community pack marketplace/downloads

## Files to Modify/Create

**New Files:**
- `assets/palettes/packs/starter_pack.json`
- `src/ui/palette_library.rs`

**Modified Files:**
- `src/scene/palette.rs` - Add PalettePack, PaletteLibrary
- `src/app.rs` - Add palette_library field
- `src/ui/workspace.rs` - Add PaletteLibrary panel
- `src/ui/mod.rs` - Export palette_library module

## Success Criteria

- ✅ Load 700+ palettes from JSON files
- ✅ Visual gradient previews in UI (10px height)
- ✅ Pack enable/disable toggles work
- ✅ Search filters palettes by name
- ✅ Memory usage < 20 MB for full library
- ✅ Palette selection updates active palette
- ✅ No performance degradation with full library
