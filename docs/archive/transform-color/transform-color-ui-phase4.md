# Transform Color UI Enhancement (Phase 4)

**Status:** In Progress
**Branch:** `transform-color-redesign`
**Started:** 2025-11-07

## Overview

Add a versatile UI for transform color editing with two modes:
1. **Palette Position Mode** - Simple 0-1 slider (current implementation)
2. **Color Picker Mode** - Visual RGB picker with automatic palette lookup

## Background

Phase 3 completed the data migration from RGB array to palette position float. Now we need to make the UI more intuitive by allowing users to pick colors visually while still storing palette positions internally.

## Requirements

### Mode Toggle
- Button to switch between "Palette Position" and "Color Picker" modes
- Mode persists per transform (stored in UI state, not config)
- Default mode: Palette Position (simpler for experts)

### Palette Position Mode
- Current implementation: Single slider 0.0-1.0
- Shows position in palette gradient
- Direct control over palette coordinate

### Color Picker Mode
- Visual RGB color picker (egui::color_picker)
- Shows current color from palette lookup
- On change: Find closest palette position using `Palette::find_position()`
- May not match exact RGB due to palette quantization
- Visual feedback shows actual palette color at found position

### Implementation Details

**New Method: `Palette::find_position()`**
```rust
impl Palette {
    /// Find the closest palette position for a given RGB color
    /// Returns position in range 0.0-1.0
    pub fn find_position(&self, target_rgb: [f32; 3]) -> f32 {
        // Brute force search: Sample palette at N points
        // Calculate Euclidean distance in RGB space
        // Return position with minimum distance
    }
}
```

**UI State:**
```rust
// Per-transform state (not in FractalConfig)
struct TransformColorUiState {
    mode: ColorEditMode,  // PalettePosition or ColorPicker
}

enum ColorEditMode {
    PalettePosition,
    ColorPicker,
}
```

**UI Layout:**
```
[Palette Position ▼]  <-- Mode dropdown/toggle
┌─────────────────────────────────────┐
│ [====|=============================] │  <-- Palette position slider
└─────────────────────────────────────┘
     0.5
```

Or in Color Picker mode:
```
[Color Picker ▼]
┌─────────────────────────────────────┐
│ [RGB Color Picker Widget]            │
│                                       │
│ Palette position: 0.52 (closest)     │  <-- Shows found position
└─────────────────────────────────────┘
```

## Implementation Plan

### Step 1: Add `Palette::find_position()` method
- File: `src/scene/palette.rs`
- Brute force search with 256 samples
- Euclidean distance in RGB space
- Unit tests for basic cases

### Step 2: Add UI state management
- File: `src/ui/transforms.rs`
- `HashMap<usize, ColorEditMode>` to track mode per transform
- Default: PalettePosition
- Not serialized (UI-only state)

### Step 3: Update transform color UI
- File: `src/ui/transforms.rs`
- Add mode toggle button/dropdown
- Conditional rendering based on mode:
  - PalettePosition: Current slider (keep existing)
  - ColorPicker: egui color picker + find_position call
- Show "actual position" label in picker mode

### Step 4: Testing
- Test palette position lookup accuracy
- Test mode switching preserves color
- Test undo/redo works in both modes
- Test preview mode works in both modes

## Design Decisions

### Why not store RGB and auto-convert?
- Flame algorithm uses palette positions, not RGB
- Storing RGB would require conversion every frame
- Palette can change → RGB would become incorrect
- Single source of truth: palette position

### Why brute force search?
- Palette is non-linear gradient (stops at arbitrary positions)
- No closed-form solution for closest point
- 256 samples is fast enough (< 1ms)
- More sophisticated algorithms (k-d tree) overkill for this

### Why per-transform mode?
- Different users have different preferences
- Expert users may prefer direct position control
- Beginners may prefer visual picker
- Mode toggle allows mixing approaches

### Why not always show both?
- UI space constraints
- Reduces clutter
- Most users will pick one mode and stick with it

## Testing Strategy

### Unit Tests
```rust
#[test]
fn test_find_position_exact_stop() {
    // Red at 0.0, should find 0.0
    // Blue at 1.0, should find 1.0
}

#[test]
fn test_find_position_interpolated() {
    // Middle color should find ~0.5
}

#[test]
fn test_find_position_ambiguous() {
    // Color that appears multiple times
    // Should find first occurrence
}
```

### Manual Testing
- [ ] Switch mode while editing color - preserves value
- [ ] Undo/redo works in both modes
- [ ] Preview mode works in both modes
- [ ] Color picker shows correct initial color
- [ ] Found position is visually close to picked color
- [ ] Mode persists when switching transforms
- [ ] Mode resets on app restart (not serialized)

## Future Enhancements (Optional)

### Phase 4.1: Palette Preview
- Show mini palette gradient next to slider
- Visual feedback of color at current position
- Helps users understand palette coordinates

### Phase 4.2: Color Swatches
- Quick access to common colors (red, blue, green, etc.)
- Click swatch to set position
- Customizable swatch library

### Phase 4.3: Gradient Editor
- Edit palette stops directly
- Add/remove/move stops
- Visual gradient editor UI

## References

- Phase 1: docs/projects/transform-color-mode-removal.md
- Phase 2: docs/projects/transform-color-blend-param.md
- Phase 3: Commits 9905062 and 27b0467
- Apophysis color system: docs/projects/apophysis-full-compatibility.md
