# Fractal Browser

**Branch:** `feature/fractal-browser`
**Status:** Complete - All phases done

## Overview

Unify the Preset Library, Random Batch results, and File Browser into a single tabbed panel called Fractal Browser. All three tabs use `FractalConfigGallery` internally with different data sources.

## Goals

1. **Unify Preset Loading** - Single `assets/presets.fflame` file for both Desktop and WASM
2. **Consolidate UI** - Combine three panels into one tabbed Fractal Browser
3. **Better UX** - Auto-switch tabs when loading files or generating batches
4. **Persistent Batch** - Random batch results persist in their own tab

## Architecture

### Fractal Browser Panel

Single `PanelType::FractalBrowser` with three internal tabs:

```
┌─────────────────────────────────────────────────────────┐
│  Fractal Browser                                    ≡   │
├─────────────────────────────────────────────────────────┤
│  [Presets] [Random Batch] [Files]                       │
├─────────────────────────────────────────────────────────┤
│                                                         │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐       │
│  │ thumb   │ │ thumb   │ │ thumb   │ │ thumb   │       │
│  │         │ │         │ │         │ │         │       │
│  │ Name 1  │ │ Name 2  │ │ Name 3  │ │ Name 4  │       │
│  └─────────┘ └─────────┘ └─────────┘ └─────────┘       │
│                                                         │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌─────────┐       │
│  │ thumb   │ │ thumb   │ │ thumb   │ │ thumb   │       │
│  │         │ │         │ │         │ │         │       │
│  │ Name 5  │ │ Name 6  │ │ Name 7  │ │ Name 8  │       │
│  └─────────┘ └─────────┘ └─────────┘ └─────────┘       │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

### Tab Data Sources

| Tab | Data Source | Persistence |
|-----|-------------|-------------|
| Presets | `assets/presets.fflame` via resources module | Loaded once at startup |
| Random Batch | Generated configs from Random Generator | Persists until new batch |
| Files | User-loaded .fflame files | Persists until new file loaded |

### Preset Loading

**Before:**
- 10 hard-coded presets in `src/scene/presets.rs`
- Desktop-only loading from `assets/presets/` directory

**After:**
- Single hard-coded default (identity transform + linear variation)
- Load all presets from `assets/presets.fflame` (multi-config JSON)
- Same loading path for Desktop and WASM via `src/resources/` module

### Auto-Switch Behavior

| Action | Result |
|--------|--------|
| Generate batch | Switch to Random Batch tab |
| Load .fflame file | Switch to Files tab |
| Open Fractal Browser | Default to Presets tab |

## Implementation Plan

### Phase 1: Unify Preset Loading ✅
- [x] Create `assets/presets.fflame` with curated preset list
- [x] Add preset loading to `src/resources/` module
- [x] Simplify `PresetLibrary` to load from single file
- [x] Remove hard-coded presets (except single default fallback)
- [x] Test on both Desktop and WASM

### Phase 2: Create Fractal Browser Panel ✅
- [x] Create `src/ui/fractal_browser.rs`
- [x] Implement tab state (enum: Presets, RandomBatch, Files)
- [x] Each tab renders a `FractalConfigGallery`
- [x] Tab switching UI
- [x] Selected config response

### Phase 3: Wire Up Data Sources ✅
- [x] Presets tab: Load from PresetLibrary
- [x] Random Batch tab: Receive configs from Random Generator
- [x] Files tab: Receive configs from file loading

### Phase 4: Integration ✅
- [x] Add `PanelType::FractalBrowser` to workspace
- [x] Update menu (Window > Fractal Browser)
- [x] Update Random Generator to send batch to Fractal Browser
- [x] Update file loading to send to Fractal Browser
- [x] Auto-switch tab on data arrival

### Phase 5: Cleanup ✅
- [x] Remove `PresetLibraryPanel`
- [x] Remove `FileBrowserPanel`
- [x] Remove old preset loading code from `presets.rs`
- [x] Remove individual preset files from `assets/presets/`
- [x] Update documentation

## Files Modified

| File | Changes |
|------|---------|
| `src/ui/fractal_browser.rs` | New - main panel implementation with tabs |
| `src/ui/mod.rs` | Added fractal_browser module, removed old panels from EguiLayer |
| `src/ui/workspace.rs` | Added FractalBrowser, removed PresetLibrary/FileBrowser panel types |
| `src/ui/panel_viewer.rs` | Added FractalBrowser panel rendering, removed old panels |
| `src/ui/menu_bar.rs` | Updated menu to use FractalBrowser, removed legacy menu items |
| `src/scene/presets.rs` | Simplified to load from resources module |
| `src/resources/presets.rs` | New - preset loading with embedded fallback |
| `src/resources/mod.rs` | Added preset loading exports |
| `src/app/ui_handlers.rs` | Updated batch/file handling to use FractalBrowser |
| `src/app/mod.rs` | Removed old thumbnail generation calls |
| `assets/presets.fflame` | New - curated preset collection (12 presets) |

## Files Removed

- `src/ui/preset_library.rs` - Replaced by FractalBrowser Presets tab
- `src/ui/file_browser.rs` - Replaced by FractalBrowser Files tab
- `assets/presets/` directory - Individual .fflame files consolidated into `assets/presets.fflame`

## Notes

- All tabs share `FractalConfigGallery` for consistent UI
- Thumbnail generation already exists - reuse existing infrastructure
- `src/resources/` module already handles Desktop/WASM fetch abstraction
