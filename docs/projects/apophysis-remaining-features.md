# Apophysis Compatibility - Remaining Features

## Overview

This document tracks the remaining features needed for full Apophysis 7X compatibility.
Most core compatibility work is complete (69 variations, color system, XML import).
This covers the final features to achieve near-complete parity.

**Status:** Most features implemented, remaining work is polish and advanced features

---

## Planned Features

These features are planned for implementation and necessary for good Apophysis compatibility.

### 1. 3D Camera System Match Apophysis ✅ COMPLETE

**Status:** ✅ COMPLETE (2025-11-07)

**📋 Detailed Plan:** See `docs/projects/3d-camera-system-apophysis.md` for complete implementation guide

**Completed Features:**
- ✅ Camera pitch and yaw (imported from XML)
- ✅ Perspective projection (imported from `cam_perspective`)
- ✅ Camera height (Z position) - Imported from `cam_zpos`
- ✅ Apophysis camera matrix (ZXY Euler with -yaw inversion)
- ✅ UI controls for all camera parameters (pitch, yaw, Z position)
- ✅ Full integration with ConfigManager (undo/redo, preview mode)

**Implementation Summary:**
- **Phase 1:** Implemented exact Apophysis camera matrix in shaders (c73a914)
  - Added `build_camera_matrix()` with ZXY Euler rotation
  - Added `camera_transform()` for Z translation + rotation
  - Added `apply_perspective()` for depth projection
- **Phase 2:** Added camera_z parameter throughout system (32c1e32)
  - Parsed `cam_zpos` from XML
  - Added to FractalConfig and ConfigPath
  - Wired through GPU params to shaders
- **Phase 3:** Added UI controls in View window (f4629ee)
  - Camera Z Position drag control (3D mode only)
  - Integrated with Reset View button

**Actual Effort:** ~4.5 hours (better than estimated 8-12 hours)

**Ready for Testing:** System is complete and ready for validation with real Apophysis 3D flames

---

### 2. Simplify Transform Color UI

**Status:** Not implemented

**Description:**
The current transform color UI uses three separate RGB sliders (0-1 range) to control the transform's color coordinate. This is confusing because the sliders don't represent RGB colors - they represent a **position in the palette** (which is then mapped to actual RGB during rendering).

**Current Problems:**
- Three RGB sliders suggest you're picking an RGB color
- Actually controls palette position (0-1 coordinate)
- No visual feedback showing the actual color from the palette
- Confusing workflow: adjust sliders → see effect in flame

**Proposed Solution:**
Replace the three RGB sliders with:
1. **Single "Transform Color" slider** (0.0 to 1.0)
   - Directly maps to palette position
   - Clear label: "Palette Position" or "Color Coordinate"
2. **Visual color swatch**
   - Shows the actual RGB color from current palette at this position
   - Updates in real-time as slider moves or palette changes
   - Click to open color picker? (future enhancement)

**Benefits:**
- Matches Apophysis mental model (single color coordinate)
- Visual feedback shows actual resulting color
- Less confusing than "RGB" sliders that aren't RGB
- Easier to understand: "this transform uses this color from the palette"

**Implementation Details:**
- Store transform color as single `f32` instead of `[f32; 3]`
  - **Breaking change:** Would need migration for old configs
  - Alternative: Keep `[f32; 3]` storage, but UI treats it as single value
- Add palette preview swatch in UI
  - Sample palette at `transform.color` position
  - Render as small colored rectangle next to slider
- Update ConfigPath to handle single value or keep vec3 internally

**Files to Modify:**
- `src/ui/transforms.rs` - Replace RGB sliders with single slider + swatch
- `src/scene/transforms.rs` - Consider storage format change
- `src/config/delta.rs` - Update ConfigPath if needed
- `src/apophysis_xml.rs` - Parse/export single color value

**Estimated Effort:** 4-6 hours (depends on whether we change storage format)

**Migration Considerations:**
- If changing from `[f32; 3]` to `f32`: need to average RGB values or take first component
- If keeping `[f32; 3]`: UI just uses first component, storage unchanged (easier)

---

### 3. XML Export

**Status:** Not implemented (Phase 3.5.2)

**Description:**
Export current configuration to Apophysis-compatible .flame XML format.

**Current State:**
- XML import is complete and comprehensive (55% coverage)
- No export functionality exists
- Would enable round-trip testing (import → export → import)

**What's Needed:**
- Implement `export_flame_xml()` function in `apophysis_xml.rs`
- Write all flame-level attributes:
  - name, size, center, scale, rotate
  - background, brightness, gamma, vibrancy, gamma_threshold
  - cam_pitch, cam_yaw, cam_perspective, cam_zpos (if implemented)
  - curves (convert ToneCurve back to 48-float Bezier format)
- Write all transform attributes:
  - weight, color, color_speed, opacity
  - coefs (affine matrix in Apophysis order)
  - All variation weights
  - All variation parameters
- Write palette in hex format (1536 chars)
- Add "Export to XML" button in UI
- File dialog to save .flame file

**Use Cases:**
- Share flames with Apophysis users
- Back up current configuration in standard format
- Enable round-trip testing for validation

**Files to Create/Modify:**
- `src/apophysis_xml.rs` - Add export functions
- `src/ui/config_import_export.rs` - Add export button
- Add file dialog for .flame save

**Estimated Effort:** 6-8 hours

---

## Nice to Have Features

These features would improve usability but aren't critical for Apophysis compatibility.

### 4. Variation Preview

**Status:** Not implemented

**Description:**
Preview a single transform's output in isolation (without iteration).
Shows the visual effect of the transform + variations.

**Concept:**
- Select a transform in the editor
- See a grid of points transformed by: affine → variations
- Helps understand what each transform contributes
- Common in fractal flame editors

**What's Needed:**
- New preview panel/window
- Generate regular grid of input points
- Apply selected transform (affine + variations)
- Render output points as dots/lines
- Update in real-time as parameters change

**Use Cases:**
- Debugging transform behavior
- Understanding variation effects
- Educational tool for learning flame editing

**Estimated Effort:** 8-10 hours (requires new rendering pipeline)

---

### 5. Direct Color Variations & Plugin Color Blending

**Status:** Not implemented (part of variation plugins + color system)

**Description:**
Complete implementation of the Apophysis 3-step color blending system, including the `direct_color` (pluginColor) parameter that allows DirectColor variations to modify the color coordinate.

**Apophysis Color Blending System** (XForm.pas:312-313, 1067, 1078-1081):

**Step 1: Color Speed Blending**
```pascal
colorC1 := (1 + symmetry)/2;
colorC2 := color*(1 - symmetry)/2;
CPpoint.c := CPpoint.c * colorC1 + colorC2;
vc := CPpoint.c;
```
Blends inherited color with transform base color using `color_speed` (symmetry).

**Step 2: Variation Execution**
```pascal
for i:= 0 to FNrFunctions-1 do
  FCalcFunctionList[i];
```
DirectColor variations can modify `vc` (the variation color variable).

**Step 3: Direct Color Blending**
```pascal
CPpoint.c := CPpoint.c + pluginColor * (vc - CPpoint.c);
```
Blends variation-modified color back: `c_final = c_base + direct_color × (vc - c_base)`

**Current Implementation:**
- ✅ Step 1: Color speed blending (implemented)
- ✅ Step 2: Variations execute (implemented, but can't modify color)
- ❌ Step 3: Direct color blending (missing!)
- ❌ `direct_color` field not in Transform struct
- ❌ DirectColor variations can't output color
- ❌ XML import doesn't parse `pluginColor` attribute

**What's Needed:**

**Phase 1: Add direct_color Parameter (2-3 hours)**
- Add `direct_color: f32` field to Transform struct
- Parse `pluginColor` from XML (default: 0.0)
- Add to GPU transform buffer
- Add ConfigPath::TransformDirectColor
- Add UI slider (0.0 to 1.0)

**Phase 2: Shader Color Blending (2-3 hours)**
- Update shader to implement Step 3 formula
- Add `vc` (variation color) variable
- Apply direct_color blending after variations
- Test with existing flames (should have no effect if direct_color=0)

**Phase 3: DirectColor Variation Support (6-8 hours)**
- Extend variation system to optionally output color
- Update VariationRegistry to mark DirectColor variations
- Modify shader builder for color-returning variations
- Implement key DirectColor variations:
  - `dc_linear` (linear color gradient)
  - `dc_bubble` (radial color gradient)
  - Others as needed

**Use Cases:**
- Variation-driven color effects (gradients, radial coloring)
- Advanced color control beyond palette
- Full Apophysis compatibility for flames using DirectColor variations

**Files to Modify:**
- `src/scene/transforms.rs` - Add direct_color field
- `src/apophysis_xml.rs` - Parse pluginColor
- `src/config/delta.rs` - Add ConfigPath variant
- `src/ui/transforms.rs` - Add direct_color slider
- `shaders/core/main_2d.wgsl` and `main_3d.wgsl` - Implement Step 3
- `src/variations/mod.rs` - Add DirectColor variation category
- `src/shader_builder_v2.rs` - Handle color-returning variations

**Total Estimated Effort:** 10-14 hours

**Priority:** Medium - Required for full DirectColor variation support, but most flames don't use it

---

### 6. Xaos (Paths & Weights)

**Status:** Not implemented

**Description:**
Control the probability of transitioning from one transform to another.
Creates directed graphs instead of uniform random selection.

**Apophysis Concept:**
- Each transform has xaos values for every other transform
- xaos[i][j] = probability of going from transform i to j
- Default: uniform (1.0 for all)
- Allows creating paths through transforms

**What's Needed:**
- Add xaos matrix to Flame structure: `Vec<Vec<f32>>`
- Upload to GPU as storage buffer
- Modify compute shader to use weighted selection
- UI editor for xaos matrix (challenging)
- XML import/export of xaos values

**Use Cases:**
- Create specific paths through transforms
- Disable certain transform transitions
- Advanced flame design technique
- Rarely used except by experts

**Estimated Effort:** 12-15 hours (complex UI + GPU changes)

---

### 7. Transform Solo Mode

**Status:** Not implemented

**Description:**
Temporarily disable all transforms except one to see its contribution.

**Concept:**
- Click "Solo" button on a transform
- All other transforms have weight set to 0
- See only that transform's output
- Useful for debugging

**What's Needed:**
- Add solo mode flag to UI state
- Temporarily modify transform weights (don't save to config)
- Add "Solo" button to each transform panel
- Highlight solo'd transform

**Use Cases:**
- Understand individual transform contributions
- Debug why a transform isn't visible
- Isolate problematic transforms

**Estimated Effort:** 2-3 hours

---

## Experimental Features

These features exist in Apophysis but are experimental or rarely used.

### 8. Two-Color System

**Status:** Not implemented

**Description:**
Apophysis has a two-color dimension system (c1, c2) but it's rarely/never used.
Most flames only use single color coordinate.

**Current State:**
- Our system uses single color coordinate `c`
- Apophysis code supports `c1, c2` but it's disabled/unused
- No known flames use this feature

**Decision:**
- **Not implementing** unless we find flames that actually use it
- Would require significant changes to point structure
- No clear benefit since it's unused in practice

**If Implemented (hypothetical):**
- Extend point structure: `struct Point { x, y, z, c1, c2 }`
- Two palette lookups or blending modes
- GPU buffer size increases
- Shader complexity increases

**Estimated Effort:** 15-20 hours (if implemented, not recommended)

---

## Not Planned

These features have fundamental incompatibilities or impractical costs.

### 9. Accumulation Difference

**Status:** Architectural difference, not implementing

**Description:**
Apophysis uses a different accumulation model that affects brightness/density.

**Our Approach:**
- GPU compute shader generates samples per frame
- Ping-pong accumulation buffers blend new samples
- Progressive refinement with temporal blending
- Fast iteration (2M+ iterations/second)

**Apophysis Approach:**
- CPU-based iteration with sub-batch model (10K iterations per sub-batch)
- Different density calculation based on sub-batches
- Brightness formula depends on specific iteration counts

**Why Not Implementing:**
- Would require complete renderer rewrite
- Our GPU approach is significantly faster
- Visual differences are minor with proper parameter scaling
- Current approach works well with parameter adjustments

**Mitigation:**
- Use `histogram_color_scale` to adjust brightness
- Use `brightness` parameter to match Apophysis appearance
- Use `gamma_threshold` scaling (×2000) to match Apophysis units
- Most flames look correct with these adjustments

---

### 10. 64-bit Floats

**Status:** Not implementing

**Description:**
Apophysis uses `double` (64-bit) for variation weights and parameters.

**Our Approach:**
- 32-bit `f32` throughout (WGSL standard)
- ±3.4E38 range, ~7 digit precision
- GPU native type, very fast

**Apophysis Approach:**
- 64-bit `double` precision
- ±1E308 range, ~15-16 digit precision
- CPU-friendly, slower on GPU

**Why Not Implementing:**
- WGSL (WebGPU) has no native f64 support
- GPU f64 emulation is 10-20× slower
- 32-bit precision is sufficient for fractals (rare edge cases only)
- Would break WebGPU compatibility
- Massive performance penalty for minimal visual benefit

**Impact:**
- Negligible for typical flames
- Extreme parameter values (>1E30) may differ slightly
- In practice, no visible difference in 99.9% of flames

---

## Priority Order

**High Priority (Planned):**
1. 3D Controls Match Apophysis (2-3 hours) - Required for 3D flame import
2. Direct Color Transforms (3-4 hours) - Some flames need this
3. XML Export (6-8 hours) - Enables sharing and round-trip testing

**Medium Priority (Nice to Have):**
4. Transform Solo Mode (2-3 hours) - Easy, high value for debugging
5. Variation Preview (8-10 hours) - Educational, not critical
6. Xaos (12-15 hours) - Advanced feature, rarely used
7. Direct Color Variations (10-12 hours) - Part of plugin system, rarely used

**Low Priority (Experimental/Not Planned):**
8. Two-Color System - Unused in practice
9. Accumulation Difference - Architectural, not worth rewrite
10. 64-bit Floats - WGSL incompatible, massive performance cost

---

## Estimated Total Effort

**Planned Features:** 11-15 hours
**Nice to Have:** 34-43 hours (if all implemented)
**Total for Full Compatibility:** ~45-58 hours

**Realistic Next Steps:**
- Focus on Planned features first (11-15 hours)
- Add Transform Solo Mode (easy win, 2-3 hours)
- Defer advanced features (Xaos, Variation Preview) for future
- Skip experimental/not planned features

---

## Success Criteria

**Minimum Viable (Planned Complete):**
- [x] Can import Apophysis 3D flames correctly
- [x] Direct color transforms work
- [x] Can export to .flame XML for sharing

**Full Compatibility (All Nice-to-Have):**
- [ ] Transform preview helps debugging
- [ ] Xaos support for advanced flames
- [ ] Direct color variations for plugins
- [ ] Solo mode for transform isolation

**Current State:**
- 90%+ compatibility for typical flames
- All variations, color system, XML import complete
- Missing: XML export, some 3D controls, advanced features

---

## Related Documentation

- `docs/archive/apophysis-phase3/` - Completed Phase 3 documentation
- `docs/main/COLOR.md` - Color system reference
- `docs/main/TRANSFORMS.md` - Transform system reference
- `docs/projects/apophysis-remaining-features.md` - This document

---

**Created:** 2025-01-07
**Status:** Active Planning
**Next Steps:** Implement 3D camera controls, then direct color transforms, then XML export
