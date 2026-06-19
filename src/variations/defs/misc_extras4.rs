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
    definition::{Feature, VariationDef, VariationParamDef},
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
/// Anamorphic cylinder warp — wraps the input around a cylinder. Outputs
/// `(f·cos(k·x), f·sin(k·x))` where `f = a·(y + b)`. The X coordinate
/// drives the angular position (× frequency `k`), and Y drives the radial
/// position (offset by `b`, scaled by `a`).
///
/// # Authors
/// - Jesus Sosa
pub static ANAMORPHCYL: VariationDef = VariationDef {
    name: "anamorphcyl",
    aliases: &[],
    display_name: "Anamorph Cyl",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[Feature::Replace],
    parameters: &[
        param!("a", "A", unlimited_float, 1.0, -10.0, 10.0, "Radial scale (multiplies the entire output magnitude)."),
        param!("b", "B", unlimited_float, 1.3, -10.0, 10.0, "Y offset added before the radial multiplication."),
        param!("k", "K", unlimited_float, 3.0, -50.0, 50.0, "Angular frequency of the X coordinate."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_anamorphcyl(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let k = get_param(xform_id, variation_id, 2u);
    let f = a * (p.y + b);
    return vec2<f32>(f * cos(k * p.x), f * sin(k * p.x));
}
"#,
    wgsl_3d: r#"
fn variation_anamorphcyl(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let k = get_param(xform_id, variation_id, 2u);
    let f = a * (p.y + b);
    return vec3<f32>(f * cos(k * p.x), f * sin(k * p.x), p.z);
}
"#,
};

// =============================================================================
// svf: 3D single-value-function (gossamer light)
//   cn = cos(n · y)
//   out = (cy · cn · cx, cy · cn · sx, sy · cn)
//   where cx = cos(x), sx = sin(x), cy = cos(y), sy = sin(y)
// =============================================================================
/// 3D single-value function — combines trigonometric terms in a fixed
/// pattern: `cos(y) · cos(n·y) · (cos(x), sin(x), sin(y))`. The `n`
/// frequency parameter controls how many oscillations appear along the Y
/// axis.
///
/// # Authors
/// - gossamer light
pub static SVF: VariationDef = VariationDef {
    name: "svf",
    aliases: &[],
    display_name: "SVF",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Any,
    features: &[Feature::AlwaysZ],
    parameters: &[
        param!("n", "N", unlimited_float, 2.0, -50.0, 50.0, "Frequency multiplier on Y in the inner `cos(n·y)` term."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_svf(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let n = get_param(xform_id, variation_id, 0u);
    let cn = cos(n * p.y);
    let sx = sin(p.x); let cx = cos(p.x);
    let cy = cos(p.y);
    return vec2<f32>(cy * cn * cx, cy * cn * sx);
}
"#,
    wgsl_3d: r#"
fn variation_svf(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let n = get_param(xform_id, variation_id, 0u);
    let cn = cos(n * p.y);
    let sx = sin(p.x); let cx = cos(p.x);
    let sy = sin(p.y); let cy = cos(p.y);
    return vec3<f32>(cy * cn * cx, cy * cn * sx, sy * cn);
}
"#,
};

// =============================================================================
// shredlin: linear shred grid (Zy0rg)
//   out_x = w · xdistance · (frac(x/xdistance)·xwidth + floor(x/xdistance)
//                            + (0.5 − xpos)·(1 − xwidth))
//   (same form for y; cpp uses `FPx = vv · ...` assignment but VVAR
//   strictly outer)
// =============================================================================
/// Linear shred grid — splits each axis into tiles of size `distance`, then
/// within each tile compresses the position by `width` and shifts by `(0.5
/// − sign_offset) · (1 − width)`. Produces a discontinuous shred-like grid
/// pattern.
///
/// # Authors
/// - Zy0rg
pub static SHREDLIN: VariationDef = VariationDef {
    name: "shredlin",
    aliases: &[],
    display_name: "Shred Lin",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[Feature::Replace],
    parameters: &[
        param!("xdistance", "X distance", unlimited_float, 1.0, -10.0, 10.0, "X-axis tile size."),
        param!("xwidth", "X width", unlimited_float, 0.5, -10.0, 10.0, "X-axis intra-tile compression. 1 = no shred; 0 = collapse to tile center."),
        param!("ydistance", "Y distance", unlimited_float, 1.0, -10.0, 10.0, "Y-axis tile size."),
        param!("ywidth", "Y width", unlimited_float, 0.5, -10.0, 10.0, "Y-axis intra-tile compression."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
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
    wgsl_3d: r#"
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
"#,
};

// =============================================================================
// shredrad: radial shred (Zy0rg)
//   alpha = 2π / n        (recovered from Java; cpp porter bug)
//   ang   = atan2(x, y)   (cpp's swapped form, preserved)
//   xang  = (ang + 3π + α/2) / α
//   zang  = (frac(xang) · w + floor(xang)) · α − π − α/2 · w
//   out   = r · (cos zang, sin zang)
// =============================================================================
/// Radial shred — splits the angular space into `n` wedges of width `2π/n`,
/// then within each wedge compresses the angular position by `width`. The
/// radius passes through unchanged. Radial analogue of `shredlin`.
///
/// # Authors
/// - Zy0rg
pub static SHREDRAD: VariationDef = VariationDef {
    name: "shredrad",
    aliases: &[],
    display_name: "Shred Rad",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[],
    parameters: &[
        param!("n", "N", unlimited_float, 4.0, 0.001, 100.0, "Number of angular wedges."),
        param!("width", "Width", float, 0.5, -1.0, 1.0, "Intra-wedge compression. 1 = no shred; 0 = collapse to wedge boundary."),
    ],
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
    state_count: 0,
    wgsl_state_init: None,
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
    wgsl_3d: r#"
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
"#,
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
/// Heart-shaped Möbius warp — applies a Möbius-like inversion `(bx·x,
/// by·y)` where `bx = 4/(r²+4)` and `by = (6+2·ratio)/(r²+4)`, then rotates
/// the result by `π/4 + π/8·angle`. The Y axis is flipped where the rotated
/// X is negative, producing a heart-shaped silhouette.
///
/// # Authors
/// - Xyrus02
pub static XHEART: VariationDef = VariationDef {
    name: "xheart",
    aliases: &[],
    display_name: "X Heart",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[],
    parameters: &[
        param!("angle", "Angle", unlimited_float, 0.0, -10.0, 10.0, "Rotation angle of the heart shape (scaled by π/8 internally and offset from π/4)."),
        param!("ratio", "Ratio", unlimited_float, 0.0, -10.0, 10.0, "Y-axis stretch factor (added to a base of 6 to control heart roundness)."),
    ],
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
    state_count: 0,
    wgsl_state_init: None,
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
    wgsl_3d: r#"
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
"#,
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
/// Sin-weighted twin warp — adds a sin-modulated correction
/// `(x²−y²)·sin(2π·distort·(x+y+offset_xy·0.1)) / (x²+y²)` to both the X
/// and Y outputs (the same correction term is applied identically to each
/// axis).
///
/// # Authors
/// - Xyrus02
pub static STWIN: VariationDef = VariationDef {
    name: "stwin",
    aliases: &[],
    display_name: "STwin",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsTransform],
    parameters: &[
        param!("distort", "Distort", unlimited_float, 1.0, -10.0, 10.0, "Frequency multiplier on the sin term (× 2π internally)."),
        param!("offset_xy", "Offset XY", unlimited_float, 0.0, -10.0, 10.0, "Phase offset added to the sin argument (× 0.1 internally)."),
        param!("offset_x2", "Offset X²", unlimited_float, 0.0, -10.0, 10.0, "Additive offset on x² (× 0.0001 internally). Prevents division by zero at the origin."),
        param!("offset_y2", "Offset Y²", unlimited_float, 0.0, -10.0, 10.0, "Additive offset on y² (× 0.0001 internally)."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
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
    wgsl_3d: r#"
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
"#,
};

// =============================================================================
// whorl: radial whorl with inside/outside knobs (Apophysis pack)
//   r = sqrt(x²+y²) + ε
//   if r < w:   a = atan2(x, y) + inside  / (w − r)
//   else:        a = atan2(x, y) + outside / (w − r)
//   out = r · (cos a, sin a)            (VVAR strictly outer)
// =============================================================================
/// Radial whorl with inside/outside knobs — adds a `1/(w − r)` angular
/// term to the polar angle, with separate `inside` and `outside`
/// multipliers depending on whether the input lies inside (`r < w`) or
/// outside (`r ≥ w`) the unit-weight radius. Produces a whorl/swirl
/// pattern that diverges as the radius approaches the variation weight.
///
/// # Authors
/// - Apophysis Plugin Pack
pub static WHORL: VariationDef = VariationDef {
    name: "whorl",
    aliases: &[],
    display_name: "Whorl",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsTransform],
    parameters: &[
        param!("inside", "Inside", unlimited_float, 0.1, -10.0, 10.0, "Angular-shift coefficient applied where `r < w`."),
        param!("outside", "Outside", unlimited_float, 0.2, -10.0, 10.0, "Angular-shift coefficient applied where `r ≥ w`."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
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
    wgsl_3d: r#"
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
"#,
};

// =============================================================================
// devil_warp: power-warp with rmin/rmax clamp (dark-beam)
//   r = (x²+y²+r2·b·y²)^warp − (y²+r2·a·x²)^warp     (r2 = 1/(x²+y²))
//   r = clamp(r, rmin, rmax) · effect
//   FPx += x · (1 + r);  FPy += y · (1 + r)         (no VVAR — divide-out)
// =============================================================================
/// Power-warp with rmin/rmax clamp — computes a complex radial term `r =
/// (x² + r²·b·y²)^warp − (y² + r²·a·x²)^warp` (with `r² = 1/(x²+y²)`),
/// clamps it to `[rmin, rmax]`, scales it by `effect`, and emits `(x·(1+r),
/// y·(1+r))`. The clamp prevents the power expression from blowing up at
/// large or singular inputs.
///
/// # Authors
/// - DarkBeam
pub static DEVIL_WARP: VariationDef = VariationDef {
    name: "devil_warp",
    aliases: &[],
    display_name: "Devil Warp",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsTransform],
    parameters: &[
        param!("a", "A", unlimited_float, 2.0, -10.0, 10.0, "x² weight in the second power term."),
        param!("b", "B", unlimited_float, 1.0, -10.0, 10.0, "y² weight in the first power term."),
        param!("effect", "Effect", unlimited_float, 1.0, -10.0, 10.0, "Scale on the final radial-warp magnitude."),
        param!("warp", "Warp", unlimited_float, 0.5, -10.0, 10.0, "Power exponent for both radial terms."),
        param!("rmin", "R min", unlimited_float, -0.24, -100.0, 100.0, "Lower clamp on the radial-warp magnitude."),
        param!("rmax", "R max", unlimited_float, 100.0, -1000.0, 1000.0, "Upper clamp on the radial-warp magnitude."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
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
    wgsl_3d: r#"
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
"#,
};
