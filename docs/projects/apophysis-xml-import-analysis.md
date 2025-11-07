# Apophysis XML Import Analysis - Phase 3.5

## Overview

This document analyzes the current state of Apophysis .flame XML import/export support in fflame-rust, comparing what we currently parse versus what Apophysis 7X provides.

**Reference XML:** Using Apo7X-251106-1 flame with extensive parameter coverage.

---

## Flame-Level Attributes

### Currently Imported ✅

| Attribute | Apophysis Value | Import Status | Mapping |
|-----------|----------------|---------------|---------|
| `name` | "Apo7X-251106-1" | ✅ Imported | → `Flame.name` |
| `size` | "808 748" | ✅ Imported | → Internal (not used in config) |
| `center` | "0.14 -0.009" | ✅ Imported | → `pan_x, pan_y` |
| `scale` | "245.73978575817" | ✅ Imported | → `zoom` (scale/200) |
| `background` | "0 0 0" | ✅ Imported | → `background_color` (0-255 → 0-1) |
| `brightness` | "6.7304347826087" | ✅ Imported | → `brightness` |
| `gamma` | "3.9" | ✅ Imported | → `gamma` (×2.2 conversion) |

### Partially Imported ⚠️

| Attribute | Apophysis Value | Import Status | Issue |
|-----------|----------------|---------------|-------|
| `version` | "Apophysis 7x Version 15D" | ⚠️ Parsed but ignored | Could validate compatibility |

### NOT Currently Imported ❌

| Attribute | Apophysis Value | Impact | Priority |
|-----------|----------------|--------|----------|
| `angle` | "0.302116493520218" | Camera angle (radians) | **HIGH** - Affects view |
| `rotate` | "-17.31" | View rotation (degrees) | **HIGH** - We have `rotation` field! |
| `zoom` | "-0.202" | Additional zoom factor | **MEDIUM** - Compounds with scale |
| `vibrancy` | "1.44" | Color algorithm blend | **HIGH** - We support this! |
| `gamma_threshold` | "0.0504782608695652" | Low-density gamma smoothing | **HIGH** - We support this! |
| `oversample` | "1" | Antialiasing (1/2/4) | **LOW** - Quality control |
| `filter` | "0.2" | Spatial filter radius | **LOW** - Blur/sharpen |
| `quality` | "1" | Iteration density | **MEDIUM** - Maps to our iteration counts |
| `estimator_radius` | "9" | Density estimation radius | **LOW** - Advanced DE feature |
| `estimator_minimum` | "0" | DE minimum threshold | **LOW** - Advanced DE feature |
| `estimator_curve` | "0.4" | DE curve shape | **LOW** - Advanced DE feature |
| `enable_de` | "0" | Density estimation toggle | **LOW** - We don't have DE |
| `cam_pitch` | "-0.000872664625997165" | 3D camera pitch | **HIGH** - We support this! |
| `cam_yaw` | "0.00610865238198015" | 3D camera yaw | **HIGH** - We support this! |
| `cam_perspective` | "-0.0003" | Perspective strength | **MEDIUM** - We have projection! |
| `cam_zpos` | "-0.002" | Camera Z position | **MEDIUM** - Affects 3D view |
| `plugins` | "" | Plugin list | **LOW** - Empty in this flame |
| `new_linear` | "1" | Linear variation behavior | **LOW** - Compatibility flag |
| `curves` | "0 0 1 ... 1 1 1" | Tone curve data | **MEDIUM** - We have ToneCurve! |

---

## Transform-Level Attributes (xform)

### Currently Imported ✅

| Attribute | Example Value | Import Status | Mapping |
|-----------|--------------|---------------|---------|
| `weight` | "0.333333333333333" | ✅ Imported | → `Transform.weight` |
| `color` | "0", "0.5", "1" | ✅ Imported | → Palette coordinate (0-1) |
| `coefs` | "a c b d e f" | ✅ Imported | → `a,b,c,d,e,f` (reordered) |
| `opacity` | "1", "0.967" | ✅ Imported | → `Transform.opacity` |
| All variation weights | "sinusoidal", "flatten" | ✅ Imported | → `variations` HashMap |
| Variation parameters | (implicit) | ✅ Imported | → `variation_params` HashMap |

### Partially Imported ⚠️

| Attribute | Example Value | Import Status | Issue |
|-----------|--------------|---------------|-------|
| `symmetry` / `color_speed` | "-0.069" | ✅ Imported | Alias handling works |

### NOT Currently Imported ❌

| Attribute | Example Value | Impact | Priority |
|-----------|--------------|--------|----------|
| `var_color` | "0.942" | Direct color influence | **MEDIUM** - Plugin color feature |
| `plotmode` | "off" | Disable plotting (opacity=0) | **LOW** - Rare, can set opacity=0 |
| `animate` | "1" | Animation flag | **LOW** - Animation not supported |
| `post` coefs | "pa pb pc pd pe pf" | Post-affine transform | **LOW** - Advanced feature |

---

## Palette-Level

### Currently Imported ✅

| Feature | Import Status | Mapping |
|---------|---------------|---------|
| Palette hex data | ✅ Imported | → `Palette` with 256 colors |
| RGB format | ✅ Imported | Converts hex to RGB floats |

### NOT Currently Imported ❌

| Attribute | Example | Impact | Priority |
|-----------|---------|--------|----------|
| `count` | "256" | Palette size | **LOW** - Always 256 in practice |
| `format` | "RGB" | Color format | **LOW** - Always RGB in practice |
| `index` | (int) | Palette hue rotation | **LOW** - We have palette_rotation |

---

## Critical Missing Imports (HIGH Priority)

These features are **already implemented** in fflame-rust but not imported from XML:

### 1. View Rotation (`rotate` attribute)
- **Apophysis:** `-17.31` degrees
- **Our field:** `FractalConfig.rotation` (radians)
- **Conversion:** `rotation = degrees * (PI / 180.0)`
- **Fix:** Add `"rotate"` case in `parse_flame_element()`, lines 78-111

### 2. Vibrancy (`vibrancy` attribute)
- **Apophysis:** `1.44`
- **Our field:** `FractalConfig.vibrancy`
- **Conversion:** Direct copy (same scale)
- **Fix:** Add `"vibrancy"` case in `parse_flame_element()`

### 3. Gamma Threshold (`gamma_threshold` attribute)
- **Apophysis:** `0.0504782608695652`
- **Our field:** `FractalConfig.gamma_threshold`
- **Conversion:** Direct copy
- **Fix:** Add `"gamma_threshold"` case in `parse_flame_element()`

### 4. Camera Rotation (`cam_pitch`, `cam_yaw`)
- **Apophysis:** `cam_pitch="-0.000872664625997165"`, `cam_yaw="0.00610865238198015"`
- **Our fields:** `FractalConfig.camera_rotation_x`, `camera_rotation_y`
- **Conversion:** Direct copy (radians)
- **Fix:** Add `"cam_pitch"` and `"cam_yaw"` cases

### 5. Perspective Projection (`cam_perspective`)
- **Apophysis:** `-0.0003`
- **Our field:** `ProjectionType::Perspective { strength: f32 }`
- **Conversion:** `if cam_perspective != 0.0 { ProjectionType::Perspective { strength: abs(cam_perspective) } }`
- **Fix:** Parse and set `flame.projection`

### 6. Tone Curve (`curves` attribute)
- **Apophysis:** "0 0 1 0 0 1 0.1125 0.861702127659574 1 1 1 1 ..." (48 floats = 4 curves × 12 values)
- **Our field:** `FractalConfig.tonemap_curve` (ToneCurve struct)
- **Conversion:** Parse 48-float array, extract RGB curves (indices 12-35)
- **Fix:** Add `parse_curves()` function, integrate in `parse_flame_element()`

---

## Medium Priority Missing Imports

### 7. Additional Zoom (`zoom` attribute)
- **Apophysis:** `-0.202`
- **Our field:** Could compound with existing `zoom` calculation
- **Conversion:** `final_zoom = (scale / 200.0) * 2.0^zoom`
- **Note:** Apophysis uses both `scale` and `zoom` multiplicatively

### 8. View Angle (`angle` attribute)
- **Apophysis:** `0.302116493520218` radians
- **Potential field:** Could add `view_angle` to FractalConfig
- **Impact:** Affects camera orientation (separate from rotation)

### 9. Camera Z Position (`cam_zpos`)
- **Apophysis:** `-0.002`
- **Potential field:** Could add to ProjectionType
- **Impact:** Affects 3D perspective strength

---

## Low Priority / Not Applicable

### Quality/Rendering Settings
- `oversample` - We use fixed MSAA or none
- `filter` - We don't have spatial filtering
- `quality` - Maps conceptually to our iteration counts, but different formula
- `estimator_radius/minimum/curve` - Density estimation not implemented
- `enable_de` - We don't have density estimation

### Compatibility Flags
- `new_linear` - Affects linear variation behavior (always "1" in modern Apophysis)
- `plugins` - Plugin list (we auto-detect from variations)
- `version` - Could validate but not critical

### Advanced Transform Features
- `var_color` - Plugin direct color (not implemented)
- `plotmode="off"` - Can be handled by setting opacity=0
- `post` coefs - Post-affine transform (not implemented)

---

## Export Requirements

For round-trip compatibility, we need to **export** everything we import:

### Currently NOT Exported (but imported)
- `name` - Easy, write `flame.name`
- `size` - Write default "1920 1080" or actual render size
- `center` - Convert `pan_x, pan_y` back
- `scale` - Convert `zoom` back (`scale = zoom * 200.0`)
- `background` - Convert 0-1 back to 0-255
- `brightness` - Write `brightness`
- `gamma` - Convert back (divide by 2.2)
- All transform attributes
- Palette hex data

### Need to Add to Export
- `rotate` - Convert radians to degrees
- `vibrancy` - Write directly
- `gamma_threshold` - Write directly
- `cam_pitch` - Write `camera_rotation_x`
- `cam_yaw` - Write `camera_rotation_y`
- `cam_perspective` - Extract from `ProjectionType::Perspective`
- `curves` - Serialize `tonemap_curve` to 48-float format

---

## Implementation Plan for Phase 3.5

### Step 1: Add Missing Imports (High Priority) ✅
1. ✅ Parse `rotate` → `rotation`
2. ✅ Parse `vibrancy` → `vibrancy`
3. ✅ Parse `gamma_threshold` → `gamma_threshold`
4. ✅ Parse `cam_pitch` → `camera_rotation_x`
5. ✅ Parse `cam_yaw` → `camera_rotation_y`
6. ✅ Parse `cam_perspective` → `ProjectionType`
7. ✅ Parse `curves` → `tonemap_curve`

### Step 2: Implement XML Export
1. Create `export_flame_xml(config: &FractalConfig) -> String`
2. Write `<flames>` wrapper
3. Write `<flame>` attributes (all imported fields + new ones)
4. Write `<xform>` elements (all variations + parameters)
5. Write `<palette>` element (hex format)

### Step 3: Round-Trip Testing
1. Import test XML → FractalConfig
2. Export FractalConfig → XML
3. Import exported XML → FractalConfig2
4. Compare config == config2
5. Visual comparison (render both, check pixel difference)

### Step 4: Documentation
1. Document import/export mappings
2. Document Apophysis compatibility notes
3. Add examples to CLAUDE.md

---

## Test Cases Needed

### Import Tests
- [ ] Parse rotation (degrees → radians)
- [ ] Parse vibrancy
- [ ] Parse gamma_threshold
- [ ] Parse camera rotations
- [ ] Parse perspective projection
- [ ] Parse tone curves (48-float format)
- [ ] Handle missing attributes (use defaults)

### Export Tests
- [ ] Export basic flame (minimal attributes)
- [ ] Export complex flame (all attributes)
- [ ] Export transforms with all variations
- [ ] Export palette (hex format)
- [ ] Validate XML structure (well-formed)

### Round-Trip Tests
- [ ] Import → Export → Import (preserve all values)
- [ ] Compare rendered images (pixel-perfect)
- [ ] Test with Apophysis-generated flames
- [ ] Test with fflame-rust-generated flames

---

## Apophysis Compatibility Notes

### Gamma Conversion
- **Apophysis:** gamma=1.0 is "no gamma correction"
- **fflame-rust:** gamma=2.2 is standard sRGB
- **Import:** Multiply by 2.2 (`our_gamma = apo_gamma * 2.2`)
- **Export:** Divide by 2.2 (`apo_gamma = our_gamma / 2.2`)

### Rotation Angle
- **Apophysis:** Degrees
- **fflame-rust:** Radians
- **Import:** `rotation = degrees * (PI / 180.0)`
- **Export:** `degrees = rotation * (180.0 / PI)`

### Scale/Zoom Mapping
- **Apophysis:** `scale` is pixels-per-unit (200 = 1:1)
- **fflame-rust:** `zoom` is multiplier (1.0 = 1:1)
- **Import:** `zoom = scale / 200.0`
- **Export:** `scale = zoom * 200.0`

### Color Coordinate
- **Apophysis:** `color` attribute is 0.0-1.0
- **Internal:** Maps to palette index 0-255
- **Palette mode:** Store as grayscale RGB (all channels same)
- **Transform mode:** Use actual RGB from palette lookup

### Perspective Strength
- **Apophysis:** `cam_perspective` can be negative
- **fflame-rust:** `ProjectionType::Perspective { strength }` is absolute
- **Import:** Use `abs(cam_perspective)`
- **Sign:** Negative values in Apophysis mean "reverse perspective" (rarely used)

---

## Summary Statistics

### Current Import Coverage
- **Flame attributes:** 7/28 (25%)
- **Transform attributes:** 5/8 (62.5%)
- **Palette:** 1/1 (100%)
- **Overall:** ~40% coverage

### After Phase 3.5 (Target)
- **Flame attributes:** 14/28 (50%) - focusing on implemented features
- **Transform attributes:** 5/8 (62.5%) - var_color, post not planned
- **Palette:** 1/1 (100%)
- **Overall:** ~55% coverage (all implemented features mapped)

### Features We Support But Don't Import Yet
1. ✅ `rotation` - We have it, just not importing from XML
2. ✅ `vibrancy` - Fully implemented, missing XML import
3. ✅ `gamma_threshold` - Fully implemented, missing XML import
4. ✅ `camera_rotation_x/y` - Fully implemented for 3D mode
5. ✅ `ProjectionType::Perspective` - Have the feature, need cam_perspective import
6. ✅ `tonemap_curve` - Have ToneCurve system, need curves import

**Priority:** Fix these 6 imports first - they unlock full compatibility for our existing feature set.

---

**Created:** 2025-01-06
**Status:** Analysis Complete
**Next Step:** Implement missing imports (Step 1)
