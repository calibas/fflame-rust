//! Apophysis miscellany 10 — four more
//!
//!   - `mask`    (Raykoid666 / CozyG)   — 5 user (Java-recovered;
//!                                          cpp APO_VARIABLES empty)
//!                                          xshift/yshift/ushift/
//!                                          xscale/yscale; output uses
//!                                          `sin² · sin · cosh` shape
//!   - `ovoid3d` (Larry Berlin)         — 3 user (x, y, z scales);
//!                                          Full3D `r = w/T` per-axis
//!                                          scaled output
//!   - `murl2`   (Zueuk)                    — 2 user (c, power int);
//!                                          body has internal `_vp =
//!                                          w · pow(c+1, 2/power)`
//!                                          factor; needs_transform
//!                                          divide-out
//!   - `minkQM`  (dark-beam / Brad Stefanov) — 6 user (a, b, c, dd, e, f);
//!                                          implements Minkowski's
//!                                          question-mark function via
//!                                          a per-axis loop bounded by
//!                                          `f` (default 20). Clean
//!                                          factor through outer
//!
//! Sources: each variation's `.cpp` file in
//! `output/jwildfire-vars/output/`.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// mask (Raykoid666 / CozyG)
//   sumsq = x² + y²
//   if sumsq != 0:
//     xfactor = xscale·x + xshift;  yfactor = yscale·y + yshift
//     out = (1/sumsq · sin(xfactor) · (cosh(yfactor)+ushift) · sin²(xfactor),
//            1/sumsq · cos(xfactor) · (cosh(yfactor)+ushift) · sin²(xfactor))
//   else: out = (0, 0)
// 5 user params (Java-recovered): xshift, yshift, ushift, xscale, yscale.
// (cpp uses `FPx = …` for x and `FPy +=` for y — porter inconsistency;
// both treated as additive in our system.)
// =============================================================================
/// Sin/cos masked warp — emits `(sin³(xf), cos(xf)·sin²(xf)) · (cosh(yf) +
/// ushift) / (x² + y²)` where `xf = xscale·x + xshift` and `yf = yscale·y +
/// yshift`. The sin-power factors create lobed masking; `cosh(yf)`
/// modulates the overall magnitude based on Y.
///
/// # Authors
/// - Raykoid666
/// - CozyG
pub static MASK: VariationDef = VariationDef {
    name: "mask",
    aliases: &[],
    display_name: "Mask",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        param!("xshift", "X shift", unlimited_float, 0.0, -10.0, 10.0, "Additive offset on X before the sin term."),
        param!("yshift", "Y shift", unlimited_float, 0.0, -10.0, 10.0, "Additive offset on Y before the cosh term."),
        param!("ushift", "U shift", unlimited_float, 1.0, -10.0, 10.0, "Constant offset added to `cosh(yf)`."),
        param!("xscale", "X scale", unlimited_float, 1.0, -10.0, 10.0, "Multiplier on X before the sin term."),
        param!("yscale", "Y scale", unlimited_float, 1.0, -10.0, 10.0, "Multiplier on Y before the cosh term."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_mask(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let xshift = get_param(xform_id, variation_id, 0u);
    let yshift = get_param(xform_id, variation_id, 1u);
    let ushift = get_param(xform_id, variation_id, 2u);
    let xscale = get_param(xform_id, variation_id, 3u);
    let yscale = get_param(xform_id, variation_id, 4u);

    let sumsq = p.x * p.x + p.y * p.y;
    if (sumsq < 1e-30) {
        return vec2<f32>(0.0, 0.0);
    }
    let xfactor = xscale * p.x + xshift;
    let yfactor = yscale * p.y + yshift;
    let sx = sin(xfactor);
    let body = (cosh(yfactor) + ushift) * sx * sx / sumsq;
    return vec2<f32>(sx * body, cos(xfactor) * body);
}
"#,
    wgsl_3d: r#"
fn variation_mask(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let xshift = get_param(xform_id, variation_id, 0u);
    let yshift = get_param(xform_id, variation_id, 1u);
    let ushift = get_param(xform_id, variation_id, 2u);
    let xscale = get_param(xform_id, variation_id, 3u);
    let yscale = get_param(xform_id, variation_id, 4u);

    let sumsq = p.x * p.x + p.y * p.y;
    if (sumsq < 1e-30) {
        return vec3<f32>(0.0, 0.0, p.z);
    }
    let xfactor = xscale * p.x + xshift;
    let yfactor = yscale * p.y + yshift;
    let sx = sin(xfactor);
    let body = (cosh(yfactor) + ushift) * sx * sx / sumsq;
    return vec3<f32>(sx * body, cos(xfactor) * body, p.z);
}
"#,
};

// =============================================================================
// ovoid3d (Larry Berlin)
//   T = x² + y² + z² + ε
//   r = w / T
//   out = (x · r · x_scale, y · r · y_scale, z · r · z_scale)
// 3 user params; Full3D; clean factor through outer.
// (In 2D `T = x² + y²`; z output ignored.)
// =============================================================================
/// 3D ovoid — emits `(x·x_scale, y·y_scale, z·z_scale) / T` where `T = x² +
/// y² + z² + ε`. Generalizes spherical inversion to per-axis scaling.
///
/// # Authors
/// - Larry Berlin
pub static OVOID3D: VariationDef = VariationDef {
    name: "ovoid3d",
    aliases: &[],
    display_name: "Ovoid 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        param!("x", "X scale", unlimited_float, 1.0, -10.0, 10.0, "X-axis scale on the inverted output."),
        param!("y", "Y scale", unlimited_float, 1.0, -10.0, 10.0, "Y-axis scale."),
        param!("z", "Z scale", unlimited_float, 1.0, -10.0, 10.0, "Z-axis scale (3D only)."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_ovoid3d(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let x_scale = get_param(xform_id, variation_id, 0u);
    let y_scale = get_param(xform_id, variation_id, 1u);
    let t = max(p.x * p.x + p.y * p.y, 1e-30);
    let r = 1.0 / t;
    return vec2<f32>(p.x * r * x_scale, p.y * r * y_scale);
}
"#,
    wgsl_3d: r#"
fn variation_ovoid3d(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let x_scale = get_param(xform_id, variation_id, 0u);
    let y_scale = get_param(xform_id, variation_id, 1u);
    let z_scale = get_param(xform_id, variation_id, 2u);
    let t = max(p.x * p.x + p.y * p.y + p.z * p.z, 1e-30);
    let r = 1.0 / t;
    return vec3<f32>(p.x * r * x_scale, p.y * r * y_scale, p.z * r * z_scale);
}
"#,
};

// =============================================================================
// murl2 (Zueuk)
//   2 user params: c, power (int)
//   p2 = power / 2;  invp = 1 / power (with special case for power == 0)
//   vp = w · pow(c + 1, 2 / power)   (special case for c = -1: vp = 0)
//   a = atan2(y, x) · power
//   r = c · pow(x² + y², p2)
//   re = r · cos(a) + 1;  im = r · sin(a)
//   r = pow(re² + im², invp);  a = atan2(im, re) · 2 · invp
//   re = r · cos(a);  im = r · sin(a)
//   rl = vp / r²
//   out = (rl · (x·re + y·im), rl · (y·re - x·im))
// Body has internal w in `vp` then in `rl` — needs_transform divide-out.
// =============================================================================
/// Variant of `murl` — applies a more involved complex Möbius mapping:
/// power-transforms the input, adds 1, then power-transforms the result
/// back, then divides by squared magnitude. The `2/power` exponent in the
/// internal `vp = w · (c+1)^(2/power)` factor adjusts the output magnitude
/// per power value.
///
/// # Authors
/// - Zueuk
pub static MURL2: VariationDef = VariationDef {
    name: "murl2",
    aliases: &[],
    display_name: "Murl 2",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsTransform],
    parameters: &[
        param!("c", "C", unlimited_float, 0.1, -10.0, 10.0, "Möbius coefficient — controls the strength of the Möbius distortion."),
        param!("power", "Power", int, 3.0, -100.0, 100.0, "Power exponent. 0 falls back to a degenerate case with a high invp."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_murl2(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let c = get_param(xform_id, variation_id, 0u);
    let power = get_param(xform_id, variation_id, 1u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let p2 = power / 2.0;
    var invp: f32;
    var vp: f32;
    if (abs(power) > 1e-30) {
        invp = 1.0 / power;
        if (abs(c + 1.0) < 1e-30) {
            vp = 0.0;
        } else {
            vp = w * pow(abs(c + 1.0), 2.0 / power) * select(1.0, sign(c + 1.0), abs(2.0 / power - floor(2.0 / power + 0.5)) < 1e-6 && fract(1.0 / power + 0.5) > 0.99);
        }
    } else {
        invp = 1.0e11;
        vp = w * pow(max(c + 1.0, 1e-30), 4.0);
    }

    var a = atan2(p.y, p.x) * power;
    var r = c * pow(max(p.x * p.x + p.y * p.y, 1e-30), p2);
    var re = r * cos(a) + 1.0;
    var im = r * sin(a);
    r = pow(max(re * re + im * im, 1e-30), invp);
    a = atan2(im, re) * 2.0 * invp;
    re = r * cos(a);
    im = r * sin(a);
    let safe_r2 = max(r * r, 1e-30);
    let rl = vp / safe_r2;
    let fx = rl * (p.x * re + p.y * im);
    let fy = rl * (p.y * re - p.x * im);
    return vec2<f32>(fx * inv_w, fy * inv_w);
}
"#,
    wgsl_3d: r#"
fn variation_murl2(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let c = get_param(xform_id, variation_id, 0u);
    let power = get_param(xform_id, variation_id, 1u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let p2 = power / 2.0;
    var invp: f32;
    var vp: f32;
    if (abs(power) > 1e-30) {
        invp = 1.0 / power;
        if (abs(c + 1.0) < 1e-30) {
            vp = 0.0;
        } else {
            vp = w * pow(abs(c + 1.0), 2.0 / power);
        }
    } else {
        invp = 1.0e11;
        vp = w * pow(max(c + 1.0, 1e-30), 4.0);
    }

    var a = atan2(p.y, p.x) * power;
    var r = c * pow(max(p.x * p.x + p.y * p.y, 1e-30), p2);
    var re = r * cos(a) + 1.0;
    var im = r * sin(a);
    r = pow(max(re * re + im * im, 1e-30), invp);
    a = atan2(im, re) * 2.0 * invp;
    re = r * cos(a);
    im = r * sin(a);
    let safe_r2 = max(r * r, 1e-30);
    let rl = vp / safe_r2;
    let fx = rl * (p.x * re + p.y * im);
    let fy = rl * (p.y * re - p.x * im);
    return vec3<f32>(fx * inv_w, fy * inv_w, p.z);
}
"#,
};

// =============================================================================
// minkQM (dark-beam / Brad Stefanov)
//   Helper minkowski(x): runs `f` iterations of the SB-tree question-mark
//   algorithm with parameters a, b, c, dd, e seeding the recursion
//   Body: per-axis sign-pass + minkowski for |x|, |y| in (0, 1)
//   out = (mnkX, mnkY)
// 6 user params: a, b, c, dd, e, f. Clean factor through outer.
// (`f` is the iteration count; default 20 = ~1e-6 precision.)
// =============================================================================
/// Minkowski's question-mark function — applies the Stern-Brocot tree
/// iteration of Minkowski's `?(x)` to each axis separately. The function
/// maps rationals with simple continued-fraction expansions to dyadic
/// rationals, producing a self-similar staircase pattern. Parameters `a, b,
/// c, dd, e` seed the SB tree recursion; `f` sets the iteration count.
///
/// # Authors
/// - DarkBeam
/// - Brad Stefanov
pub static MINKQM: VariationDef = VariationDef {
    name: "minkQM",
    aliases: &[],
    display_name: "Mink QM",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        param!("a", "A", unlimited_float, 1.0, -10.0, 10.0, "Initial denominator `q` of the SB-tree recursion."),
        param!("b", "B", unlimited_float, 1.0, -10.0, 10.0, "Initial offset on the SB-tree numerator `r`."),
        param!("c", "C", unlimited_float, 1.0, -10.0, 10.0, "Initial denominator `s` of the SB-tree recursion."),
        param!("dd", "D", unlimited_float, 1.0, -10.0, 10.0, "Initial step size on the output `y`."),
        param!("e", "E", unlimited_float, 0.5, -10.0, 10.0, "Step decay factor — multiplied with `d` each iteration. 0.5 gives the standard Minkowski function."),
        param!("f", "Iters", int, 20.0, 1.0, 50.0, "Iteration count. Default 20 yields about 1e-6 precision."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn minkqm_minkowski(xv: f32, a: f32, b: f32, c: f32, dd: f32, e: f32, iters: i32) -> f32 {
    var p: f32 = 0.0;
    var q = a;
    var r = p + b;
    var s = c;
    var d = dd;
    var y: f32 = p;
    for (var it: i32 = 0; it < iters; it = it + 1) {
        d = d * e;
        let m = p + r;
        let n = q + s;
        let safe_n = select(n, 1e-30, abs(n) < 1e-30);
        if (xv < (m / safe_n)) {
            r = m;
            s = n;
        } else {
            y = y + d;
            p = m;
            q = n;
        }
    }
    return y + d;
}

fn variation_minkQM(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    let dd = get_param(xform_id, variation_id, 3u);
    let e = get_param(xform_id, variation_id, 4u);
    let iters = i32(max(get_param(xform_id, variation_id, 5u), 1.0));

    var mnkX = p.x;
    var sx = 1.0;
    if (mnkX < 0.0) {
        mnkX = -mnkX;
        sx = -1.0;
    }
    if (mnkX > 0.0 && mnkX < 1.0) {
        mnkX = minkqm_minkowski(mnkX, a, b, c, dd, e, iters);
    }
    mnkX = sx * mnkX;

    var mnkY = p.y;
    var sy = 1.0;
    if (mnkY < 0.0) {
        mnkY = -mnkY;
        sy = -1.0;
    }
    if (mnkY > 0.0 && mnkY < 1.0) {
        mnkY = minkqm_minkowski(mnkY, a, b, c, dd, e, iters);
    }
    mnkY = sy * mnkY;

    return vec2<f32>(mnkX, mnkY);
}
"#,
    wgsl_3d: r#"
fn minkqm_minkowski(xv: f32, a: f32, b: f32, c: f32, dd: f32, e: f32, iters: i32) -> f32 {
    var p: f32 = 0.0;
    var q = a;
    var r = p + b;
    var s = c;
    var d = dd;
    var y: f32 = p;
    for (var it: i32 = 0; it < iters; it = it + 1) {
        d = d * e;
        let m = p + r;
        let n = q + s;
        let safe_n = select(n, 1e-30, abs(n) < 1e-30);
        if (xv < (m / safe_n)) {
            r = m;
            s = n;
        } else {
            y = y + d;
            p = m;
            q = n;
        }
    }
    return y + d;
}

fn variation_minkQM(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    let dd = get_param(xform_id, variation_id, 3u);
    let e = get_param(xform_id, variation_id, 4u);
    let iters = i32(max(get_param(xform_id, variation_id, 5u), 1.0));

    var mnkX = p.x;
    var sx = 1.0;
    if (mnkX < 0.0) {
        mnkX = -mnkX;
        sx = -1.0;
    }
    if (mnkX > 0.0 && mnkX < 1.0) {
        mnkX = minkqm_minkowski(mnkX, a, b, c, dd, e, iters);
    }
    mnkX = sx * mnkX;

    var mnkY = p.y;
    var sy = 1.0;
    if (mnkY < 0.0) {
        mnkY = -mnkY;
        sy = -1.0;
    }
    if (mnkY > 0.0 && mnkY < 1.0) {
        mnkY = minkqm_minkowski(mnkY, a, b, c, dd, e, iters);
    }
    mnkY = sy * mnkY;

    return vec3<f32>(mnkX, mnkY, p.z);
}
"#,
};
