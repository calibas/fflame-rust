# Phase 3: Apophysis Color System Compatibility

## Overview

Implement full color system compatibility with Apophysis 7X, including proper palette mode, color speed (symmetry), opacity, vibrancy, and XML import/export.

**Goal:** Ensure colors match exactly between Apophysis and fflame-rust when importing/exporting .flame files.

---

## Current State Analysis

### What We Have ✓
1. **Palette system** - 256-color gradients with interpolation
2. **Histogram accumulation** - U32 atomic color accumulation
3. **Three color modes**:
   - Transform Color (mode=0) - Direct RGB from transforms
   - Palette Mode (mode=1) - Lookup from palette texture
   - Speed Mode (mode=2) - Speed-based palette lookup
4. **Basic XML import** - Parses palettes and transform colors
5. **Color speed** - Stored as `Transform.color_speed` (0.0-1.0)
6. **Opacity** - Stored as `Transform.opacity` (though not used in XML export)

### What's Missing/Wrong ✗
1. **Color mode in XML import** - Always sets to Transform mode (should detect Palette mode)
2. **Color coordinate evolution** - Not implemented (Apophysis formula: `c = c * colorC1 + colorC2`)
3. **Color speed range** - We use 0-1, Apophysis uses -1 to 1 (symmetry)
4. **Opacity rendering** - Not implemented (stochastic transparency)
5. **Vibrancy** - Not implemented (old vs new color algorithm blend)
6. **Hue rotation** - Not implemented in palette loading
7. **White level** - Not implemented (palette brightness scaling)
8. **Plugin color influence** - Not implemented (variation-driven coloring)

---

## Phase 3 Implementation Plan

### Phase 3.1: Color Coordinate Evolution ✅ **COMPLETE**

**Goal:** Implement proper Apophysis color coordinate (`c`) evolution through transforms.

#### 3.1.1 - Data Structures
- [x] Add `color_coordinate: f32` to point structure in shaders
- [x] Add `color_speed: f32` to `GpuTransform` (already exists, verify range)
- [x] Verify `Transform.color_speed` range is -1 to 1 (currently 0-1)

#### 3.1.2 - Shader Implementation
- [x] Update `main_2d.wgsl` and `main_3d.wgsl`:
  ```wgsl
  // Initialize color coordinate (once per batch)
  var color_coord: f32 = random_0_1();

  // Each iteration through transforms
  let colorC1 = (1.0 + xform.color_speed) / 2.0;
  let colorC2 = xform.color * (1.0 - xform.color_speed) / 2.0;
  color_coord = color_coord * colorC1 + colorC2;

  // Final palette lookup
  final_color = textureSample(palette_texture, palette_sampler, color_coord);
  ```

#### 3.1.3 - Color Mode Detection
- [x] Update `parse_flame_element()` in `src/apophysis_xml.rs`:
  - Detect if flame has palette → Set `ColorMode::Palette`
  - No palette → Use `ColorMode::Transform`
- [x] Update `Transform.color` usage in Palette mode:
  - Store as 0-1 coordinate (not RGB)
  - Don't apply palette lookup during import

**Files Modified:**
- ✅ `shaders/core/main_2d.wgsl` - Added color coordinate evolution
- ✅ `shaders/core/main_3d.wgsl` - Added color coordinate evolution
- ✅ `src/scene/transforms.rs` - Updated color_speed range to -1 to 1, default to 0.0
- ✅ `src/ui/transforms.rs` - Updated slider range to -1 to 1
- ✅ `src/apophysis_xml.rs` - Fixed color mode detection, color coordinate storage, color_speed parsing

**Commits:**
- 2950ac2 - FEAT: Implement Apophysis color coordinate evolution (Phase 3.1)
- 93cfdd1 - FIX: Proper color mode detection and color_speed parsing in XML import

**Outcome:**
- ✅ Palette mode uses Apophysis color coordinate evolution formula
- ✅ Transform colors stored as palette coordinates (0-1) in Palette mode
- ✅ Color speed (symmetry) range expanded to -1 to 1
- ✅ Color mode auto-detected based on palette presence
- ✅ color_speed/symmetry parsed from XML (no longer hardcoded)

---

### Phase 3.2: Opacity (Stochastic Transparency)

**Goal:** Implement transform opacity as probability-based plotting.

#### 3.2.1 - RNG-Based Opacity Check
- [ ] Add opacity check in shaders after transform selection:
  ```wgsl
  if (random_0_1() >= xform.opacity) {
      continue;  // Skip this iteration (don't plot)
  }
  ```
- [ ] Verify `Transform.opacity` field exists and defaults to 1.0
- [ ] Update XML import to parse `opacity` attribute
- [ ] Update XML export to write `opacity` attribute

#### 3.2.2 - UI Control
- [ ] Add opacity slider to Transform window (0.0-1.0)
- [ ] Update ConfigManager with opacity parameter path

**Files to Modify:**
- `shaders/core/main_2d.wgsl` - Add opacity check
- `shaders/core/main_3d.wgsl` - Same as 2D
- `src/apophysis_xml.rs` - Parse/export opacity
- `src/ui/transforms.rs` - Add opacity slider
- `src/config/delta.rs` - Add TransformOpacity path

**Expected Outcome:**
- Transforms with opacity < 1.0 appear fainter
- Matches Apophysis stochastic transparency exactly

---

### Phase 3.3: Vibrancy (Color Algorithm Blend)

**Goal:** Implement vibrancy control for old vs new color algorithm blending.

#### 3.3.1 - Tonemap Shader Modification
- [ ] Add `vibrancy: f32` to `TonemapParams`
- [ ] Implement blend formula in `tonemap.wgsl`:
  ```wgsl
  let vib = vibrancy;
  let notvib = 1.0 - vib;

  if (notvib > 0.0) {
      // Blend old and new algorithms
      let old_color = pow(color, vec3(gamma));
      let new_color = color * brightness;
      final_color = new_color * vib + old_color * notvib;
  } else {
      final_color = color * brightness;
  }
  ```

#### 3.3.2 - Config and UI
- [ ] Add `vibrancy: f32` to `FractalConfig` (default 1.0)
- [ ] Add vibrancy slider to Tone Mapping window
- [ ] Update XML import/export with `vibrancy` attribute

**Files to Modify:**
- `shaders/tonemap.wgsl` - Implement vibrancy blend
- `src/gpu/buffers.rs` - Add vibrancy to TonemapParams
- `src/config/fractal_config.rs` - Add vibrancy field
- `src/apophysis_xml.rs` - Parse/export vibrancy
- `src/ui/tone_mapping.rs` - Add vibrancy slider

**Expected Outcome:**
- Vibrancy=1.0: Modern vibrant colors
- Vibrancy=0.0: Classic gamma-only colors
- 0-1: Smooth blend between both

---

### Phase 3.4: Palette Enhancements

**Goal:** Add Apophysis palette features (hue rotation, white level).

#### 3.4.1 - Hue Rotation
- [ ] Add `hue_rotation: f32` to `FractalConfig` (range 0-1)
- [ ] Implement RGB→HSV→RGB conversion in palette generation
- [ ] Apply hue rotation during palette upload:
  ```rust
  let hsv = rgb_to_hsv(color);
  hsv[0] = (hsv[0] + hue_rotation * 360.0) % 360.0;
  let rotated_color = hsv_to_rgb(hsv);
  ```
- [ ] Add hue rotation slider to Palette Editor

#### 3.4.2 - White Level (Brightness)
- [ ] Add `white_level: f32` to `FractalConfig` (default 256.0)
- [ ] Scale palette colors during upload:
  ```rust
  palette_data[i] = ((color * white_level) / 256.0).clamp(0.0, 255.0) as u8;
  ```
- [ ] Add white level slider to Color Settings
- [ ] Update XML import/export with `brightness` attribute

**Files to Modify:**
- `src/scene/palette.rs` - Add RGB↔HSV conversion utilities
- `src/gpu/buffers.rs` - Apply hue rotation and white level during upload
- `src/config/fractal_config.rs` - Add hue_rotation and white_level
- `src/apophysis_xml.rs` - Parse/export brightness (maps to white_level)
- `src/ui/palette_editor.rs` - Add hue rotation slider
- `src/ui/settings.rs` - Add white level slider

**Expected Outcome:**
- Hue rotation shifts all palette colors by specified degrees
- White level controls overall palette brightness
- Matches Apophysis rendering exactly

---

### Phase 3.5: XML Import/Export Fixes

**Goal:** Ensure perfect round-trip compatibility with Apophysis .flame files.

#### 3.5.1 - Import Improvements
- [ ] Fix color mode detection (Palette vs Transform)
- [ ] Parse `color_speed` (currently ignores XML value)
- [ ] Parse `opacity` attribute
- [ ] Parse `vibrancy` attribute
- [ ] Parse `brightness` (white level)
- [ ] Parse `hue_rotation` if present
- [ ] Handle `plotmode="off"` (sets opacity=0)

#### 3.5.2 - Export Implementation
- [ ] Implement `export_flame_xml()` function
- [ ] Write flame attributes (name, size, center, scale, etc.)
- [ ] Write transforms with:
  - `color` (as 0-1 coordinate in Palette mode, or averaged RGB in Transform mode)
  - `color_speed` (symmetry)
  - `opacity`
  - All variation weights and parameters
- [ ] Write palette in hex format (1536 chars)
- [ ] Write render settings (brightness, gamma, vibrancy)

#### 3.5.3 - Round-Trip Testing
- [ ] Create test suite for XML import/export:
  - Import Apophysis flame → Export → Re-import → Compare
  - Verify colors match pixel-for-pixel
  - Test all color modes (Transform, Palette, Speed)
  - Test opacity and vibrancy
  - Test hue rotation and white level

**Files to Modify:**
- `src/apophysis_xml.rs` - Fix import, implement export
- `tests/apophysis_xml_tests.rs` (NEW) - Round-trip tests
- `src/app/export.rs` - Add XML export menu option

**Expected Outcome:**
- Perfect round-trip: Import → Export → Import produces identical results
- Colors match Apophysis exactly for all test flames

---

### Phase 3.6: Advanced Features (Future/Optional)

**Goal:** Implement remaining Apophysis color features.

#### 3.6.1 - Plugin Color Influence
- [ ] Add `plugin_color: f32` to variation system
- [ ] Allow variations to modify color coordinate
- [ ] Implement blending formula: `c = c + pluginColor * (vc - c)`

#### 3.6.2 - Two-Color Dimensions (Experimental)
- [ ] Research Apophysis two-color mode (currently unused)
- [ ] Decide if implementation is valuable
- [ ] If yes: Extend point structure with `c1, c2` coordinates

#### 3.6.3 - Direct Color Variations
- [ ] Identify which variations should affect color
- [ ] Add color parameter to variation function signatures
- [ ] Update shader builder to pass color coordinate

**Files to Modify:**
- TBD based on feature decisions

**Expected Outcome:**
- Advanced color features for experimental fractals
- Full parity with Apophysis color system

---

## Testing Strategy

### Unit Tests
- [ ] Color coordinate evolution formula
- [ ] Opacity probability check
- [ ] Vibrancy blend formula
- [ ] Hue rotation RGB↔HSV conversion
- [ ] White level scaling

### Integration Tests
- [ ] Import known Apophysis flames
- [ ] Compare rendered output pixel-by-pixel
- [ ] Test all color modes
- [ ] Test edge cases (opacity=0, vibrancy=0, etc.)

### Visual Regression Tests
- [ ] Create baseline renders from Apophysis
- [ ] Render same flames in fflame-rust
- [ ] Compare images with SSIM or PSNR metrics
- [ ] Acceptable threshold: >99% similarity

### Round-Trip Tests
- [ ] Import → Export → Import → Compare configs
- [ ] Verify all parameters preserved
- [ ] Test with complex flames (many transforms, variations, parameters)

---

## Success Criteria

**Phase 3 Complete when:**
1. ✅ Color coordinate evolution matches Apophysis formula
2. ✅ Palette mode works correctly with color_speed
3. ✅ Opacity creates stochastic transparency
4. ✅ Vibrancy controls color algorithm blend
5. ✅ Hue rotation and white level implemented
6. ✅ XML import/export round-trips perfectly
7. ✅ Visual regression tests pass (>99% similarity)
8. ✅ All existing tests still pass

---

## Dependencies

**Before Starting:**
- Phase 2 complete (parameter system)
- All existing tests passing
- Documentation up to date

**External Resources:**
- Apophysis 7X source code (reference implementation)
- Test flame library (known good flames)
- Color science utilities (RGB↔HSV conversion)

---

## Estimated Effort

| Phase | Complexity | Time Estimate |
|-------|-----------|---------------|
| 3.1 - Color Coordinate | Medium | 2-3 hours |
| 3.2 - Opacity | Easy | 1 hour |
| 3.3 - Vibrancy | Medium | 2 hours |
| 3.4 - Palette Enhancements | Medium | 2-3 hours |
| 3.5 - XML Round-Trip | Hard | 4-5 hours |
| 3.6 - Advanced Features | Hard | 6-8 hours (optional) |
| **Total Core (3.1-3.5)** | | **11-14 hours** |

---

## Open Questions

1. **Color speed range:** Should we keep 0-1 in UI but map to -1 to 1 internally?
2. **Transform mode with palette:** Should Transform mode ignore palette, or allow hybrid?
3. **Speed mode compatibility:** Does Apophysis have a speed-based color mode?
4. **Plugin colors:** Which variations actually use direct color in Apophysis?
5. **Two-color mode:** Is this worth implementing if unused in Apophysis?

---

## References

- [Apophysis 7X Source Code](https://github.com/xyrus02/apophysis-7x)
- [docs/main/COLOR.md](../main/COLOR.md) - Current color system
- [docs/COLOR_PIPELINE.md](../COLOR_PIPELINE.md) - Color pipeline details
- [src/apophysis_xml.rs](../../src/apophysis_xml.rs) - XML import/export
- [shaders/core/main_2d.wgsl](../../shaders/core/main_2d.wgsl) - Color generation

---

**Created:** 2025-01-04
**Updated:** 2025-01-04
**Status:** In Progress
**Completed:** Phase 3.1 (Color Coordinate Evolution)
**Next Step:** Phase 3.2 - Opacity (Stochastic Transparency)
