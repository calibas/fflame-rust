# Fractal Browser

**Branch:** `feature/fractal-browser`
**Status:** In Progress

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

### Phase 1: Unify Preset Loading
- [ ] Create `assets/presets.fflame` with curated preset list
- [ ] Add preset loading to `src/resources/` module
- [ ] Simplify `PresetLibrary` to load from single file
- [ ] Remove hard-coded presets (except single default fallback)
- [ ] Test on both Desktop and WASM

### Phase 2: Create Fractal Browser Panel
- [ ] Create `src/ui/fractal_browser.rs`
- [ ] Implement tab state (enum: Presets, RandomBatch, Files)
- [ ] Each tab renders a `FractalConfigGallery`
- [ ] Tab switching UI
- [ ] Selected config response

### Phase 3: Wire Up Data Sources
- [ ] Presets tab: Load from PresetLibrary
- [ ] Random Batch tab: Receive configs from Random Generator
- [ ] Files tab: Receive configs from file loading

### Phase 4: Integration
- [ ] Add `PanelType::FractalBrowser` to workspace
- [ ] Update menu (Window > Fractal Browser)
- [ ] Update Random Generator to send batch to Fractal Browser
- [ ] Update file loading to send to Fractal Browser
- [ ] Auto-switch tab on data arrival

### Phase 5: Cleanup
- [ ] Remove `PresetLibraryPanel`
- [ ] Remove `FileBrowserPanel`
- [ ] Remove old preset loading code from `presets.rs`
- [ ] Update documentation

## Files to Modify

| File | Changes |
|------|---------|
| `src/ui/fractal_browser.rs` | New - main panel implementation |
| `src/ui/mod.rs` | Add fractal_browser module, update EguiLayer |
| `src/ui/workspace.rs` | Add FractalBrowser panel type, remove old types |
| `src/scene/presets.rs` | Simplify to single default + file loading |
| `src/resources/mod.rs` | Add preset loading (like palettes) |
| `src/app/ui_handlers.rs` | Update batch/file handling |
| `assets/presets.fflame` | New - curated preset collection |

## Files to Remove

- `src/ui/preset_library.rs`
- `src/ui/file_browser.rs`
- `assets/presets/` directory (individual .fflame files)

## Notes

- All tabs share `FractalConfigGallery` for consistent UI
- Thumbnail generation already exists - reuse existing infrastructure
- `src/resources/` module already handles Desktop/WASM fetch abstraction
