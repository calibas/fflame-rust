# Density-Aware Color Targeting - POC Test Results

**Date:** 2025-10-27
**Status:** ⚠️ **INVALID TESTS - CONDUCTED ON WRONG COMMIT**
**Conclusion:** All tests were conducted on commit 16b972c, which was BEFORE overflow fixes were merged. The observed "corruption" was actually u16 histogram overflow at scale=100, NOT a fundamental problem with density-aware adjustments.

## ⚠️ CRITICAL ERROR IN TEST METHODOLOGY

**The Problem:** Tests were conducted on an outdated commit that still had histogram overflow issues.

**Timeline of Confusion:**
1. User created PR #6 (batched-accumulation) which fixed overflow with true u32 unpacked histogram
2. PR was merged to main on GitHub
3. Tests were conducted on local commit 16b972c WITHOUT pulling the merged changes
4. Commit 16b972c was still using u16 packed histogram with scale=100 (max 655 hits before overflow)
5. All observed "corruption" was actually overflow wraparound, not density-aware adjustment issues

**Current Status (after git fetch):**
- HEAD is now at correct commit with true u32 unpacked histogram (4 words per pixel)
- Overflow is actually fixed (can handle ~42M hits per pixel)
- Test 2 (brightness compression) needs to be re-tested on correct commit
- Previous conclusions about "double-application" and "mathematical imbalance" may be incorrect

**Recommendation:** Re-run all tests on current HEAD with overflow actually fixed

---

## Test Summary

Tested multiple approaches to adjust colors based on pixel density (sparse vs dense areas). All approaches resulted in **color corruption in extremely dense areas**, making this technique unsuitable for production use.

---

## Test 1: Saturation Boost in Dark Areas (Post-Tone-Mapping)

**Implementation:** HSV conversion, boost saturation in low-density areas
```wgsl
// Lines 87-153 in tonemap.wgsl (initial POC)
let sat_dark = 1.5;    // Boost saturation 1.5x in dark areas
let sat_bright = 0.7;  // Reduce saturation 0.7x in bright areas
let sat_mult = mix(sat_dark, sat_bright, d);
s = clamp(s * sat_mult, 0.0, 1.0);
```

**Result:** ❌ **Grey halo around dense areas**
- Reducing saturation in bright areas creates washed-out grey appearance
- The visual problem is brightness, not saturation
- User feedback: "grey halo around the denser areas"

**Conclusion:** Saturation adjustment alone doesn't solve the problem

---

## Test 2: Brightness Compression (Post-Tone-Mapping)

**Implementation:** Simple RGB multiplier based on density
```wgsl
// Applied after tone curve (line 93-101, first attempt)
let d = clamp(density * tonemap_params.density_scale, 0.0, 1.0);
let brightness_mult = mix(1.0, 0.75, d);  // 25% darker in bright areas
fractal_color *= brightness_mult;
```

**Result:** ❌ **Color corruption in extremely dense areas**
- Bright areas looked better initially (less washed out)
- BUT: Densest areas showed complete color corruption
- User feedback: "color corruption now in the densest areas! Completely undoes what I've been working to do."

**Root Cause:** Using raw density without sqrt compression created mismatch with line 53's `sqrt(density)` scaling

---

## Test 3: Brightness Compression with sqrt() Fix (Post-Tone-Mapping)

**Implementation:** Fixed density normalization to match line 53
```wgsl
// Line 94-100, second attempt
let d = clamp(sqrt(density * tonemap_params.density_scale), 0.0, 1.0);
let brightness_mult = mix(1.0, 0.8, d);  // 20% darker (reduced from 25%)
fractal_color *= brightness_mult;
```

**Result:** ❌ **Still corrupted in extremely dense areas**
- Fixed the sqrt mismatch
- Reduced darkening intensity (0.8 instead of 0.75)
- User feedback: "Nope, color corruption is still there in the extremely dense areas."

**Conclusion:** The issue isn't the sqrt - it's applying adjustments post-tone-mapping

---

## Test 4: Brightness Compression Before Tone Mapping

**Implementation:** Moved adjustment before tone mapping, gamma, and clamping
```wgsl
// Lines 59-70, moved before exposure/log/gamma
if (density > 0.001) {
    let d = clamp(sqrt(density * tonemap_params.density_scale), 0.0, 1.0);
    let brightness_mult = mix(1.0, 0.85, d);  // 15% darker
    color *= brightness_mult;  // Applied to full dynamic range colors
}
// Then tone mapping, gamma, clamp happen after
```

**Result:** ❌ **STILL corrupted in extremely dense areas**
- Moved before all tone mapping operations
- Colors have full dynamic range (not clamped yet)
- Gentler adjustment (0.85 = only 15% darker)
- User feedback: "Nope, still there in the extremely dense areas."

**Conclusion:** Even pre-tone-mapping adjustment causes corruption at extreme densities

---

## Root Cause Analysis

### Why Corruption Occurs

**The mathematical relationship is fragile:**

1. **Accumulation buffer stores:** `color_sum / density` (averaged colors) + `density` (alpha)
2. **Tonemap shader applies:** `color = (color_sum / density) * sqrt(density * scale)`
3. **Any additional density-based adjustment breaks this relationship**

**In extremely dense areas:**
- `density` can be >> 1.0 (e.g., 10.0, 50.0, 100.0+)
- `sqrt(density * scale)` compresses but doesn't normalize to [0,1]
- Our adjustment: `color *= mix(1.0, 0.85, clamp(sqrt(density * scale), 0.0, 1.0))`
- At high density: clamps to 1.0, applies maximum darkening (0.85x)
- But the underlying sqrt(density) keeps growing → mathematical imbalance

**The fundamental issue:**
- We're trying to adjust based on density
- But colors are ALREADY density-adjusted (line 56: `color *= normalized_density`)
- This is a **double-application** of density information
- Creates corruption at extreme values where the math breaks down

---

## Why This Approach Cannot Work

### Problem 1: Double Application of Density
```
Accumulate shader:     color = sum / density (averaging)
Tonemap line 56:       color *= sqrt(density)  (scaling)
Our adjustment:        color *= f(density)     (second scaling)
                       ^^^^^^ DOUBLE APPLICATION
```

### Problem 2: Mathematical Inconsistency
The tone mapping pipeline carefully maintains color correctness:
- sqrt() compression for perceptual uniformity
- Log mapping for HDR → LDR
- Gamma correction for display

Any additional density-based scaling disrupts these carefully balanced transformations.

### Problem 3: Extreme Values
In extremely dense areas (density >> 1.0):
- Normalized density saturates our interpolation range
- Clamping to [0,1] loses information about actual density
- Darkening is applied uniformly at max density
- But underlying color values continue growing → corruption

---

## Alternative Approaches Considered

### Option A: Adjust in Accumulate Shader ❌
**Why rejected:** Would affect progressive rendering quality, permanent change to accumulated values

### Option B: Use Existing Controls ✅
**Recommended alternative:**
- Adjust `density_scale` parameter (already exists)
- Use tone curve (1D LUT) for artistic adjustments
- Use exposure/gamma controls
- Accept architectural limitations

### Option C: Pre-Accumulation (Compute Shader) ❌
**Why rejected:**
- No density info available during compute pass
- Would need previous frame's density (temporal lag)
- Race conditions during accumulation
- Chicken-and-egg problem

---

## Corrected Understanding (Post-Investigation)

**The "corruption" was histogram overflow, not a fundamental problem with the approach.**

### What Actually Happened

**Test Environment (16b972c):**
- Using u16 packed histogram (2× u32 with 16-bit channels)
- Scale = 100
- Max capacity = 65,535 / 100 = **655 hits before overflow**
- Dense fractal areas easily exceed 655 hits → **u16 wraparound corruption**

**Correct Environment (current HEAD after PR #6 merge):**
- Using u32 unpacked histogram (4× u32 separate channels)
- Scale = configurable via `histogram_color_scale` parameter
- Max capacity = 4,294,967,295 / scale = **~42 million hits** (at default scale)
- No overflow in practical use cases

### Re-Evaluation Needed

**Test 2 (Brightness Compression) showed promise** before "corruption" appeared:
- Initial feedback: "Looks better"
- Applied 20% darkening to dense areas
- May actually work correctly on overflow-free histogram

**The "double-application" theory may be incorrect:**
- The math `color *= mix(1.0, 0.8, normalized_density)` is a simple post-process
- Doesn't actually interact with the `color *= sqrt(density)` scaling at line 56
- Those are sequential operations, not compounding
- The "corruption" was overflow wraparound, not mathematical imbalance

### Next Steps

1. Re-test Test 2 on current HEAD with true u32 histogram
2. Verify no corruption occurs with brightness compression
3. If successful, parameterize the adjustment amount
4. Add UI controls for density-aware brightness adjustment

## Original (Invalid) Lessons Learned

**NOTE: The following conclusions were based on faulty tests and may not be accurate:**

1. **Density is already baked into the color pipeline**
   - Colors are averaged by density (accumulate shader)
   - Colors are scaled by density (tonemap shader)
   - Adding more density adjustments = double-application

   **↑ This may be incorrect - needs re-evaluation with proper histogram**

2. **Tone mapping math is fragile**
   - Carefully balanced transformations
   - Adding per-pixel adjustments breaks correctness
   - Extreme values expose mathematical issues

3. **Post-accumulation adjustments have limits**
   - Initial analysis said "post-accumulation is easy" (wrong!)
   - Density is available, but using it correctly is hard
   - Some artistic goals aren't achievable within current architecture

4. **Test with extreme values**
   - Moderate densities may look fine
   - Extreme densities (100×+ hits) expose corruption
   - Need to test zoomed-out views with very dense centers

---

## Recommendations

### ❌ Do NOT Implement Density-Aware Color Targeting
**Verdict:** This approach is fundamentally flawed and causes unacceptable visual corruption.

### ✅ Use Existing Controls Instead

**For controlling brightness in dense areas:**
1. **Density Scale** slider (already exists)
   - Lower values = darker overall
   - Affects all densities uniformly

2. **Tone Mapping Mode** (already exists)
   - Logarithmic mode compresses bright areas
   - Better than linear for high dynamic range

3. **Exposure** slider (already exists)
   - Global brightness control
   - Simple and predictable

4. **Tone Curve** (already exists)
   - 1D LUT for artistic adjustments
   - Can create S-curves or custom mappings
   - Applied uniformly (not density-aware)

5. **Gamma** slider (already exists)
   - Adjusts mid-tone brightness
   - Standard display correction

**For artistic effects:**
- Combine existing controls creatively
- Adjust palette colors directly
- Use transform colors (color_mode = Transform)
- Accept that some effects aren't feasible

---

## Code Cleanup Required

**Remove POC code from `shaders/tonemap.wgsl`:**
- Lines 59-70: POC density-aware adjustment (Test 4)
- Lines 100-162: Commented HSV approach (Test 1/2)

**Revert to clean tonemap shader** without experimental code.

---

## Final Verdict

**Status:** ❌ **Closed - Will Not Implement**

**Reasoning:**
- All 4 test approaches failed with color corruption
- Fundamental architectural limitation
- Existing controls provide sufficient artistic control
- Risk/complexity not justified by uncertain benefits

**Alternative path forward:**
- Document existing controls better
- Create presets showcasing different tone mapping settings
- Focus on other high-priority features (tiled export, randomize, etc.)

---

**Last Updated:** 2025-10-27
**Tests Conducted:** 4 approaches, all failed
**User Feedback:** Color corruption unacceptable
**Recommendation:** Abandon this feature
