//! Miscellaneous misc — fourth extras file
//!
//! Eight more standalone variations, mix of clean ports and
//! `needs_transform` cases:
//!
//!   - `anamorphcyl` (Sosa)              — anamorphic cylinder warp
//!   - `svf`         (gossamer light)    — 3D single-value-function
//!   - `shredlin`    (Zy0rg)             — linear shred grid
//!   - `shredrad`    (Zy0rg)             — radial shred (init bug fix recovered from Java)
//!   - `xheart`      (xyrus02)           — heart-shaped Möbius warp
//!   - `stwin`       (Apophysis pack)     — sin-weighted twin warp
//!   - `whorl`       (Apophysis pack)     — radial whorl with inside/outside knobs
//!   - `devil_warp`  (dark-beam)          — power-warp with rmin/rmax clamp
//!
//! Sources: each variation's `.cpp` file in
//! `output/jwildfire-vars/output/`.
//!
//! `whorl`, `stwin`, and `devil_warp` use `needs_transform` for
//! their internal-weight uses; the rest factor cleanly through
//! the outer multiplier.
//!
//! Faithfulness:
//!   - `shredrad` upstream cpp `PluginVarPrepare` is empty — it never
//!     sets `_alpha` even though the body reads it. Porter bug. The
//!     Java `setParameter` derives `alpha = 2π / n`. Recovered.
//!     Also preserves cpp's `atan2(x, y)` swap (Java uses
//!     `getPrecalcAtanYX()` = `atan2(y, x)`).
//!   - `whorl` body uses `atan2(FTx, FTy)` (cpp swap from Java's
//!     `getPrecalcAtan()` = `atan2(x, y)` actually — already
//!     matching). Preserved.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// anamorphcyl: anamorphic cylinder warp (Sosa)
//   out = a · (y + b) · (cos(k·x), sin(k·x))
// (cpp uses `FPx = ... * VVAR` (assignment); standard +=-friendly
// outer multiplier matches when anamorphcyl is the only normal
// variation.)
// =============================================================================
pub static ANAMORPHCYL: VariationDef = VariationDef {
    name: "anamorphcyl",
    display_name: "Anamorph Cyl",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("a", "A", unlimited_float, 1.0, -10.0, 10.0),
        param!("b", "B", unlimited_float, 1.3, -10.0, 10.0),
        param!("k", "K", unlimited_float, 3.0, -50.0, 50.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_anamorphcyl(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let k = get_param(xform_id, variation_id, 2u);
    let f = a * (p.y + b);
    return vec2<f32>(f * cos(k * p.x), f * sin(k * p.x));
}
"#,
    wgsl_3d: Some(r#"
fn variation_anamorphcyl(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let k = get_param(xform_id, variation_id, 2u);
    let f = a * (p.y + b);
    return vec3<f32>(f * cos(k * p.x), f * sin(k * p.x), p.z);
}
"#),
};

// =============================================================================
// svf: 3D single-value-function (gossamer light)
//   cn = cos(n · y)
//   out = (cy · cn · cx, cy · cn · sx, sy · cn)
//   where cx = cos(x), sx = sin(x), cy = cos(y), sy = sin(y)
// =============================================================================
pub static SVF: VariationDef = VariationDef {
    name: "svf",
    display_name: "SVF",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("n", "N", unlimited_float, 2.0, -50.0, 50.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_svf(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let n = get_param(xform_id, variation_id, 0u);
    let cn = cos(n * p.y);
    let sx = sin(p.x); let cx = cos(p.x);
    let cy = cos(p.y);
    return vec2<f32>(cy * cn * cx, cy * cn * sx);
}
"#,
    wgsl_3d: Some(r#"
fn variation_svf(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let n = get_param(xform_id, variation_id, 0u);
    let cn = cos(n * p.y);
    let sx = sin(p.x); let cx = cos(p.x);
    let sy = sin(p.y); let cy = cos(p.y);
    return vec3<f32>(cy * cn * cx, cy * cn * sx, sy * cn);
}
"#),
};

// =============================================================================
// shredlin: linear shred grid (Zy0rg)
//   out_x = w · xdistance · (frac(x/xdistance)·xwidth + floor(x/xdistance)
//                            + (0.5 − xpos)·(1 − xwidth))
//   (same form for y; cpp uses `FPx = vv · ...` assignment but VVAR
//   strictly outer)
// =============================================================================
pub static SHREDLIN: VariationDef = VariationDef {
    name: "shredlin",
    display_name: "Shred Lin",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("xdistance", "X distance", unlimited_float, 1.0, -10.0, 10.0),
        param!("xwidth", "X width", unlimited_float, 0.5, -10.0, 10.0),
        param!("ydistance", "Y distance", unlimited_float, 1.0, -10.0, 10.0),
        param!("ywidth", "Y width", unlimited_float, 0.5, -10.0, 10.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_shredlin(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let sxd = get_param(xform_id, variation_id, 0u);
    let sxw = get_param(xform_id, variation_id, 1u);
    let syd = get_param(xform_id, variation_id, 2u);
    let syw = get_param(xform_id, variation_id, 3u);
    let safe_sxd = select(sxd, 1e-30, abs(sxd) < 1e-30);
    let safe_syd = select(syd, 1e-30, abs(syd) < 1e-30);
    let xpos = select(0.0, 1.0, p.x < 0.0);
    let ypos = select(0.0, 1.0, p.y < 0.0);
    let xrng = p.x / safe_sxd;
    let yrng = p.y / safe_syd;
    let xrng_f = trunc(xrng);
    let yrng_f = trunc(yrng);
    return vec2<f32>(
        sxd * ((xrng - xrng_f) * sxw + xrng_f + (0.5 - xpos) * (1.0 - sxw)),
        syd * ((yrng - yrng_f) * syw + yrng_f + (0.5 - ypos) * (1.0 - syw)),
    );
}
"#,
    wgsl_3d: Some(r#"
fn variation_shredlin(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let sxd = get_param(xform_id, variation_id, 0u);
    let sxw = get_param(xform_id, variation_id, 1u);
    let syd = get_param(xform_id, variation_id, 2u);
    let syw = get_param(xform_id, variation_id, 3u);
    let safe_sxd = select(sxd, 1e-30, abs(sxd) < 1e-30);
    let safe_syd = select(syd, 1e-30, abs(syd) < 1e-30);
    let xpos = select(0.0, 1.0, p.x < 0.0);
    let ypos = select(0.0, 1.0, p.y < 0.0);
    let xrng = p.x / safe_sxd;
    let yrng = p.y / safe_syd;
    let xrng_f = trunc(xrng);
    let yrng_f = trunc(yrng);
    return vec3<f32>(
        sxd * ((xrng - xrng_f) * sxw + xrng_f + (0.5 - xpos) * (1.0 - sxw)),
        syd * ((yrng - yrng_f) * syw + yrng_f + (0.5 - ypos) * (1.0 - syw)),
        p.z,
    );
}
"#),
};

// =============================================================================
// shredrad: radial shred (Zy0rg)
//   alpha = 2π / n        (recovered from Java; cpp porter bug)
//   ang   = atan2(x, y)   (cpp's swapped form, preserved)
//   xang  = (ang + 3π + α/2) / α
//   zang  = (frac(xang) · w + floor(xang)) · α − π − α/2 · w
//   out   = r · (cos zang, sin zang)
// =============================================================================
pub static SHREDRAD: VariationDef = VariationDef {
    name: "shredrad",
    display_name: "Shred Rad",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("n", "N", unlimited_float, 4.0, 0.001, 100.0),
        param!("width", "Width", float, 0.5, -1.0, 1.0),
    ],
    needs_transform: false,
    writes_color: false,
    // 1 derived value at slot 2:
    //   2: alpha  (2π / n)        — porter-omitted from cpp; recovered from Java
    init_param_count: 1,
    wgsl_init: Some(r#"
fn init_shredrad(user: array<f32, 2>) -> array<f32, 1> {
    let n = user[0];
    let safe_n = select(n, 1e-6, abs(n) < 1e-6);
    let two_pi = 6.28318530717959;
    var out: array<f32, 1>;
    out[0] = two_pi / safe_n;
    return out;
}
"#),
    wgsl_2d: r#"
fn variation_shredrad(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let sw = get_param(xform_id, variation_id, 1u);
    let sa = get_param(xform_id, variation_id, 2u);
    let pi = 3.14159265358979;
    let three_pi = 9.42477796076938;

    let ang = atan2(p.x, p.y);  // cpp swap preserved
    let rad = sqrt(p.x * p.x + p.y * p.y) + 1e-6;
    let safe_sa = select(sa, 1e-30, abs(sa) < 1e-30);
    let xang = (ang + three_pi + sa * 0.5) / safe_sa;
    let xang_f = trunc(xang);
    let zang = ((xang - xang_f) * sw + xang_f) * sa - pi - sa * 0.5 * sw;
    return vec2<f32>(rad * cos(zang), rad * sin(zang));
}
"#,
    wgsl_3d: Some(r#"
fn variation_shredrad(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let sw = get_param(xform_id, variation_id, 1u);
    let sa = get_param(xform_id, variation_id, 2u);
    let pi = 3.14159265358979;
    let three_pi = 9.42477796076938;

    let ang = atan2(p.x, p.y);
    let rad = sqrt(p.x * p.x + p.y * p.y) + 1e-6;
    let safe_sa = select(sa, 1e-30, abs(sa) < 1e-30);
    let xang = (ang + three_pi + sa * 0.5) / safe_sa;
    let xang_f = trunc(xang);
    let zang = ((xang - xang_f) * sw + xang_f) * sa - pi - sa * 0.5 * sw;
    return vec3<f32>(rad * cos(zang), rad * sin(zang), p.z);
}
"#),
};

// =============================================================================
// xheart: heart-shaped Möbius warp (xyrus02)
//   ang = π/4 + π/8 · angle
//   rat = 6 + 2 · ratio
//   r2_4 = x² + y² + 4
//   bx = 4/r2_4;  by = rat/r2_4
//   x = cos(ang) · (bx·x) − sin(ang) · (by·y)
//   y = sin(ang) · (bx·x) + cos(ang) · (by·y)
//   if x > 0:  out = (x, y)  else: out = (x, -y)
// =============================================================================
pub static XHEART: VariationDef = VariationDef {
    name: "xheart",
    display_name: "X Heart",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("angle", "Angle", unlimited_float, 0.0, -10.0, 10.0),
        param!("ratio", "Ratio", unlimited_float, 0.0, -10.0, 10.0),
    ],
    needs_transform: false,
    writes_color: false,
    // 3 derived values at slots 2..5:
    //   2: rat   (6 + 2 · ratio)
    //   3: cosa  (cos(π/4 + π/8 · angle))
    //   4: sina  (sin(π/4 + π/8 · angle))
    init_param_count: 3,
    wgsl_init: Some(r#"
fn init_xheart(user: array<f32, 2>) -> array<f32, 3> {
    let pi_4 = 0.7853981633974483;  // π/4
    let pi_8 = 0.39269908169872414; // π/8
    let ang = pi_4 + pi_8 * user[0];
    var out: array<f32, 3>;
    out[0] = 6.0 + 2.0 * user[1];
    out[1] = cos(ang);
    out[2] = sin(ang);
    return out;
}
"#),
    wgsl_2d: r#"
fn variation_xheart(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let rat = get_param(xform_id, variation_id, 2u);
    let cosa = get_param(xform_id, variation_id, 3u);
    let sina = get_param(xform_id, variation_id, 4u);
    var r2_4 = p.x * p.x + p.y * p.y + 4.0;
    if (r2_4 == 0.0) { r2_4 = 1.0; }
    let bx = 4.0 / r2_4;
    let by = rat / r2_4;
    let x = cosa * (bx * p.x) - sina * (by * p.y);
    let y = sina * (bx * p.x) + cosa * (by * p.y);
    if (x > 0.0) {
        return vec2<f32>(x, y);
    }
    return vec2<f32>(x, -y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_xheart(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let rat = get_param(xform_id, variation_id, 2u);
    let cosa = get_param(xform_id, variation_id, 3u);
    let sina = get_param(xform_id, variation_id, 4u);
    var r2_4 = p.x * p.x + p.y * p.y + 4.0;
    if (r2_4 == 0.0) { r2_4 = 1.0; }
    let bx = 4.0 / r2_4;
    let by = rat / r2_4;
    let x = cosa * (bx * p.x) - sina * (by * p.y);
    let y = sina * (bx * p.x) + cosa * (by * p.y);
    if (x > 0.0) {
        return vec3<f32>(x, y, p.z);
    }
    return vec3<f32>(x, -y, p.z);
}
"#),
};

// =============================================================================
// stwin: sin-weighted twin warp (Apophysis pack)
//   x' = w · x · 0.05;  y' = w · y · 0.05
//   x2 = x'² + offset_x2 · 0.0001
//   y2 = y'² + offset_y2 · 0.0001
//   result = (x2 − y2) · sin(2π · distort · (x' + y' + offset_xy · 0.1))
//   result /= (x2 + y2)
//   FPx += w · x + result            (no VVAR on `result`)
//   FPy += w · y + result
//
// `result` lacks VVAR; output line `w · x + result` doesn't factor
// cleanly. Body uses `needs_transform` to read w and divide-out.
// =============================================================================
pub static STWIN: VariationDef = VariationDef {
    name: "stwin",
    display_name: "STwin",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("distort", "Distort", unlimited_float, 1.0, -10.0, 10.0),
        param!("offset_xy", "Offset XY", unlimited_float, 0.0, -10.0, 10.0),
        param!("offset_x2", "Offset X²", unlimited_float, 0.0, -10.0, 10.0),
        param!("offset_y2", "Offset Y²", unlimited_float, 0.0, -10.0, 10.0),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_stwin(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let distort = get_param(xform_id, variation_id, 0u);
    let off_xy = get_param(xform_id, variation_id, 1u);
    let off_x2 = get_param(xform_id, variation_id, 2u);
    let off_y2 = get_param(xform_id, variation_id, 3u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let two_pi = 6.28318530717959;

    let x = p.x * w * 0.05;
    let y = p.y * w * 0.05;
    let x2 = x * x + off_x2 * 0.0001;
    let y2 = y * y + off_y2 * 0.0001;
    var divident = x2 + y2;
    if (divident == 0.0) { divident = 1.0; }
    let result = (x2 - y2) * sin(two_pi * distort * (x + y + off_xy * 0.1)) / divident;
    let fx = w * p.x + result;
    let fy = w * p.y + result;
    return vec2<f32>(fx * inv_w, fy * inv_w);
}
"#,
    wgsl_3d: Some(r#"
fn variation_stwin(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let distort = get_param(xform_id, variation_id, 0u);
    let off_xy = get_param(xform_id, variation_id, 1u);
    let off_x2 = get_param(xform_id, variation_id, 2u);
    let off_y2 = get_param(xform_id, variation_id, 3u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let two_pi = 6.28318530717959;

    let x = p.x * w * 0.05;
    let y = p.y * w * 0.05;
    let x2 = x * x + off_x2 * 0.0001;
    let y2 = y * y + off_y2 * 0.0001;
    var divident = x2 + y2;
    if (divident == 0.0) { divident = 1.0; }
    let result = (x2 - y2) * sin(two_pi * distort * (x + y + off_xy * 0.1)) / divident;
    let fx = w * p.x + result;
    let fy = w * p.y + result;
    return vec3<f32>(fx * inv_w, fy * inv_w, p.z);
}
"#),
};

// =============================================================================
// whorl: radial whorl with inside/outside knobs (Apophysis pack)
//   r = sqrt(x²+y²) + ε
//   if r < w:   a = atan2(x, y) + inside  / (w − r)
//   else:        a = atan2(x, y) + outside / (w − r)
//   out = r · (cos a, sin a)            (VVAR strictly outer)
// =============================================================================
pub static WHORL: VariationDef = VariationDef {
    name: "whorl",
    display_name: "Whorl",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("inside", "Inside", unlimited_float, 0.1, -10.0, 10.0),
        param!("outside", "Outside", unlimited_float, 0.2, -10.0, 10.0),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_whorl(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let inside = get_param(xform_id, variation_id, 0u);
    let outside = get_param(xform_id, variation_id, 1u);
    let w = transforms[xform_id].variations[variation_id];
    let r = sqrt(p.x * p.x + p.y * p.y) + 1e-6;
    let denom = w - r;
    let safe_denom = select(denom, 1e-30, abs(denom) < 1e-30);
    var a = atan2(p.x, p.y);  // cpp swap preserved
    if (r < w) {
        a = a + inside / safe_denom;
    } else {
        a = a + outside / safe_denom;
    }
    return vec2<f32>(r * cos(a), r * sin(a));
}
"#,
    wgsl_3d: Some(r#"
fn variation_whorl(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let inside = get_param(xform_id, variation_id, 0u);
    let outside = get_param(xform_id, variation_id, 1u);
    let w = transforms[xform_id].variations[variation_id];
    let r = sqrt(p.x * p.x + p.y * p.y) + 1e-6;
    let denom = w - r;
    let safe_denom = select(denom, 1e-30, abs(denom) < 1e-30);
    var a = atan2(p.x, p.y);
    if (r < w) {
        a = a + inside / safe_denom;
    } else {
        a = a + outside / safe_denom;
    }
    return vec3<f32>(r * cos(a), r * sin(a), p.z);
}
"#),
};

// =============================================================================
// devil_warp: power-warp with rmin/rmax clamp (dark-beam)
//   r = (x²+y²+r2·b·y²)^warp − (y²+r2·a·x²)^warp     (r2 = 1/(x²+y²))
//   r = clamp(r, rmin, rmax) · effect
//   FPx += x · (1 + r);  FPy += y · (1 + r)         (no VVAR — divide-out)
// =============================================================================
pub static DEVIL_WARP: VariationDef = VariationDef {
    name: "devil_warp",
    display_name: "Devil Warp",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("a", "A", unlimited_float, 2.0, -10.0, 10.0),
        param!("b", "B", unlimited_float, 1.0, -10.0, 10.0),
        param!("effect", "Effect", unlimited_float, 1.0, -10.0, 10.0),
        param!("warp", "Warp", unlimited_float, 0.5, -10.0, 10.0),
        param!("rmin", "R min", unlimited_float, -0.24, -100.0, 100.0),
        param!("rmax", "R max", unlimited_float, 100.0, -1000.0, 1000.0),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_devil_warp(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let effect = get_param(xform_id, variation_id, 2u);
    let warp = get_param(xform_id, variation_id, 3u);
    let rmin = get_param(xform_id, variation_id, 4u);
    let rmax = get_param(xform_id, variation_id, 5u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let xx = p.x;
    let yy = p.y;
    let denom = xx * xx + yy * yy;
    let safe_denom = max(denom, 1e-30);
    let r2 = 1.0 / safe_denom;
    var r = pow(max(xx * xx + r2 * b * yy * yy, 1e-30), warp)
          - pow(max(yy * yy + r2 * a * xx * xx, 1e-30), warp);
    r = clamp(r, rmin, rmax);
    r = effect * r;
    return vec2<f32>(xx * (1.0 + r) * inv_w, yy * (1.0 + r) * inv_w);
}
"#,
    wgsl_3d: Some(r#"
fn variation_devil_warp(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let effect = get_param(xform_id, variation_id, 2u);
    let warp = get_param(xform_id, variation_id, 3u);
    let rmin = get_param(xform_id, variation_id, 4u);
    let rmax = get_param(xform_id, variation_id, 5u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let xx = p.x;
    let yy = p.y;
    let denom = xx * xx + yy * yy;
    let safe_denom = max(denom, 1e-30);
    let r2 = 1.0 / safe_denom;
    var r = pow(max(xx * xx + r2 * b * yy * yy, 1e-30), warp)
          - pow(max(yy * yy + r2 * a * xx * xx, 1e-30), warp);
    r = clamp(r, rmin, rmax);
    r = effect * r;
    return vec3<f32>(xx * (1.0 + r) * inv_w, yy * (1.0 + r) * inv_w, p.z);
}
"#),
};
