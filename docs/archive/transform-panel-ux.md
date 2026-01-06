# Transform Panel UX Improvements

**Status:** Complete ✅
**Created:** 2025-12-29
**Completed:** 2025-12-29
**Branch:** feature/ux-improvements

## Overview

Reorganize the Transform panel to highlight important controls and hide advanced settings. Also improve Triangle Editor with dynamic bounds and transform selection sync.

## Transform Panel Changes

### Per-Transform Layout

**Always Visible:**
- Weight slider
- Palette Position + Color preview
- "Edit Triangle" button (selects transform in Triangle Editor)
- Clone and Delete buttons

**Advanced Section (CollapsingHeader, collapsed by default):**
- Affine Matrix (a, b, c, d, e, f, g)
- Color Speed (Symmetry)
- Opacity

**Variations Section:**
- Only show *enabled* variations (explicitly added by user)
- Each variation shows:
  - Name + weight slider
  - Parameters in CollapsingHeader (if any)
  - Delete button (✕)
- "Add Variation" button at bottom

### Enabled Variations Paradigm

**Key change:** A variation is "enabled" when explicitly added, regardless of weight value.

- New transform starts with `linear` variation (weight 1.0)
- User clicks "Add Variation" → searchable list with categories → variation added with weight 1.0
- Weight of 0.0 removes the variation from the HashMap
- Adding/removing variations triggers shader rebuild

### Add Variation UI

- Searchable dropdown/popup with text filter
- Categories within list (Basic 2D, Advanced 2D, 3D Depth, 3D Rotation, Full 3D)
- Shows variation name and category
- Click to add, closes popup
- Already-enabled variations are filtered out

## Triangle Editor Changes

### Transform Selection Sync

- "Edit Triangle" button in Transform panel sets `selected_transform_index` via egui persisted data
- Triangle Editor reads this index and highlights/selects that triangle
- Uses shared egui Id: `triangle_editor_selected_transform`
- Allows quick jumping between Transform panel and Triangle Editor

### Dynamic Bounds

**Previous:** Hardcoded -2 to 2 range

**New behavior:**
- Calculate bounds from all triangle vertices across all transforms
- Add 20% padding
- Minimum bounds remain -2 to 2
- Formula: `bounds = max(2.0, max_vertex_coord * 1.2)`

## Implementation Summary

### Phase 1: Transform Panel Reorganization ✅
1. [x] Restructure per-transform UI layout
2. [x] Move affine/advanced settings into CollapsingHeader
3. [x] Add "Edit Triangle" button with selection sync

### Phase 2: Enabled Variations System ✅
1. [x] Change UI to only show enabled variations (HashMap key presence = enabled)
2. [x] Add "Add Variation" popup with search and categories
3. [x] Add delete button per variation
4. [x] Wire up shader rebuild on variation add/remove

### Phase 3: Triangle Editor Improvements ✅
1. [x] Implement transform selection from external source (via egui persisted data)
2. [x] Calculate dynamic bounds from all triangles
3. [x] Update view to fit all triangles with padding

## Technical Notes

### Transform Data Structure

`Transform` uses `HashMap<String, f32>` for variations where presence = enabled.

This already supports the paradigm:
- Only show variations that are in the HashMap
- Add variation: insert into HashMap with weight 1.0
- Delete variation: set weight to 0.0 (Transform.set_variation removes if ~0)

### Shader Rebuild

Shader rebuild is triggered via `ConfigPath::TransformVariation` updates which set the `rebuild_shader` flag in UpdateAction.

### UI State

Shared state between panels:
- `triangle_editor_selected_transform: Option<usize>` - stored in egui persisted data
- Per-transform: CollapsingHeaders are open/closed automatically by egui

### Files Changed

- `src/ui/transforms.rs` - Complete rewrite with new layout (864 lines)
- `src/ui/triangle_editor.rs` - Dynamic bounds calculation
- `src/variations/mod.rs` - Added `Copy` derive to `VariationCategory`
