//! Simple Apophysis classics — eight more
//!
//!   - `exp2`         (Apo classic)              — `exp(x·π)·(cos y·π,
//!                                                  sin y·π)`
//!   - `exponential`  (Apo classic)              — `exp(x-1)·(cos π·y,
//!                                                  sin π·y)`
//!   - `flipy`        (Faber)                    — `if x>0: y = -y`
//!   - `funnel`       (Raykoid666)               — `tanh·(sec + N·π)`,
//!                                                  1 user param (effect int)
//!   - `invpolar`     (Apo classic)              — `(1+y)·(sin x·π,
//!                                                  cos x·π)`
//!   - `perspective`  (Apo classic)              — 2 user params (angle,
//!                                                  dist), 2 init slots
//!   - `line`         (chronologicaldot)            — 2 user (Java-recovered;
//!                                                  cpp APO_VARIABLES
//!                                                  empty, defaults
//!                                                  embedded as constants;
//!                                                  recovered from Java),
//!                                                  RNG, Full3D,
//!                                                  needs_transform
//!                                                  divide-out
//!   - `holesq`       (dark-beam)                — square hole; needs_transform
//!                                                  divide-out (additive
//!                                                  ±1 offsets cost
//!                                                  one `1/w` each)
//!
//! Sources: each variation's `.cpp` file in
//! `output/jwildfire-vars/output/`.
//!
//! Faithfulness:
//!   - `line` cpp APO_VARIABLES list is empty (porter omitted both user
//!     params); the `_delta` and `_phi` struct fields are initialized
//!     to 0 in `PluginVarPrepare`. Recovered the user params from
//!     Java's `setParameter`. (When `delta = phi = 0`, `(ux, uy, uz) =
//!     (1, 0, 0)`, so the line projects onto X.) Also drops the
//!     unit-vector normalization since `ux²+uy²+uz² ≡ 1` always.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// exp2: w · exp(x·π) · (cos y·π, sin y·π)
// Clean factor through outer.
// =============================================================================
/// Exponential polar warp — emits `exp(x·π) · (cos(y·π), sin(y·π))`. The X
/// coordinate drives an exponential radial scaling, and Y drives the
/// angular position.
pub static EXP2: VariationDef = VariationDef {
    name: "exp2",
    aliases: &[],
    display_name: "Exp 2",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_exp2(p: vec2<f32>) -> vec2<f32> {
    let pi = 3.14159265358979;
    let a = exp(p.x * pi);
    return vec2<f32>(a * cos(p.y * pi), a * sin(p.y * pi));
}
"#,
    wgsl_3d: r#"
fn variation_exp2(p: vec3<f32>) -> vec3<f32> {
    let pi = 3.14159265358979;
    let a = exp(p.x * pi);
    return vec3<f32>(a * cos(p.y * pi), a * sin(p.y * pi), p.z);
}
"#,
};

// =============================================================================
// exponential: w · exp(x-1) · (cos π·y, sin π·y)
// Clean factor through outer.
// =============================================================================
/// Classic exponential variation — emits `exp(x − 1) · (cos(π·y),
/// sin(π·y))`. Same shape as `exp2` but with a slightly different
/// exponential argument (subtracts 1 so the unit radius lies at x = 1
/// instead of x = 0).
///
/// # Authors
/// - Scott Draves
pub static EXPONENTIAL: VariationDef = VariationDef {
    name: "exponential",
    aliases: &[],
    display_name: "Exponential",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_exponential(p: vec2<f32>) -> vec2<f32> {
    let pi = 3.14159265358979;
    let r = pi * p.y;
    let d = exp(p.x - 1.0);
    return vec2<f32>(cos(r) * d, sin(r) * d);
}
"#,
    wgsl_3d: r#"
fn variation_exponential(p: vec3<f32>) -> vec3<f32> {
    let pi = 3.14159265358979;
    let r = pi * p.y;
    let d = exp(p.x - 1.0);
    return vec3<f32>(cos(r) * d, sin(r) * d, p.z);
}
"#,
};

// =============================================================================
// flipy: y-flip when x > 0 (Faber)
//   x > 0:  out = (x, -y)
//   else:   out = (x,  y)
// Clean.
// =============================================================================
/// Y-flip when x > 0 — leaves the input alone when `x ≤ 0`; flips the sign
/// of Y when `x > 0`. Useful as a building block for left/right asymmetric
/// flames.
///
/// # Authors
/// - Michael Faber
pub static FLIPY: VariationDef = VariationDef {
    name: "flipy",
    aliases: &[],
    display_name: "Flip Y",
    category: VariationCategory::Basic2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_flipy(p: vec2<f32>) -> vec2<f32> {
    let sy = select(1.0, -1.0, p.x > 0.0);
    return vec2<f32>(p.x, sy * p.y);
}
"#,
    wgsl_3d: r#"
fn variation_flipy(p: vec3<f32>) -> vec3<f32> {
    let sy = select(1.0, -1.0, p.x > 0.0);
    return vec3<f32>(p.x, sy * p.y, p.z);
}
"#,
};

// =============================================================================
// funnel: tanh · (sec + effect·π) (Raykoid666)
//   out = (tanh x · (1/cos x + effect·π),
//          tanh y · (1/cos y + effect·π))
// Clean factor through outer; 1 user param (effect int, default 8).
// =============================================================================
/// Tangent · secant + offset funnel — per axis, emits `tanh(coord) ·
/// (sec(coord) + effect · π)`. The hyperbolic tangent saturates the input
/// toward ±1, and the secant term creates poles where the input's cosine
/// hits zero.
///
/// # Authors
/// - Raykoid666
pub static FUNNEL: VariationDef = VariationDef {
    name: "funnel",
    aliases: &[],
    display_name: "Funnel",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        param!("effect", "Effect", int, 8.0, -100.0, 100.0, "Additive angular offset on each axis (scaled by π). Higher = more horizontal/vertical bias."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_funnel(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let effect = get_param(xform_id, variation_id, 0u);
    let pi = 3.14159265358979;
    let cx = cos(p.x);
    let cy = cos(p.y);
    let safe_cx = select(cx, 1e-30, abs(cx) < 1e-30);
    let safe_cy = select(cy, 1e-30, abs(cy) < 1e-30);
    let fx = tanh(p.x) * (1.0 / safe_cx + effect * pi);
    let fy = tanh(p.y) * (1.0 / safe_cy + effect * pi);
    return vec2<f32>(fx, fy);
}
"#,
    wgsl_3d: r#"
fn variation_funnel(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let effect = get_param(xform_id, variation_id, 0u);
    let pi = 3.14159265358979;
    let cx = cos(p.x);
    let cy = cos(p.y);
    let safe_cx = select(cx, 1e-30, abs(cx) < 1e-30);
    let safe_cy = select(cy, 1e-30, abs(cy) < 1e-30);
    let fx = tanh(p.x) * (1.0 / safe_cx + effect * pi);
    let fy = tanh(p.y) * (1.0 / safe_cy + effect * pi);
    return vec3<f32>(fx, fy, p.z);
}
"#,
};

// =============================================================================
// invpolar: inverse polar
//   ny = 1 + y
//   out = ny · (sin(π·x), cos(π·x))
// Clean factor through outer.
// =============================================================================
/// Inverse polar — emits `(1 + y) · (sin(π·x), cos(π·x))`. Treats X as the
/// angle (in units of π) and `1 + y` as the radius — the inverse of the
/// `polar` variation.
pub static INVPOLAR: VariationDef = VariationDef {
    name: "invpolar",
    aliases: &[],
    display_name: "Inv Polar",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_invpolar(p: vec2<f32>) -> vec2<f32> {
    let pi = 3.14159265358979;
    let ny = 1.0 + p.y;
    return vec2<f32>(ny * sin(p.x * pi), ny * cos(p.x * pi));
}
"#,
    wgsl_3d: r#"
fn variation_invpolar(p: vec3<f32>) -> vec3<f32> {
    let pi = 3.14159265358979;
    let ny = 1.0 + p.y;
    return vec3<f32>(ny * sin(p.x * pi), ny * cos(p.x * pi), p.z);
}
"#,
};

// =============================================================================
// perspective: 2D perspective projection (Apo classic)
//   2 user params: angle, dist
//   Init: vsin = sin(angle·π/2);  vfcos = dist · cos(angle·π/2)
//   t = 1 / (dist - y · vsin)
//   out = (dist · x · t, vfcos · y · t)
// Clean factor through outer.
// =============================================================================
/// 2D perspective projection — applies a perspective foreshortening with `t
/// = 1 / (dist − y · sin(angle·π/2))`, then emits `(dist · x · t, dist ·
/// cos(angle·π/2) · y · t)`. The `angle` parameter tilts the projection
/// plane; `dist` sets the viewing distance.
///
/// # Authors
/// - Scott Draves
pub static PERSPECTIVE: VariationDef = VariationDef {
    name: "perspective",
    aliases: &[],
    display_name: "Perspective",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        param!("angle", "Angle", unlimited_float, 0.62, -10.0, 10.0, "Tilt of the projection plane (in units of π/2). 0 = parallel (no perspective); 1 = perpendicular."),
        param!("dist", "Dist", unlimited_float, 2.2, -10.0, 10.0, "Viewing distance from the projection plane. Larger = milder perspective effect."),
    ],
    // 2 derived values at slots 2..4:
    //   2: vsin  = sin(angle · π/2)
    //   3: vfcos = dist · cos(angle · π/2)
    init_param_count: 2,
    wgsl_init: Some(r#"
fn init_perspective(user: array<f32, 2>) -> array<f32, 2> {
    let pi_2 = 1.5707963267948966;
    let ang = user[0] * pi_2;
    var out: array<f32, 2>;
    out[0] = sin(ang);
    out[1] = user[1] * cos(ang);
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_perspective(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let dist = get_param(xform_id, variation_id, 1u);
    let vsin = get_param(xform_id, variation_id, 2u);
    let vfcos = get_param(xform_id, variation_id, 3u);

    let d = dist - p.y * vsin;
    let safe_d = select(d, 1e-30, abs(d) < 1e-30);
    let t = 1.0 / safe_d;
    return vec2<f32>(dist * p.x * t, vfcos * p.y * t);
}
"#,
    wgsl_3d: r#"
fn variation_perspective(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let dist = get_param(xform_id, variation_id, 1u);
    let vsin = get_param(xform_id, variation_id, 2u);
    let vfcos = get_param(xform_id, variation_id, 3u);

    let d = dist - p.y * vsin;
    let safe_d = select(d, 1e-30, abs(d) < 1e-30);
    let t = 1.0 / safe_d;
    return vec3<f32>(dist * p.x * t, vfcos * p.y * t, p.z);
}
"#,
};

// =============================================================================
// line: project to a random spot on a 3D line (chronologicaldot)
//   2 user params (delta, phi) — Java-recovered; cpp omitted both
//   Unit vector u = (cos(δπ)·cos(φπ), sin(δπ)·cos(φπ), sin(φπ))
//   (drop normalization — |u| ≡ 1 by construction)
//   r = rand · w
//   out = u · r
// Body has internal w in `r` — needs_transform divide-out.
// Full3D (writes z explicitly).
// =============================================================================
/// Project to a random spot on a 3D line — picks a random distance `r =
/// rand · w` and emits along a unit vector `(cos(δπ)·cos(φπ),
/// sin(δπ)·cos(φπ), sin(φπ))`. The 2D output drops the Z component.
///
/// # Authors
/// - chronologicaldot
pub static LINE: VariationDef = VariationDef {
    name: "line",
    aliases: &[],
    display_name: "Line",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::NeedsTransform, Feature::AlwaysZ],
    parameters: &[
        param!("delta", "Delta", unlimited_float, 0.0, -10.0, 10.0, "Azimuthal angle of the line direction (in units of π)."),
        param!("phi", "Phi", unlimited_float, 0.0, -10.0, 10.0, "Polar angle of the line direction (in units of π). When `δ = φ = 0`, the line projects onto the X axis."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_line(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let delta = get_param(xform_id, variation_id, 0u);
    let phi = get_param(xform_id, variation_id, 1u);
    let pi = 3.14159265358979;
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let cphi = cos(phi * pi);
    let ux = cos(delta * pi) * cphi;
    let uy = sin(delta * pi) * cphi;
    let r = rng_nextf(rng) * w;
    return vec2<f32>(ux * r * inv_w, uy * r * inv_w);
}
"#,
    wgsl_3d: r#"
fn variation_line(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let delta = get_param(xform_id, variation_id, 0u);
    let phi = get_param(xform_id, variation_id, 1u);
    let pi = 3.14159265358979;
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let cphi = cos(phi * pi);
    let ux = cos(delta * pi) * cphi;
    let uy = sin(delta * pi) * cphi;
    let uz = sin(phi * pi);
    let r = rng_nextf(rng) * w;
    return vec3<f32>(ux * r * inv_w, uy * r * inv_w, uz * r * inv_w);
}
"#,
};

// =============================================================================
// holesq: square hole (dark-beam)
//   x = w · x_in;  y = w · y_in
//   if |x| + |y| > 1:    out = (x, y)
//   else if |x| > |y|:
//     if x ≥ 0: t = (x - |y| + 1) / 2;  out = (t, y)
//     else:     t = (x + |y| - 1) / 2;  out = (t, y)
//   else:
//     if y ≥ 0: t = (y - |x| + 1) / 2;  out = (x, t)
//     else:     t = (y + |x| - 1) / 2;  out = (x, t)
// Body has additive ±1 inside the t formulas not scaled by VVAR —
// needs_transform divide-out (the offsets become ±1/(2w) in the
// stripped result; outer × w restores ±1/2).
// =============================================================================
/// Square hole — outside the square `|x| + |y| > 1` the input passes
/// through; inside, the dominant-axis coordinate gets folded toward `±0.5`
/// based on its sign and the magnitude of the perpendicular coordinate.
/// Produces a square-hole-shaped scatter pattern.
///
/// # Authors
/// - DarkBeam
pub static HOLESQ: VariationDef = VariationDef {
    name: "holesq",
    aliases: &[],
    display_name: "Hole Square",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsTransform],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_holesq(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let x = w * p.x;
    let y = w * p.y;
    let fax = abs(x);
    let fay = abs(y);
    var fx: f32;
    var fy: f32;

    if (fax + fay > 1.0) {
        fx = x;
        fy = y;
    } else if (fax > fay) {
        var t: f32;
        if (x < 0.0) {
            t = (x + fay - 1.0) * 0.5;
        } else {
            t = (x - fay + 1.0) * 0.5;
        }
        fx = t;
        fy = y;
    } else {
        var t: f32;
        if (y < 0.0) {
            t = (y + fax - 1.0) * 0.5;
        } else {
            t = (y - fax + 1.0) * 0.5;
        }
        fx = x;
        fy = t;
    }
    return vec2<f32>(fx * inv_w, fy * inv_w);
}
"#,
    wgsl_3d: r#"
fn variation_holesq(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let x = w * p.x;
    let y = w * p.y;
    let fax = abs(x);
    let fay = abs(y);
    var fx: f32;
    var fy: f32;

    if (fax + fay > 1.0) {
        fx = x;
        fy = y;
    } else if (fax > fay) {
        var t: f32;
        if (x < 0.0) {
            t = (x + fay - 1.0) * 0.5;
        } else {
            t = (x - fay + 1.0) * 0.5;
        }
        fx = t;
        fy = y;
    } else {
        var t: f32;
        if (y < 0.0) {
            t = (y + fax - 1.0) * 0.5;
        } else {
            t = (y - fax + 1.0) * 0.5;
        }
        fx = x;
        fy = t;
    }
    return vec3<f32>(fx * inv_w, fy * inv_w, p.z);
}
"#,
};
