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

### 2.0 Precision Limitations (Important!)

**Apophysis Implementation (Pascal):**
- **Variation weights:** `double` (64-bit, ±1E308 range, ~15-16 digit precision)
- **Variation parameters:** `double` (64-bit, ±1E308 range, ~15-16 digit precision)
  - "Integer" parameters (e.g., JuliaN power, Julia3D power) stored as `double` internally
  - UI limits integer parameters to 32-bit signed int range (-2147483648 to 2147483647)
  - Actual calculations use full double precision
- **Smallest UI value:** ±1E-6 (hardcoded minimum in Apophysis UI)

**Our Implementation (WGSL f32):**
- **Variation weights:** `f32` (32-bit, ±3.4E38 range, ~7 digit precision)
- **Variation parameters:** `f32` (32-bit, ±3.4E38 range, ~7 digit precision)
  - "Integer" parameters stored as `f32`, converted to `i32` in shader where needed
  - No separate integer storage - all parameters are f32
- **Smallest value:** ±1.18E-38 (normalized), ±1.4E-45 (denormalized)

**Why We Can't Use f64:**
- ❌ **WGSL has NO f64 type** - WebGPU shading language only supports `f32` and `f16` (half precision)
- ❌ **No extension available** - Feature request exists ([gpuweb/gpuweb#2805](https://github.com/gpuweb/gpuweb/issues/2805)) but not implemented as of 2024
- ❌ **Not on roadmap** - No timeline for f64 support in WebGPU
- ⚠️ **Fundamental platform limitation** - Not a design choice, but a WebGPU constraint
- 🔍 **Investigated 2025-11-03** - Confirmed no workaround exists (no extension flags, no feature bits)

**Implications:**
- ✅ **We CAN support** Apophysis's smallest value (1E-6) - well within f32 range
- ⚠️ **We CANNOT match** Apophysis's full ±1E308 range (limited to ±3.4E38)
- ⚠️ **Precision loss:** f32's ~7 digits vs double's ~15-16 digits
  - May cause slight differences in renders at extreme iteration counts
  - Accumulated rounding errors over billions of iterations
  - Very large/very small parameter values will be rounded
- ✅ **Practical impact:** Minimal for vast majority of flames
- ⚠️ **Edge cases:**
  - Extremely large parameter values from Apophysis (>3.4E38) will clamp to ±3.4E38
  - High-precision parameter values will round to 7 significant digits

**Alternative Considered (Rejected):**
Using native compute APIs (CUDA/Vulkan/Metal) instead of WebGPU would allow f64:
- ❌ **Lose WASM/browser support** - major feature regression
- ❌ **Require platform-specific builds** - more complex, less portable
- ❌ **f64 GPU performance** - Many GPUs run f64 at 1/2 to 1/32 speed of f32
- ❌ **Limited GPU support** - Not all GPUs have f64 (especially mobile)
- ✅ **Only benefit:** Match Apophysis precision exactly (not worth trade-offs)

**Decision:** Accept f32 limitation as unavoidable WebGPU platform constraint.

**UI Implementation:**
- **Slider range:** -10.0 to 10.0 (practical range for common values)
- **Actual limits:** -3.4E38 to 3.4E38 (f32 max, enforced)
- **Direct input:** Allow typing any value, clamp to f32 range if exceeded
- **Validation:** Warn if |value| > 1E10 (unusual but valid)
- **Import handling:** Values imported from Apophysis may be slightly rounded due to f32 precision
  - No data loss for typical values (most parameters < 1000)
  - Precision loss only affects extreme edge cases

### 2.1 Variation Weight Ranges
**Current:** Variation weights limited to 0.0-2.0 range
**Apophysis:** Variation weights can be any double value including negative

**Changes needed:**
- Remove min/max constraints on variation weight sliders
- Allow negative weights (important for artistic effects)
- **Slider range:** -10.0 to 10.0 (covers 99% of use cases)
- **Actual limits:** -3.4E38 to 3.4E38 (f32 max)
- Update ConfigPath::TransformVariation to handle negative weights
- Test that negative weights work correctly in shader (should already work)

### 2.2 Variation Parameter Ranges
**Current:** Each parameter has hardcoded min/max values
**Apophysis:** Most parameters use double precision with ±1E308 theoretical range

**Standard approach for all parameters:**
- **Slider range:** Practical range for common use (e.g., -10 to 10)
- **Actual limits:** f32 range (±3.4E38)
- **Direct input:** Always available via DragValue widget

**Current parameters to fix:**
- **JuliaN power**: -10 to 10 slider (OK), but allow beyond via input
- **JuliaN dist**: 0 to 5 slider → **Change to -10 to 10** (negative values valid)
- **Blob high/low**: 0 to 3 slider → **Verify if negative needed**, widen range
- **Blob waves**: 1 to 20 slider → **Widen to -100 to 100** for flexibility
- **Waves2 freq**: 0 to 10 slider → **Widen to -100 to 100**
- **Waves2 scale**: -2 to 2 slider → **Widen to -10 to 10**
- **Julia3D power**: -10 to 10 slider (OK)

**New parameters from Phase 1 variations:**
- Use practical slider ranges based on typical usage
- All parameters allow f32 full range via direct input
- Document recommended ranges in variation registry

**Action items:**
1. Update all `VariationParameter` definitions to use wide min/max
2. Keep slider UI ranges practical (user can always type extreme values)
3. Add validation: clamp to f32 limits, warn if unusual
4. Test parameter edge cases (0, negative, very large, very small)

### 2.3 Slider UX Improvements
**Goals:**
- Maintain usable slider ranges for common values
- Allow f32 full range via typed input
- Clear feedback for unusual values

**Implementation:**
- **Slider:** Practical range (-10 to 10 or similar)
- **DragValue:** Always available, accepts f32 range
- **Validation:** Clamp to ±3.4E38, show warning if |value| > 1000
- **Visual indicator:** Different color if value outside slider range
- **Reset button:** Restore default value for each parameter
- **Tooltip:** Show actual value + (slider range) info

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

**Simple Variations (14):** 14/14 complete ✓
- [x] log
- [x] polar2
- [x] cross
- [x] loonie
- [x] escher
- [x] scry
- [x] foci
- [x] bipolar
- [x] elliptic
- [x] lazysusan
- [x] pre_spherical
- [x] pre_sinusoidal
- [x] pre_disc
- [x] falloff2

**Parameterized Variations (26):** 17/26 complete
- [x] rings2
- [x] fan2
- [x] wedge
- [x] epispiral
- [x] bwraps
- [x] pdj
- [x] juliascope
- [x] julia3Dz
- [x] curl
- [x] curl3D
- [x] radial_blur
- [x] blur_circle
- [x] blur_zoom
- [x] blur_pixelize
- [x] rectangles
- [x] splits
- [x] separation
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

**Total: 31/43 variations complete (72%)**
