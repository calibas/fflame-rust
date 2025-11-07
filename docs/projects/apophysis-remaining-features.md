# Apophysis Compatibility - Remaining Features

## Overview

This document tracks the remaining features needed for full Apophysis 7X compatibility.
Most core compatibility work is complete (69 variations, color system, XML import).
This covers the final features to achieve near-complete parity.

**Status:** Most features implemented, remaining work is polish and advanced features

---

## Planned Features

These features are planned for implementation and necessary for good Apophysis compatibility.

### 1. 3D Camera System Match Apophysis

**Status:** Partially implemented, needs correction

**📋 Detailed Plan:** See `docs/projects/3d-camera-system-apophysis.md` for complete implementation guide

**Current State:**
- ✅ Camera pitch and yaw (imported from XML)
- ✅ Perspective projection (imported from `cam_perspective`)
- ⚠️ Camera height (Z position) - Not imported
- ⚠️ Perspective strength - Imported but may need UI adjustment
- ❌ **Critical Issue:** Axis reference frame problem

**Problems:**
1. **Axis Reference Frame:** Our camera rotation uses world-space axes, not view-relative
   - Apophysis rotates around view-relative axes (camera's current orientation)
   - We rotate around fixed world axes (X, Y, Z)
   - Result: Different behavior when combining pitch and yaw

2. **Gimbal Lock:** Susceptible to gimbal lock with Euler angles
   - At pitch = ±90°, yaw and roll become equivalent
   - Loses one degree of freedom
   - Camera can get "stuck" in certain orientations

**What Apophysis Does:**
- Uses view-relative rotation (rotate around camera's current axes)
- Applies rotations in specific order (yaw, then pitch)
- Avoids gimbal lock in typical usage

**What We Need:**
1. **Import `cam_zpos`** - Camera Z position/height
2. **Fix rotation system:**
   - Option A: Switch to quaternions (avoids gimbal lock)
   - Option B: Use view-relative Euler angles (matches Apophysis)
   - Option C: Use rotation matrices with proper composition
3. **Add UI controls:**
   - Camera Pitch (view-relative X rotation)
   - Camera Yaw (view-relative Y rotation)
   - Camera Height (Z position)
   - Perspective Strength
4. **Test against Apophysis 3D flames:**
   - Verify identical camera behavior
   - Test edge cases (pitch near ±90°)

**Recommended Approach:**
- **Option B (View-Relative Euler):** Closest to Apophysis, easier to implement
- Store pitch/yaw as separate values
- Apply in correct order: world_to_camera = rotate_pitch(rotate_yaw(point))
- Update rotation incrementally (not absolute)

**Files to Modify:**
- `src/apophysis_xml.rs` - Parse `cam_zpos`
- `src/config/fractal_config.rs` - Add camera_z field
- `src/config/delta.rs` - Add ConfigPath::CameraZ
- `src/ui/` - Add 3D camera controls panel
- `shaders/core/utilities.wgsl` - Fix camera rotation to use view-relative axes
- `src/scene/transforms.rs` - Update camera rotation system

**Estimated Effort:** 8-12 hours (4 phases: camera matrix fix, camera Z, UI controls, testing)

**Implementation Phases:**
1. **Camera Matrix Fix** (2-3 hours) - Use exact Apophysis ZXY Euler rotation formula
2. **Camera Z Position** (2-3 hours) - Import and use `cam_zpos`
3. **UI Controls** (2-3 hours) - Expose pitch/yaw/z/perspective sliders
4. **Testing & Validation** (2-3 hours) - Verify against Apophysis reference flames

---

### 2. Direct Color Transforms

**Status:** Not implemented

**Description:**
In Apophysis, transforms can use direct RGB colors instead of palette coordinates.
This is separate from the palette-based color system.

**Current State:**
- We have `ColorMode::Transform` but it's not properly implemented
- Transform colors are stored but not used correctly
- XML import sets color mode but doesn't distinguish direct vs palette

**What's Needed:**
- Implement `ColorMode::Transform` to use direct RGB from transforms
- Update shader to check color mode and use `transform.color` as RGB
- Update XML import to detect direct color vs palette mode
- Add UI toggle to switch between palette and direct color modes

**Use Case:**
- Some Apophysis flames use direct RGB colors per transform
- Allows more precise color control without palette
- Common in simple flames with 2-3 transforms

**Files to Modify:**
- `shaders/core/main_2d.wgsl` and `main_3d.wgsl` - Check color mode
- `src/apophysis_xml.rs` - Detect direct color mode
- `src/ui/color_settings.rs` - Add color mode selector
- `src/scene/color.rs` - Document ColorMode::Transform behavior

**Estimated Effort:** 3-4 hours

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

### 5. Direct Color Variations

**Status:** Not implemented (part of variation plugins)

**Description:**
Some variations can directly modify the color coordinate, not just position.
This is part of the plugin color system.

**Apophysis Concept:**
- Variations have optional `plugin_color` parameter
- Variation can return new color: `(x', y', z', c')`
- Formula: `c_out = c_in + pluginColor × (variation_c - c_in)`

**Current State:**
- Our variations only return position: `(x, y)` or `(x, y, z)`
- No color output from variations
- Would require variation signature changes

**What's Needed:**
- Extend variation system to optionally return color
- Add `plugin_color` parameter to VariationParameter
- Update shader builder to handle color-returning variations
- Identify which Apophysis variations use direct color (rare)

**Use Cases:**
- Advanced color effects
- Variation-driven coloring
- Rarely used in practice

**Estimated Effort:** 10-12 hours (requires variation system redesign)

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
