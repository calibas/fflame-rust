# Apophysis 7X Full Compatibility Plan

**Goal:** Achieve 100% compatibility with Apophysis 7X Version 15D

**Current Status:** 40/80+ variations implemented (50%)
- Phase 1 (Waves2, Julia3D): ✅ COMPLETE
- Phase 2 (Core variations 26-37): ✅ COMPLETE

**Remaining Work:** 3 major phases

---

## Phase 1: Extended Variations (40 variations)

Implement all remaining Apophysis 7X variations to reach parity with Version 15D.

### 1.1 Basic Extended Variations (No Parameters)
**Count: 14 variations**

1. **log** - Logarithmic transform
2. **polar2** - Improved polar coordinates
3. **cross** - Cross/plus shape
4. **loonie** - Lune/crescent shape
5. **escher** - Escher-style tessellation
6. **scry** - Crystal ball effect
7. **foci** - Focal point distortion
8. **bipolar** - Bipolar coordinates
9. **elliptic** - Elliptic coordinates
10. **lazysusan** - Rotating lazy susan effect
11. **pre_spherical** - Pre-phase spherical (variation 40)
12. **pre_sinusoidal** - Pre-phase sinusoidal (variation 41)
13. **pre_disc** - Pre-phase disc (variation 42)
14. **falloff2** - Falloff with parameters

### 1.2 Parameterized Extended Variations
**Count: 26 variations**

15. **rings2** - Ring patterns (1 param: val)
16. **fan2** - Fan effect (2 params: x, y)
17. **wedge** - Wedge shape (4 params: angle, hole, count, swirl)
18. **epispiral** - Epicycloid spiral (4 params: thickness, n, holes, waves)
19. **bwraps** - B-wraps transform (3 params: cellsize, space, gain)
20. **pdj** - Peter de Jong attractor (4 params: a, b, c, d)
21. **juliascope** - Julia scope (2 params: power, dist)
22. **julia3Dz** - Julia 3D Z-variant (1 param: power)
23. **curl** - Curl distortion (2 params: c1, c2)
24. **curl3D** - 3D curl (3 params: cx, cy, cz)
25. **radial_blur** - Radial blur effect (1 param: angle)
26. **blur_circle** - Circular blur (no params, RNG)
27. **blur_zoom** - Zoom blur (no params, RNG)
28. **blur_pixelize** - Pixelize blur (2 params: size, scale)
29. **rectangles** - Rectangle tiling (2 params: x, y)
30. **splits** - Split transform (2 params: x, y)
31. **separation** - Separation effect (2 params: x, y)
32. **ngon** - N-gon shape (4 params: sides, power, circle, corners)
33. **mobius** - Möbius transformation (8 params: re_a, im_a, re_b, im_b, re_c, im_c, re_d, im_d)
34. **crop** - Crop/window (6 params: left, right, top, bottom, scatter_area, zero)
35. **auger** - Auger spiral (4 params: freq, weight, sym, scale)
36. **pre_bwraps** - Pre-phase bwraps (3 params)
37. **pre_crop** - Pre-phase crop (6 params)
38. **pre_falloff2** - Pre-phase falloff2 (parameters TBD)
39. **post_bwraps** - Post-phase bwraps (3 params)
40. **post_curl** - Post-phase curl (2 params)
41. **post_curl3D** - Post-phase curl3D (3 params)
42. **post_crop** - Post-phase crop (6 params)
43. **post_falloff2** - Post-phase falloff2 (parameters TBD)

**Total Phase 1: 43 variations (indices 40-82)**

### Implementation Strategy
1. Group by complexity: start with simple non-parameterized variations
2. Implement Pre/Post-phase variations together with their Normal-phase counterparts
3. Test each variation against Apophysis reference images
4. Add shader builder support for new parameter counts (Mobius has 8 params!)

---

## Phase 2: UI Parameter Range Fixes

**Problem:** Current UI enforces arbitrary limits on variation weights and parameters that don't match Apophysis behavior.

### 2.1 Variation Weight Ranges
**Current:** Variation weights limited to 0.0-2.0 range
**Apophysis:** Variation weights can be any value including negative

**Changes needed:**
- Remove min/max constraints on variation weight sliders
- Allow negative weights (important for artistic effects)
- Default range suggestion: -5.0 to 5.0 (but allow typing any value)
- Update ConfigPath::TransformVariation to handle negative weights
- Test that negative weights work correctly in shader (should already work)

### 2.2 Variation Parameter Ranges
**Current:** Each parameter has hardcoded min/max values
**Apophysis:** Most parameters allow wider or unlimited ranges

**Review needed for each parameter:**
- **JuliaN power**: Currently -10 to 10, check if Apophysis allows more
- **JuliaN dist**: Currently 0 to 5, **should allow negative values**
- **Blob high/low**: Currently 0 to 3, check if negative allowed
- **Blob waves**: Currently 1 to 20, check wider range
- **Waves2 freq**: Currently 0 to 10, check wider range
- **Waves2 scale**: Currently -2 to 2, **likely needs wider range**
- **Julia3D power**: Currently -10 to 10, verify sufficient

**Action items:**
1. Review Apophysis source for actual parameter limits
2. Identify parameters that should be unbounded
3. Update VariationParameter min/max values
4. Consider adding "extended range" mode for UI sliders
5. Add direct numeric input for unbounded parameters

### 2.3 Slider UX Improvements
**Goals:**
- Maintain usable slider ranges for common values
- Allow unlimited typed input for extreme values
- Show when value is outside "normal" range

**Implementation:**
- Keep reasonable slider ranges (e.g., -10 to 10)
- Add DragValue widget alongside slider for direct numeric input
- Visual indicator when value exceeds slider range
- "Reset to default" button for each parameter

---

## Phase 3: Color System Overhaul

**Problem:** Color system doesn't match Apophysis behavior, imported flames have wrong colors.

### 3.1 Investigation: Current vs Apophysis Color System

**Current system (hybrid):**
- Transform has `color` field (0.0-1.0)
- ColorMode enum: TransformColors, PaletteColors, SpeedBased
- Unclear how transform color interacts with palette
- Imported flames don't match Apophysis colors

**Apophysis system (need to verify):**
- Each transform has a color index (0.0-1.0)
- Color index is blended with palette
- Final color = palette[color_index]
- Speed-based coloring is separate mode

**Questions to answer:**
1. How does Apophysis blend transform color with iteration speed?
2. What is the exact formula for color accumulation?
3. How does the histogram color buffer work in Apophysis?
4. Is there transform color override or always palette lookup?

### 3.2 Color Mode Analysis

**Need to understand:**
- **Direct Transform Colors**: Does this exist in Apophysis?
- **Palette Colors**: Standard mode, color_index → palette lookup
- **Speed-Based**: How is "speed" calculated? Distance traveled per iteration?

**Action items:**
1. Read Apophysis color code (XForm.pas, Render.pas)
2. Document exact color calculation formula
3. Compare with our current implementation
4. Identify discrepancies

### 3.3 Color Accumulation Formula

**Current implementation:**
- Histogram buffer stores (R, G, B, Density) as u32 atomics
- Accumulation shader blends colors
- Unclear if this matches Apophysis

**Need to verify:**
- How does Apophysis accumulate color in histogram?
- Is it additive, averaged, or something else?
- How is density factored in?
- What is the tone mapping formula?

**Files to investigate:**
- `src/scene/color.rs` - ColorMode enum
- `src/renderer/compute_kernel.rs` - Color calculation
- `shaders/trajectory*.wgsl` - GPU color assignment
- `shaders/accumulate.wgsl` - Color accumulation

### 3.4 Implementation Plan

**Step 1: Document Apophysis behavior**
- Create `docs/COLOR_APOPHYSIS_ANALYSIS.md`
- Extract exact formulas from Apophysis source
- Document each color mode with examples

**Step 2: Implement matching system**
- Update ColorMode enum if needed
- Fix color calculation in compute shader
- Fix color accumulation in accumulate shader
- Update tone mapping if needed

**Step 3: Validate**
- Import reference flames from Apophysis
- Compare rendered output pixel-by-pixel
- Adjust until match is exact

**Step 4: Remove legacy code**
- Remove any hybrid color system remnants
- Simplify code to match Apophysis exactly
- Update documentation

---

## Success Criteria

**Phase 1 Complete:**
- [ ] All 43 extended variations implemented (variations 40-82)
- [ ] Shader builder supports up to 8 parameters (for Mobius)
- [ ] All variations tested against Apophysis reference images
- [ ] Documentation updated with all variation formulas

**Phase 2 Complete:**
- [ ] Variation weights can be negative
- [ ] All variation parameters have correct ranges
- [ ] Parameters can be unbounded where appropriate
- [ ] UI allows direct numeric input for extreme values
- [ ] Apophysis flames import with correct parameter values

**Phase 3 Complete:**
- [ ] Color system exactly matches Apophysis
- [ ] Imported flames render with identical colors
- [ ] All color modes (palette, speed-based) work correctly
- [ ] Histogram accumulation matches Apophysis
- [ ] Documentation explains color system clearly

**Overall Success:**
- [ ] Can import any Apophysis 7X flame and render identically
- [ ] All 80+ variations implemented and working
- [ ] Parameter ranges match Apophysis exactly
- [ ] Color output pixel-perfect match

---

## Timeline Estimate

**Phase 1 (Extended Variations):**
- Simple variations (14): ~3-4 hours
- Parameterized variations (26): ~6-8 hours
- Testing and debugging: ~2-3 hours
- **Total: ~12-15 hours**

**Phase 2 (Parameter Ranges):**
- Research Apophysis limits: ~2 hours
- Update VariationParameter definitions: ~1 hour
- UI improvements: ~2-3 hours
- Testing: ~1 hour
- **Total: ~6-7 hours**

**Phase 3 (Color System):**
- Research Apophysis color code: ~3-4 hours
- Document findings: ~1 hour
- Implementation: ~4-6 hours
- Testing and validation: ~2-3 hours
- **Total: ~10-14 hours**

**Grand Total: ~28-36 hours of work**

---

## Next Steps

1. **Start Phase 1:** Begin with simple extended variations (log, polar2, cross, etc.)
2. **Create variation implementation checklist** in this file
3. **Track progress** with completed variation count
4. **Test incrementally** - validate each variation before moving to next

## Phase 1 Progress Tracker

**Simple Variations (14):** 0/14 complete
- [ ] log
- [ ] polar2
- [ ] cross
- [ ] loonie
- [ ] escher
- [ ] scry
- [ ] foci
- [ ] bipolar
- [ ] elliptic
- [ ] lazysusan
- [ ] pre_spherical
- [ ] pre_sinusoidal
- [ ] pre_disc
- [ ] falloff2

**Parameterized Variations (26):** 0/26 complete
- [ ] rings2
- [ ] fan2
- [ ] wedge
- [ ] epispiral
- [ ] bwraps
- [ ] pdj
- [ ] juliascope
- [ ] julia3Dz
- [ ] curl
- [ ] curl3D
- [ ] radial_blur
- [ ] blur_circle
- [ ] blur_zoom
- [ ] blur_pixelize
- [ ] rectangles
- [ ] splits
- [ ] separation
- [ ] ngon
- [ ] mobius
- [ ] crop
- [ ] auger
- [ ] pre_bwraps
- [ ] pre_crop
- [ ] pre_falloff2
- [ ] post_bwraps
- [ ] post_curl
- [ ] post_curl3D
- [ ] post_crop
- [ ] post_falloff2

**Total: 0/43 variations complete (0%)**
