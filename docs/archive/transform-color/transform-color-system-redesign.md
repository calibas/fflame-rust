# Transform Color System Redesign

## Overview

Redesign the transform color system to be more intuitive and versatile, supporting both palette-based and direct color workflows with visual feedback.

**Status:** Planning → Implementation
**Branch:** `transform-color-redesign`
**Priority:** Medium - Improves UX significantly
**Estimated Effort:** 6-9 hours

---

## Current Problems

1. **Confusing UI**: Three RGB sliders (0-1) suggest picking RGB colors, but they actually control a palette coordinate (averaged to single value)
2. **No Visual Feedback**: Can't see what color you're actually getting from the palette
3. **Broken ColorMode**: ColorMode::Transform (mode 0) exists but is broken and unused
4. **Limited Workflow**: Only supports palette coordinates, no direct color picking
5. **Implementation Mismatch**: Stores `[f32; 3]` but shaders average to single value

**Current Storage:**
```rust
pub color: [f32; 3],  // RGB array, but averaged in shader
pub color_speed: f32, // -1.0 to 1.0, Apophysis symmetry
```

**Current Shader (Palette mode):**
```wgsl
let xform_color_value = (xform.color.r + xform.color.g + xform.color.b) / 3.0;
let symmetry = xform.color_speed;
let colorC1 = (1.0 + symmetry) / 2.0;
let colorC2 = xform_color_value * (1.0 - symmetry) / 2.0;
color_index = color_index * colorC1 + colorC2;
```

---

## Proposed Solution

### Phase 1: Clean Up Broken ColorMode (1-2 hours)

**Goal:** Remove vestigial ColorMode::Transform that doesn't work.

**Tasks:**
1. Remove `ColorMode::Transform` enum variant
2. Remove mode 0 handling from shaders (lines 41-44 in main_2d/3d.wgsl)
3. Simplify ColorMode enum to just `Palette` and `Speed`
4. Update all references (imports, match statements, UI)
5. Test that existing flames still work

**Files:**
- `src/scene/palette.rs` - ColorMode enum
- `shaders/core/main_2d.wgsl` - Remove mode 0 block
- `shaders/core/main_3d.wgsl` - Remove mode 0 block
- `src/ui/color_settings.rs` or similar - Update UI

**Verification:**
- ✅ All color modes removed except Palette and Speed
- ✅ Shaders compile without errors
- ✅ Existing flames render correctly
- ✅ No references to ColorMode::Transform remain

---

### Phase 2: Add Color Blend Parameter (2-3 hours)

**Goal:** Add new blend parameter for finer control over color contribution.

**New Storage:**
```rust
pub color: [f32; 3],      // Keep for now, change in Phase 3
pub color_speed: f32,     // Keep unchanged, Apophysis compat
pub color_blend: f32,     // NEW: 0.0 to 1.0, default 1.0
```

**New Shader Formula:**
```wgsl
let symmetry = xform.color_speed;
let colorC1 = (1.0 + symmetry) / 2.0;
let xform_color_value = (xform.color.r + xform.color.g + xform.color.b) / 3.0;
let colorC2 = xform_color_value * (1.0 - symmetry) / 2.0 * xform.color_blend;
color_index = color_index * colorC1 + colorC2;
```

**Behavior:**
- `color_blend = 0.0`: No transform color influence (pure inheritance)
- `color_blend = 0.5`: Half influence
- `color_blend = 1.0`: Full influence (standard Apophysis behavior)

**Tasks:**
1. Add `color_blend: f32` to Transform struct (default: 1.0)
2. Add to GPU transform buffer (GpuTransform struct)
3. Update shader formula in both 2D and 3D
4. Parse from XML (default to 1.0 if missing)
5. Add ConfigPath::TransformColorBlend
6. Add UI slider (0.0-1.0, label: "Color Blend" or "Color Influence")
7. Test with various blend values

**Files:**
- `src/scene/transforms.rs` - Add color_blend field
- `src/gpu/buffers.rs` - Update GpuTransform
- `shaders/core/header.wgsl` - Update Transform struct
- `shaders/core/main_2d.wgsl` - Update formula
- `shaders/core/main_3d.wgsl` - Update formula
- `src/apophysis_xml.rs` - Parse color_blend attribute
- `src/config/delta.rs` - Add ConfigPath variant
- `src/config/manager.rs` - Add getter/setter
- `src/ui/transforms.rs` - Add blend slider

**Verification:**
- ✅ color_blend = 1.0 renders identically to current system
- ✅ color_blend = 0.0 shows pure color inheritance
- ✅ Slider updates work with undo/redo
- ✅ XML import/export preserves blend value

---

### Phase 3: Versatile Color Input UI (3-4 hours)

**Goal:** Intuitive UI with visual feedback, supporting both palette and direct color workflows.

**Storage Change:**
```rust
pub color: f32,          // Single palette coordinate (0-1)
pub color_speed: f32,    // Unchanged
pub color_blend: f32,    // From Phase 2
```

**Migration:**
- Old `[f32; 3]` → New `f32`: Average RGB values
- Formula: `color = (old_color[0] + old_color[1] + old_color[2]) / 3.0`

**UI Design:**

**Mode Toggle:**
```
Transform Color:
( ) Palette Position  (•) Color Picker
```

**Palette Position Mode:**
```
┌─────────────────────────────────┐
│ [====|=======================] │ 0.45
│ [████████████████████████████] │ ← Color swatch
└─────────────────────────────────┘
```
- Slider: 0.0 to 1.0 (palette coordinate)
- Swatch: Shows palette.sample(position) color
- Updates in real-time as slider moves or palette changes

**Color Picker Mode:**
```
┌─────────────────────────────────┐
│ [Color Picker Widget]           │
│ RGB: (255, 128, 64)             │
│ [████████████████████████████] │ ← Color swatch
│ → Palette position: ~0.42       │ ← Approximate match
└─────────────────────────────────┘
```
- Standard RGB color picker (egui::color_picker or custom)
- Shows selected RGB color in swatch
- Converts to nearest palette position using color matching
- Stores resulting coordinate in `transform.color`

**Color Matching Algorithm:**
```rust
fn find_palette_position(target_rgb: [f32; 3], palette: &Palette) -> f32 {
    let mut best_distance = f32::MAX;
    let mut best_position = 0.0;

    // Sample palette at multiple positions
    for i in 0..256 {
        let position = i as f32 / 255.0;
        let palette_color = palette.sample(position);
        let distance = color_distance(target_rgb, palette_color);

        if distance < best_distance {
            best_distance = distance;
            best_position = position;
        }
    }

    best_position
}

fn color_distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    // Euclidean distance in RGB space
    let dr = a[0] - b[0];
    let dg = a[1] - b[1];
    let db = a[2] - b[2];
    (dr * dr + dg * dg + db * db).sqrt()
}
```

**Mode Persistence:**
- Store `color_input_mode: ColorInputMode` in FractalConfig
- Enum: `Palette` or `DirectRGB`
- UI remembers which mode was used per transform
- Default: Palette (matches current behavior)

**Tasks:**

**3.1 Data Structure Changes:**
1. Change `Transform::color` from `[f32; 3]` to `f32`
2. Add migration logic in FractalConfig deserialization
3. Add `ColorInputMode` enum to config
4. Update all code that reads/writes `transform.color`

**3.2 Palette Functions:**
1. Add `Palette::find_position(rgb: [f32; 3]) -> f32` method
2. Implement color matching algorithm
3. Test with various target colors

**3.3 UI Implementation:**
1. Replace 3 RGB sliders with mode toggle
2. Implement palette position mode (slider + swatch)
3. Implement color picker mode (picker + swatch + conversion)
4. Add real-time swatch updates
5. Integrate with ConfigManager (undo/redo)

**3.4 GPU Updates:**
1. Update GpuTransform to store single `color: f32`
2. Update shader to use `xform.color` directly (no averaging needed)
3. Verify alignment/padding is correct

**Files:**
- `src/scene/transforms.rs` - Change color type, add migration
- `src/scene/palette.rs` - Add find_position method
- `src/config/fractal_config.rs` - Add ColorInputMode, migration
- `src/config/delta.rs` - Update ConfigPath for single value
- `src/ui/transforms.rs` - Complete UI rewrite for color section
- `shaders/core/header.wgsl` - Update Transform struct
- `shaders/core/main_2d.wgsl` - Use xform.color directly
- `shaders/core/main_3d.wgsl` - Use xform.color directly
- `src/gpu/buffers.rs` - Update GpuTransform struct
- `src/apophysis_xml.rs` - Parse single color value

**Verification:**
- ✅ Old configs migrate correctly (RGB → single float)
- ✅ Palette position mode works with visual feedback
- ✅ Color picker mode finds reasonable palette matches
- ✅ Mode toggle persists across sessions
- ✅ Undo/redo works for color changes
- ✅ Shaders render correctly with single float
- ✅ Apophysis XML import/export still works

---

## Benefits

**User Experience:**
- ✅ **Intuitive**: Clear what you're controlling (palette position OR color)
- ✅ **Visual**: See actual resulting color in real-time
- ✅ **Versatile**: Two workflows (palette-based OR direct color)
- ✅ **Flexible**: Fine control with color_blend parameter

**Code Quality:**
- ✅ **Clean**: Remove broken ColorMode::Transform
- ✅ **Consistent**: Storage matches usage (single float)
- ✅ **Maintainable**: Simpler shader logic
- ✅ **Compatible**: Keep Apophysis color_speed formula

---

## Migration Strategy

**Backward Compatibility:**
1. Old configs with `color: [f32; 3]`:
   - Auto-convert: `new_color = (r + g + b) / 3.0`
   - Add `color_blend: 1.0` (maintain current behavior)
   - Default mode: Palette

2. Apophysis XML import:
   - Parse single color value (already single in XML)
   - Default `color_blend = 1.0`
   - Works seamlessly

3. Shader compatibility:
   - Remove averaging step (already single value)
   - Add `color_blend` multiplication
   - Identical output with `color_blend = 1.0`

---

## Testing Plan

**Phase 1 Testing:**
- Load existing flames → Verify renders unchanged
- Check all ColorMode references removed
- UI still shows color settings

**Phase 2 Testing:**
- Test color_blend = 0.0, 0.5, 1.0
- Verify 1.0 matches current behavior
- Test undo/redo with blend changes
- Import/export XML with blend parameter

**Phase 3 Testing:**
- Migrate old configs → Verify colors match
- Test palette position slider → Visual swatch correct
- Test color picker → Reasonable palette match
- Switch modes → Values preserved
- Test with different palettes → Swatch updates
- Edge cases: position 0.0, 0.5, 1.0
- Performance: Real-time updates smooth

**Regression Testing:**
- All built-in presets render correctly
- Existing user configs load and migrate
- Apophysis XML import still works
- Export → Import roundtrip preserves colors

---

## Implementation Order

**Phase 1: Clean Up** (1-2 hours)
1. Remove ColorMode::Transform
2. Remove shader mode 0 handling
3. Test existing flames work

**Phase 2: Add Blend** (2-3 hours)
1. Add color_blend field
2. Update shaders with blend formula
3. Add UI slider
4. XML import/export

**Phase 3: Versatile UI** (3-4 hours)
1. Change storage to single float
2. Implement color matching
3. Build new UI with mode toggle
4. Test and polish

**Total: 6-9 hours** (can be done in one session or split across days)

---

## Future Enhancements

**Phase 4 (Optional):**
- Better color matching (perceptual color space like LAB)
- "Sample from flame" - click to pick color
- Color presets (save/load favorite colors)
- Gradient editor per transform
- Color animation/interpolation

---

**Created:** 2025-11-07
**Status:** Planning Complete → Ready for Implementation
**Branch:** `transform-color-redesign`
**Estimated Effort:** 6-9 hours
