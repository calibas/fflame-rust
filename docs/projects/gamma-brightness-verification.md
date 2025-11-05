# Gamma and Brightness Verification

**Date:** 2025-01-05
**Status:** Analysis for Phase 3.3 sub-task

## Apophysis Implementation

### UI Slider Conversion
From Adjust.pas:1033 and Adjust.pas:1041:
```pascal
cp.Gamma := scrollGamma.Position / 100;
cp.Brightness := ScrollBrightness.Position / 100;
```
Both sliders divide by 100 before storing in `fcp.gamma` and `fcp.brightness`.

### Gamma Application
From ImageMaker.pas:408-411:
```pascal
if fcp.gamma = 0 then
  gamma := fcp.gamma
else
  gamma := 1 / fcp.gamma;
```
**Gamma is inverted** (1/gamma) before use!

Applied to brightness/alpha at ImageMaker.pas:597:
```pascal
alpha := power(fp[3], gamma);
```

Applied to color channels in "old algorithm" at ImageMaker.pas:613-615:
```pascal
ri := Round(ls * fp[0] + notvib * power(fp[0], gamma));
gi := Round(ls * fp[1] + notvib * power(fp[1], gamma));
bi := Round(ls * fp[2] + notvib * power(fp[2], gamma));
```

**Effect:**
- gamma < 1: Darkens (because inverted: 1/0.5 = 2.0)
- gamma = 1: No change (1/1 = 1.0)
- gamma > 1: Brightens (because inverted: 1/2 = 0.5)

### Brightness Application
From ControlPoint.pas:39:
```pascal
BRIGHT_ADJUST = 2.3;
```

From ImageMaker.pas:450:
```pascal
k1 := (fcp.Contrast * BRIGHT_ADJUST * fcp.brightness * 268 * PREFILTER_WHITE) / 256.0;
```

From ImageMaker.pas:457:
```pascal
lsa[i] := (k1 * log10(1 + fcp.White_level * i * k2)) / (fcp.White_level * i);
```

Brightness is:
1. Divided by 100 from UI
2. Multiplied by 2.3 (BRIGHT_ADJUST)
3. Used in logarithmic density-to-brightness mapping (k1 scaling factor)

**Effect:**
- Higher brightness values increase overall image brightness
- UI value 1.0 → brightness = 1.0 → k1 scaling factor ~2.3x

---

## Our Current Implementation

### FractalConfig
```rust
pub gamma: f32,  // Default: 1.0
pub exposure: f32,  // Default: 1.0 (acts as brightness)
```

### UI Sliders (tone_mapping.rs:55-56)
```rust
ui.lazy_slider(config_manager, ConfigPath::Gamma, -1.0..=5.0, "Gamma")
ui.lazy_slider(config_manager, ConfigPath::Exposure, 0.01..=10.0, "Exposure")
```

**NO DIVISION BY 100!** Values are used directly.

### Shader Application (tonemap.wgsl)

**Gamma in vibrancy blend (line 120):**
```wgsl
let old_algo = pow(color, vec3<f32>(tonemap_params.gamma));  // NOT INVERTED!
```

**Final gamma correction (line 128):**
```wgsl
color = pow(color, vec3<f32>(1.0 / tonemap_params.gamma));  // Inverted here
```

**Exposure/brightness (line 64):**
```wgsl
color *= tonemap_params.exposure;  // Simple multiplication
```

---

## Issues Found

### Issue 1: Gamma Not Inverted in Vibrancy Blend
**Current (line 120):**
```wgsl
let old_algo = pow(color, vec3<f32>(tonemap_params.gamma));
```

**Should be (matching Apophysis ImageMaker.pas:613):**
```wgsl
let old_algo = pow(color, vec3<f32>(1.0 / tonemap_params.gamma));
```

Apophysis inverts gamma BEFORE using it in power(), not after.

### Issue 2: UI Slider Ranges Don't Match Apophysis
**Current:**
- Gamma: -1.0 to 5.0 (used directly)
- Exposure: 0.01 to 10.0 (used directly)

**Apophysis:**
- Gamma slider: 0 to 500 (divided by 100 → 0.0 to 5.0)
- Brightness slider: 0 to 500 (divided by 100 → 0.0 to 5.0)

**Problem:** Our slider values are NOT divided by 100!

**Proposed fix:**
```rust
// Option 1: Divide by 100 in shader (like vibrancy)
ui.lazy_slider(config_manager, ConfigPath::Gamma, 0.0..=500.0, "Gamma")
// Then in shader: let gamma = tonemap_params.gamma / 100.0;

// Option 2: Use 0-5 range but document it matches Apophysis/100
ui.lazy_slider(config_manager, ConfigPath::Gamma, 0.0..=5.0, "Gamma (0-5 = Apo 0-500)")
```

### Issue 3: Brightness Not Logarithmic
**Current:**
```wgsl
color *= tonemap_params.exposure;  // Simple linear multiplication
```

**Apophysis:**
```pascal
lsa[i] := (k1 * log10(1 + fcp.White_level * i * k2)) / (fcp.White_level * i);
```
Brightness affects logarithmic density-to-brightness mapping.

**However:** This may be acceptable for our implementation since we use a different rendering pipeline. Apophysis uses a histogram with bucket counts, while we use direct accumulation.

---

## Recommended Fixes

### Priority 1: Fix Gamma Inversion in Vibrancy Blend
Change line 120 in tonemap.wgsl:
```wgsl
let old_algo = pow(color, vec3<f32>(1.0 / tonemap_params.gamma));  // Invert gamma
```

This matches Apophysis behavior where gamma is inverted before power().

### Priority 2: Document UI Range Difference
If we keep the 0-5 range (NOT 0-500 divided by 100), document that:
- Our gamma 1.0 = Apophysis gamma 100
- Our gamma 4.0 = Apophysis gamma 400

This is acceptable since the effect is identical, just different scaling.

### Priority 3 (Optional): Add Brightness Slider Separate from Exposure
If we want exact Apophysis compatibility:
- Keep `exposure` for our own brightness control (linear)
- Add separate `brightness` parameter matching Apophysis (0-500 / 100)
- Apply `brightness` to affect the density-to-color mapping differently

**However:** This may be overkill. Exposure works well for our purposes.

---

## Vibrancy Scaling Issue (CRITICAL)

**Problem:** With vibrancy=1.0 in our UI:
- vib = 1.0 / 100 * 256 = 2.56
- notvib = 256 - 2.56 = 253.44
- Result: Almost entirely OLD algorithm (notvib/256 = 99% old!)

But based on user feedback, vibrancy=1.0 should give high contrast (new algorithm dominant).

**Analysis of Apophysis Vibrancy Slider:**
From user: "vibrancy slider 0-30 -> divide by 100"

But what is the DEFAULT vibrancy value?
- If default is 100 (not 1), then: vib = 100/100 * 256 = 256, notvib = 0 ✅
- If default is 1, then: vib = 1/100 * 256 = 2.56, notvib = 253.44 ❌

**Hypothesis:** Apophysis stores vibrancy as 0-100 BEFORE dividing by 100.
- UI slider: 0-100 range (not 0-1!)
- Storage: fcp.vibrancy = slider_value (0-100)
- Usage: vib = fcp.vibrancy * 256 (NO division by 100 at this point!)

Then the division by 100 happens elsewhere (probably in the UI display or in brightness calculations).

**Revised Understanding:**
```pascal
// At render time (ImageMaker.pas:412):
vib := round(fcp.vibrancy * 256.0);  // fcp.vibrancy is 0-1 (already divided by 100)
```

So if the UI slider is 0-100, it gets divided by 100 BEFORE storing in fcp.vibrancy.
Then at render time, it's multiplied by 256.

**For our implementation:**
- UI slider: 0-30 range
- If we want vibrancy=1.0 to mean "full new algorithm":
  - vib = vibrancy * 256 = 256
  - DON'T divide by 100!

**Proposed fix:**
```wgsl
let vib = tonemap_params.vibrancy * 256.0;  // NO division by 100!
let notvib = 256.0 - vib;
```

But this means our UI range of 0-30 would give vib = 0-7680, which is way too high.

**Alternative: UI stores 0-1 range, not 0-30**
- Change UI slider to 0.0-1.0 range
- Then vib = vibrancy * 256 works correctly
- vibrancy=1.0 → vib=256 (full new algorithm)
- vibrancy=0.0 → vib=0 (full old algorithm)

---

## Conclusion

**Multiple issues found:**

1. ✅ **FIXED:** Gamma inversion in vibrancy blend (line 120)
2. ❌ **BROKEN:** Vibrancy scaling is wrong - dividing by 100 when we shouldn't
3. ❌ **BROKEN:** UI slider range should be 0.0-1.0, not 0-30

**User reported behavior confirms vibrancy scaling is broken:**
- With vibrancy=1.0, gamma has huge effect (because we're using 99% old algorithm)
- Should be using 100% new algorithm at vibrancy=1.0
