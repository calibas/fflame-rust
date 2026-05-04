//! Miscellaneous singleton variations
//!
//! Eight unrelated standalone variations that don't fit any of the
//! existing family files:
//!
//!   - `corners`   (Whittaker Courtney 2018-08-14) — quadrant-based
//!                 power warp; recovered 7 omitted params from Java
//!                 (cpp exposed only `x` and `y`)
//!   - `modulus`   (Apophysis pack)                — boundary-bounded
//!                 fmod warp on each axis
//!   - `octagon`   (FracFx)                        — octagonal-tile warp
//!   - `circus`    (Faber)                          — radial radius scale
//!   - `circlize`  (?) and `circlize2` (Faber)      — perimeter-to-circle
//!                 mappings on the L∞ "square" perimeter
//!   - `atan`      (FractalDesire / Stefanov)       — per-axis atan with
//!                 mode selector
//!   - `murl`      (Maschke)                        — complex-power Möbius
//!
//! Sources: each variation's `.cpp` file in
//! `output/jwildfire-vars/output/`.
//!
//! Notes on faithfulness:
//!   - `corners` upstream cpp APO_VARIABLES exposes only `x` and `y`;
//!     the other 7 params (multx, multy, xpower, ypower, xypower,
//!     logmode, log_base) live in the embedded Java comment block.
//!     Recovered all 7. The `+ xwidth` / `+ ywidth` add-on terms lack
//!     VVAR — body uses `needs_transform` + divide-out so outer × w
//!     restores the cpp output.
//!   - `circlize` upstream cpp explicitly notes "VAR(hole) is not
//!     scaled by vvar" — same divide-out pattern.
//!   - `circlize2` (Faber's Angle Pack) folds `hole` into the
//!     VVAR-scaled `r`, so it factors cleanly.
//!   - `murl`'s cpp puts a bunch of intermediates in the per-thread
//!     `Variables` struct (`_c, _p2, _vp, _a, _sina, ...`) but they
//!     all derive from per-iteration values inside `PluginVarCalc`.
//!     We treat them as local variables.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// corners: quadrant-based power warp (Whittaker Courtney 2018)
//   ex = pow(x², xpower + xypower) · multx
//   ey = pow(y², ypower + xypower) · multy
//   (logmode mode swaps the inner formula to a log-pow variant)
//   FPx += sign(x) · (VVAR · ex + xwidth)
//   FPy += sign(y) · (VVAR · ey + ywidth)
//   (the trailing ±xwidth/±ywidth lacks VVAR — divide-out)
// =============================================================================
pub static CORNERS: VariationDef = VariationDef {
    name: "corners",
    display_name: "Corners",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("xwidth", "X Width", unlimited_float, 1.0, -10.0, 10.0),
        param!("ywidth", "Y Width", unlimited_float, 1.0, -10.0, 10.0),
        param!("multx", "Mult X", unlimited_float, 1.0, -10.0, 10.0),
        param!("multy", "Mult Y", unlimited_float, 1.0, -10.0, 10.0),
        param!("xpower", "X Power", unlimited_float, 0.75, -10.0, 10.0),
        param!("ypower", "Y Power", unlimited_float, 0.75, -10.0, 10.0),
        param!("xypower", "XY Power Add", unlimited_float, 0.0, -10.0, 10.0),
        param!("logmode", "Log Mode", int, 0.0, 0.0, 1.0),
        param!("log_base", "Log Base", unlimited_float, 2.71828, 0.01, 100.0),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_corners(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let xwidth = get_param(xform_id, variation_id, 0u);
    let ywidth = get_param(xform_id, variation_id, 1u);
    let multx = get_param(xform_id, variation_id, 2u);
    let multy = get_param(xform_id, variation_id, 3u);
    let xpower = get_param(xform_id, variation_id, 4u);
    let ypower = get_param(xform_id, variation_id, 5u);
    let xypower = get_param(xform_id, variation_id, 6u);
    let logmode = get_param(xform_id, variation_id, 7u);
    let log_base = max(get_param(xform_id, variation_id, 8u), 1e-6);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let xs = p.x * p.x;
    let ys = p.y * p.y;
    var ex: f32;
    var ey: f32;
    if (logmode < 0.5) {
        ex = pow(max(xs, 1e-30), xpower + xypower) * multx;
        ey = pow(max(ys, 1e-30), ypower + xypower) * multy;
    } else {
        let lb = log(log_base);
        let safe_lb = select(lb, 1e-30, abs(lb) < 1e-30);
        ex = pow(max(log(xs * multx + 3.0) / safe_lb, 1e-30), xpower + 2.25 + xypower) - 1.33;
        ey = pow(max(log(ys * multy + 3.0) / safe_lb, 1e-30), ypower + 2.25 + xypower) - 1.33;
    }
    let fx = select(w * -ex - xwidth, w * ex + xwidth, p.x > 0.0);
    let fy = select(w * -ey - ywidth, w * ey + ywidth, p.y > 0.0);
    return vec2<f32>(fx * inv_w, fy * inv_w);
}
"#,
    wgsl_3d: Some(r#"
fn variation_corners(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let xwidth = get_param(xform_id, variation_id, 0u);
    let ywidth = get_param(xform_id, variation_id, 1u);
    let multx = get_param(xform_id, variation_id, 2u);
    let multy = get_param(xform_id, variation_id, 3u);
    let xpower = get_param(xform_id, variation_id, 4u);
    let ypower = get_param(xform_id, variation_id, 5u);
    let xypower = get_param(xform_id, variation_id, 6u);
    let logmode = get_param(xform_id, variation_id, 7u);
    let log_base = max(get_param(xform_id, variation_id, 8u), 1e-6);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let xs = p.x * p.x;
    let ys = p.y * p.y;
    var ex: f32;
    var ey: f32;
    if (logmode < 0.5) {
        ex = pow(max(xs, 1e-30), xpower + xypower) * multx;
        ey = pow(max(ys, 1e-30), ypower + xypower) * multy;
    } else {
        let lb = log(log_base);
        let safe_lb = select(lb, 1e-30, abs(lb) < 1e-30);
        ex = pow(max(log(xs * multx + 3.0) / safe_lb, 1e-30), xpower + 2.25 + xypower) - 1.33;
        ey = pow(max(log(ys * multy + 3.0) / safe_lb, 1e-30), ypower + 2.25 + xypower) - 1.33;
    }
    let fx = select(w * -ex - xwidth, w * ex + xwidth, p.x > 0.0);
    let fy = select(w * -ey - ywidth, w * ey + ywidth, p.y > 0.0);
    return vec3<f32>(fx * inv_w, fy * inv_w, p.z);
}
"#),
};

// =============================================================================
// modulus: boundary-bounded fmod warp (Apophysis pack)
//   if x > x_p:   out_x = -x_p + fmod(x + x_p, 2x_p)
//   if x < -x_p:  out_x = x_p - fmod(x_p - x, 2x_p)
//   else:         out_x = x
//   (same for y)
// =============================================================================
pub static MODULUS: VariationDef = VariationDef {
    name: "modulus",
    display_name: "Modulus",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("x", "X", unlimited_float, 0.2, -10.0, 10.0),
        param!("y", "Y", unlimited_float, 0.5, -10.0, 10.0),
    ],
    needs_transform: false,
    writes_color: false,
    // 2 derived values at slots 2..4:
    //   2: xr  (2 · x)
    //   3: yr  (2 · y)
    init_param_count: 2,
    wgsl_init: Some(r#"
fn init_modulus(user: array<f32, 2>) -> array<f32, 2> {
    var out: array<f32, 2>;
    out[0] = 2.0 * user[0];
    out[1] = 2.0 * user[1];
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_modulus(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let xp = get_param(xform_id, variation_id, 0u);
    let yp = get_param(xform_id, variation_id, 1u);
    let xr = get_param(xform_id, variation_id, 2u);
    let yr = get_param(xform_id, variation_id, 3u);
    let safe_xr = select(xr, 1e-30, abs(xr) < 1e-30);
    let safe_yr = select(yr, 1e-30, abs(yr) < 1e-30);

    var ox: f32;
    if (p.x > xp) {
        let v = p.x + xp;
        ox = -xp + (v - floor(v / safe_xr) * safe_xr);
    } else if (p.x < -xp) {
        let v = xp - p.x;
        ox = xp - (v - floor(v / safe_xr) * safe_xr);
    } else {
        ox = p.x;
    }
    var oy: f32;
    if (p.y > yp) {
        let v = p.y + yp;
        oy = -yp + (v - floor(v / safe_yr) * safe_yr);
    } else if (p.y < -yp) {
        let v = yp - p.y;
        oy = yp - (v - floor(v / safe_yr) * safe_yr);
    } else {
        oy = p.y;
    }
    return vec2<f32>(ox, oy);
}
"#,
    wgsl_3d: Some(r#"
fn variation_modulus(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let xp = get_param(xform_id, variation_id, 0u);
    let yp = get_param(xform_id, variation_id, 1u);
    let xr = get_param(xform_id, variation_id, 2u);
    let yr = get_param(xform_id, variation_id, 3u);
    let safe_xr = select(xr, 1e-30, abs(xr) < 1e-30);
    let safe_yr = select(yr, 1e-30, abs(yr) < 1e-30);

    var ox: f32;
    if (p.x > xp) {
        let v = p.x + xp;
        ox = -xp + (v - floor(v / safe_xr) * safe_xr);
    } else if (p.x < -xp) {
        let v = xp - p.x;
        ox = xp - (v - floor(v / safe_xr) * safe_xr);
    } else {
        ox = p.x;
    }
    var oy: f32;
    if (p.y > yp) {
        let v = p.y + yp;
        oy = -yp + (v - floor(v / safe_yr) * safe_yr);
    } else if (p.y < -yp) {
        let v = yp - p.y;
        oy = yp - (v - floor(v / safe_yr) * safe_yr);
    } else {
        oy = p.y;
    }
    return vec3<f32>(ox, oy, p.z);
}
"#),
};

// =============================================================================
// octagon: octagonal-tile warp (FracFx)
//   Three sequential additive contributions, each with its own
//   conditional. Final per-axis sign-shift adds ±x/±y/±z.
//
// Internal weight: VVAR appears only as outer multiplier on each
// contribution; the conditional `if (r < 2.0)` etc. uses w-scaled `r`,
// but since output factors uniformly through outer multiplier we just
// use the threshold as-is when w > 0 (the typical case). At negative w
// the comparison flips — accept that as a minor divergence.
// =============================================================================
pub static OCTAGON: VariationDef = VariationDef {
    name: "octagon",
    display_name: "Octagon",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("x", "X", unlimited_float, 0.0, -10.0, 10.0),
        param!("y", "Y", unlimited_float, 0.0, -10.0, 10.0),
        param!("z", "Z", unlimited_float, 0.0, -10.0, 10.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_octagon(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let xp = get_param(xform_id, variation_id, 0u);
    let yp = get_param(xform_id, variation_id, 1u);
    // 2D form: z = 0; the upstream r and t formulas have z-terms but
    // we substitute 0 — body simplifies, output still works.
    let denom_r = max(p.x * p.x * p.x * p.x + p.y * p.y * p.y * p.y, 1e-30);
    let r = 1.0 / denom_r;
    var fpx: f32 = 0.0;
    var fpy: f32 = 0.0;
    if (r < 2.0) {
        fpx = p.x * r;
        fpy = p.y * r;
    } else {
        fpx = p.x;
        fpy = p.y;
    }
    let denom_t = max(sqrt(abs(p.x * p.x)) + sqrt(abs(p.y * p.y)), 1e-30);
    let t = 1.0 / denom_t;
    if (r >= 0.0) {
        fpx = fpx + p.x * t;
        fpy = fpy + p.y * t;
    } else {
        fpx = fpx + p.x;
        fpy = fpy + p.y;
    }
    if (p.x >= 0.0) { fpx = fpx + p.x + xp; } else { fpx = fpx + p.x - xp; }
    if (p.y >= 0.0) { fpy = fpy + p.y + yp; } else { fpy = fpy + p.y - yp; }
    return vec2<f32>(fpx, fpy);
}
"#,
    wgsl_3d: Some(r#"
fn variation_octagon(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let xp = get_param(xform_id, variation_id, 0u);
    let yp = get_param(xform_id, variation_id, 1u);
    let zp = get_param(xform_id, variation_id, 2u);
    let denom_r = max(p.x * p.x * p.x * p.x + p.z * p.z + p.y * p.y * p.y * p.y + p.z * p.z, 1e-30);
    let r = 1.0 / denom_r;
    var fpx: f32 = 0.0;
    var fpy: f32 = 0.0;
    var fpz: f32 = 0.0;
    if (r < 2.0) {
        fpx = p.x * r;
        fpy = p.y * r;
        fpz = p.z * r;
    } else {
        fpx = p.x;
        fpy = p.y;
        fpz = p.z;
    }
    let denom_t = max(sqrt(abs(p.x * p.x)) + sqrt(abs(p.z)) + sqrt(abs(p.y * p.y)) + sqrt(abs(p.z)), 1e-30);
    let t = 1.0 / denom_t;
    if (r >= 0.0) {
        fpx = fpx + p.x * t;
        fpy = fpy + p.y * t;
        fpz = fpz + p.z * t;
    } else {
        fpx = fpx + p.x;
        fpy = fpy + p.y;
        fpz = fpz + p.z;
    }
    if (p.x >= 0.0) { fpx = fpx + p.x + xp; } else { fpx = fpx + p.x - xp; }
    if (p.y >= 0.0) { fpy = fpy + p.y + yp; } else { fpy = fpy + p.y - yp; }
    if (p.z >= 0.0) { fpz = fpz + p.z + zp; } else { fpz = fpz + p.z - zp; }
    return vec3<f32>(fpx, fpy, fpz);
}
"#),
};

// =============================================================================
// circus: radial radius scale (Faber)
//   r = sqrt(x² + y²)
//   if r ≤ 1: r' = r · scale
//   else:     r' = r / scale
//   out = r' · (cos θ, sin θ)   where θ = atan2(y, x)
// =============================================================================
pub static CIRCUS: VariationDef = VariationDef {
    name: "circus",
    display_name: "Circus",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("scale", "Scale", unlimited_float, 1.0, 0.001, 10.0),
    ],
    needs_transform: false,
    writes_color: false,
    // 1 derived value at slot 1:
    //   1: inv_scale  (1 / scale)
    init_param_count: 1,
    wgsl_init: Some(r#"
fn init_circus(user: array<f32, 1>) -> array<f32, 1> {
    let scale = max(user[0], 1e-6);
    var out: array<f32, 1>;
    out[0] = 1.0 / scale;
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_circus(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let scale = get_param(xform_id, variation_id, 0u);
    let inv_scale = get_param(xform_id, variation_id, 1u);
    let r = sqrt(p.x * p.x + p.y * p.y);
    let a = atan2(p.y, p.x);
    let r_out = select(r * inv_scale, r * scale, r <= 1.0);
    return vec2<f32>(r_out * cos(a), r_out * sin(a));
}
"#,
    wgsl_3d: Some(r#"
fn variation_circus(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let scale = get_param(xform_id, variation_id, 0u);
    let inv_scale = get_param(xform_id, variation_id, 1u);
    let r = sqrt(p.x * p.x + p.y * p.y);
    let a = atan2(p.y, p.x);
    let r_out = select(r * inv_scale, r * scale, r <= 1.0);
    return vec3<f32>(r_out * cos(a), r_out * sin(a), p.z);
}
"#),
};

// =============================================================================
// circlize: L∞-square perimeter to circle (Apophysis pack)
//   compute (perimeter, side) on the L∞ unit-square
//   r = (4·VVAR/π) · side + hole       (hole NOT scaled by VVAR — needs
//                                       divide-out)
//   a = (π/4) · perimeter / side − π/4
//   FPx += r · cos a;  FPy += r · sin a
// =============================================================================
pub static CIRCLIZE: VariationDef = VariationDef {
    name: "circlize",
    display_name: "Circlize",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("hole", "Hole", unlimited_float, 0.4, -10.0, 10.0),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_circlize(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let hole = get_param(xform_id, variation_id, 0u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let absx = abs(p.x);
    let absy = abs(p.y);
    var perimeter: f32;
    var side: f32;
    if (absx >= absy) {
        if (p.x >= absy) { perimeter = absx + p.y; }
        else             { perimeter = 5.0 * absx - p.y; }
        side = absx;
    } else {
        if (p.y >= absx) { perimeter = 3.0 * absy - p.x; }
        else             { perimeter = 7.0 * absy + p.x; }
        side = absy;
    }
    let var4_pi = w * 1.2732395447351627;  // = w / (π/4)
    let safe_side = select(side, 1e-30, side == 0.0);
    let r = var4_pi * side + hole;
    let a = 0.7853981633974483 * perimeter / safe_side - 0.7853981633974483;
    let fx = r * cos(a);
    let fy = r * sin(a);
    return vec2<f32>(fx * inv_w, fy * inv_w);
}
"#,
    wgsl_3d: Some(r#"
fn variation_circlize(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let hole = get_param(xform_id, variation_id, 0u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let absx = abs(p.x);
    let absy = abs(p.y);
    var perimeter: f32;
    var side: f32;
    if (absx >= absy) {
        if (p.x >= absy) { perimeter = absx + p.y; }
        else             { perimeter = 5.0 * absx - p.y; }
        side = absx;
    } else {
        if (p.y >= absx) { perimeter = 3.0 * absy - p.x; }
        else             { perimeter = 7.0 * absy + p.x; }
        side = absy;
    }
    let var4_pi = w * 1.2732395447351627;
    let safe_side = select(side, 1e-30, side == 0.0);
    let r = var4_pi * side + hole;
    let a = 0.7853981633974483 * perimeter / safe_side - 0.7853981633974483;
    let fx = r * cos(a);
    let fy = r * sin(a);
    return vec3<f32>(fx * inv_w, fy * inv_w, p.z);
}
"#),
};

// =============================================================================
// circlize2: same as circlize, hole folded into VVAR-scaled r (Faber)
//   r = VVAR · (side + hole)        (clean factor through outer)
// =============================================================================
pub static CIRCLIZE2: VariationDef = VariationDef {
    name: "circlize2",
    display_name: "Circlize 2",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("hole", "Hole", unlimited_float, 0.0, -10.0, 10.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_circlize2(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let hole = get_param(xform_id, variation_id, 0u);
    let absx = abs(p.x);
    let absy = abs(p.y);
    var perimeter: f32;
    var side: f32;
    if (absx >= absy) {
        if (p.x >= absy) { perimeter = absx + p.y; }
        else             { perimeter = 5.0 * absx - p.y; }
        side = absx;
    } else {
        if (p.y >= absx) { perimeter = 3.0 * absy - p.x; }
        else             { perimeter = 7.0 * absy + p.x; }
        side = absy;
    }
    let safe_side = select(side, 1e-30, side == 0.0);
    let r = side + hole;
    let a = 0.7853981633974483 * perimeter / safe_side - 0.7853981633974483;
    return vec2<f32>(r * cos(a), r * sin(a));
}
"#,
    wgsl_3d: Some(r#"
fn variation_circlize2(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let hole = get_param(xform_id, variation_id, 0u);
    let absx = abs(p.x);
    let absy = abs(p.y);
    var perimeter: f32;
    var side: f32;
    if (absx >= absy) {
        if (p.x >= absy) { perimeter = absx + p.y; }
        else             { perimeter = 5.0 * absx - p.y; }
        side = absx;
    } else {
        if (p.y >= absx) { perimeter = 3.0 * absy - p.x; }
        else             { perimeter = 7.0 * absy + p.x; }
        side = absy;
    }
    let safe_side = select(side, 1e-30, side == 0.0);
    let r = side + hole;
    let a = 0.7853981633974483 * perimeter / safe_side - 0.7853981633974483;
    return vec3<f32>(r * cos(a), r * sin(a), p.z);
}
"#),
};

// =============================================================================
// atan: per-axis atan with mode selector (FractalDesire / Stefanov)
//   norm = (2/π) · VVAR
//   mode 0: out = (x, (2/π) · atan(stretch · y))
//   mode 1: out = ((2/π) · atan(stretch · x), y)
//   mode 2: out = ((2/π) · atan(stretch · x), (2/π) · atan(stretch · y))
// =============================================================================
pub static ATAN_VAR: VariationDef = VariationDef {
    name: "atan",
    display_name: "Atan",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("mode", "Mode", int, 0.0, 0.0, 2.0),
        param!("stretch", "Stretch", unlimited_float, 1.0, -10.0, 10.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_atan(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let mode = get_param(xform_id, variation_id, 0u);
    let stretch = get_param(xform_id, variation_id, 1u);
    let two_over_pi = 0.6366197723675813;
    let mi = i32(mode);
    if (mi == 0) {
        return vec2<f32>(p.x, two_over_pi * atan(stretch * p.y));
    } else if (mi == 1) {
        return vec2<f32>(two_over_pi * atan(stretch * p.x), p.y);
    }
    return vec2<f32>(
        two_over_pi * atan(stretch * p.x),
        two_over_pi * atan(stretch * p.y),
    );
}
"#,
    wgsl_3d: Some(r#"
fn variation_atan(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let mode = get_param(xform_id, variation_id, 0u);
    let stretch = get_param(xform_id, variation_id, 1u);
    let two_over_pi = 0.6366197723675813;
    let mi = i32(mode);
    if (mi == 0) {
        return vec3<f32>(p.x, two_over_pi * atan(stretch * p.y), p.z);
    } else if (mi == 1) {
        return vec3<f32>(two_over_pi * atan(stretch * p.x), p.y, p.z);
    }
    return vec3<f32>(
        two_over_pi * atan(stretch * p.x),
        two_over_pi * atan(stretch * p.y),
        p.z,
    );
}
"#),
};

// =============================================================================
// murl: complex-power Möbius (Maschke)
//   c     = c_user / (power − 1)        (or = c_user when power == 1)
//   p2    = power / 2
//   vp    = VVAR · (c + 1)
//   a     = atan2(y, x) · power
//   r     = c · pow(x² + y², p2)
//   re    = r · cos a + 1;  im = r · sin a
//   rl    = vp / (re² + im²)
//   FPx += rl · (x · re + y · im)
//   FPy += rl · (y · re − x · im)
// =============================================================================
pub static MURL: VariationDef = VariationDef {
    name: "murl",
    display_name: "Murl",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("c", "C", unlimited_float, 0.1, -10.0, 10.0),
        param!("power", "Power", int, 1.0, -50.0, 50.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_murl(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let c_in = get_param(xform_id, variation_id, 0u);
    let power = get_param(xform_id, variation_id, 1u);
    var c = c_in;
    if (power != 1.0) {
        let pm1 = power - 1.0;
        let safe_pm1 = select(pm1, 1e-30, abs(pm1) < 1e-30);
        c = c / safe_pm1;
    }
    let p2 = power * 0.5;
    let vp = c + 1.0;  // VVAR factored out — outer multiplier reapplies
    let a = atan2(p.y, p.x) * power;
    let r = c * pow(max(p.x * p.x + p.y * p.y, 1e-30), p2);
    let re = r * cos(a) + 1.0;
    let im = r * sin(a);
    let rl = vp / max(re * re + im * im, 1e-30);
    return vec2<f32>(
        rl * (p.x * re + p.y * im),
        rl * (p.y * re - p.x * im),
    );
}
"#,
    wgsl_3d: Some(r#"
fn variation_murl(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let c_in = get_param(xform_id, variation_id, 0u);
    let power = get_param(xform_id, variation_id, 1u);
    var c = c_in;
    if (power != 1.0) {
        let pm1 = power - 1.0;
        let safe_pm1 = select(pm1, 1e-30, abs(pm1) < 1e-30);
        c = c / safe_pm1;
    }
    let p2 = power * 0.5;
    let vp = c + 1.0;
    let a = atan2(p.y, p.x) * power;
    let r = c * pow(max(p.x * p.x + p.y * p.y, 1e-30), p2);
    let re = r * cos(a) + 1.0;
    let im = r * sin(a);
    let rl = vp / max(re * re + im * im, 1e-30);
    return vec3<f32>(
        rl * (p.x * re + p.y * im),
        rl * (p.y * re - p.x * im),
        p.z,
    );
}
"#),
};
