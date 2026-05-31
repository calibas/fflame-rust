//! Apophysis miscellany 9 — four more
//!
//!   - `eJulia`    (Faber)             — elliptic-coordinate Julia;
//!                                          1 user (power int) + 1
//!                                          init slot (sign); RNG;
//!                                          clean
//!   - `eMotion`   (Faber)             — elliptic-coordinate motion;
//!                                          2 user (move, rotate);
//!                                          clean
//!   - `flower_db` (CozyG / dark-beam) — flower with stem; 7 user
//!                                          params, Full3D output
//!                                          (writes z explicitly);
//!                                          needs_transform divide-out
//!                                          (output mixes w-scaled
//!                                          and constant terms).
//!                                          Preserves cpp's
//!                                          `atan2(x, y)` swap (vs
//!                                          Java's `atan2(y, x)`)
//!   - `julian2`   (Xyrus02)           — JuliaN with affine pre-transform
//!                                          (8 user: power int, dist,
//!                                          a..f) + 2 init slots
//!                                          (absN, cN); RNG; clean.
//!                                          cpp's
//!                                          `GOODRAND_0X(INT_MAX) %
//!                                          absN` replaced with
//!                                          `floor(rand · absN)`
//!                                          (uniform N-th-root branch
//!                                          without bias)
//!
//! Sources: each variation's `.cpp` file in
//! `output/jwildfire-vars/output/`.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// eJulia (Faber)
//   1 user param (power int, default 2)
//   1 init slot: sign = (power < 0) ? -1 : 1
//   if sign == 1: x = X            else: r2 = 1/r2; x = X · r2
//   tmp = r2 + 1;  tmp2 = 2x
//   xmax = (sqrt(max(tmp+tmp2, 0)) + sqrt(max(tmp-tmp2, 0))) / 2
//   if xmax < 1: xmax = 1
//   mu = acosh(xmax)
//   t = clamp(x / xmax, -1, 1);  nu = acos(t)
//   if y < 0: nu = -nu
//   nu = nu/power + 2π/power · floor(rand · power)
//   mu /= power
//   out = (cosh(mu)·cos(nu), sinh(mu)·sin(nu))
// Clean factor through outer.
// =============================================================================
/// Elliptic-coordinate Julia — converts the input to elliptic coordinates
/// `(μ, ν)`, divides both by `power`, adds a random angular branch
/// `2π·floor(rand·|power|)/power` to ν, then emits `(cosh(μ)·cos(ν),
/// sinh(μ)·sin(ν))`. With negative `power`, the input is first inverted
/// through the unit circle. Produces N-fold rotationally symmetric Julia-
/// style elliptic patterns.
///
/// # Authors
/// - Michael Faber
pub static EJULIA: VariationDef = VariationDef {
    name: "eJulia",
    aliases: &[],
    display_name: "E-Julia",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("power", "Power", int, 2.0, -100.0, 100.0, "Number of angular branches. Negative values invert the input through the unit circle first."),
    ],
    needs_transform: false,
    writes_color: false,
    // 1 derived value at slot 1: sign
    init_param_count: 1,
    wgsl_init: Some(r#"
fn init_eJulia(user: array<f32, 1>) -> array<f32, 1> {
    var out: array<f32, 1>;
    out[0] = select(1.0, -1.0, user[0] < 0.0);
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_eJulia(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let power = get_param(xform_id, variation_id, 0u);
    let sign = get_param(xform_id, variation_id, 1u);
    let two_pi = 6.28318530717959;
    let safe_power = select(power, 1.0, abs(power) < 1e-30);
    let abs_power = max(abs(safe_power), 1.0);

    var r2 = p.y * p.y + p.x * p.x;
    var x: f32;
    if (sign == 1.0) {
        x = p.x;
    } else {
        r2 = 1.0 / max(r2, 1e-30);
        x = p.x * r2;
    }
    let tmp = r2 + 1.0;
    let tmp2 = 2.0 * x;
    let xmax_raw = (sqrt(max(tmp + tmp2, 0.0)) + sqrt(max(tmp - tmp2, 0.0))) * 0.5;
    let xmax = max(xmax_raw, 1.0);
    let mu = acosh(xmax) / safe_power;
    let t = clamp(x / xmax, -1.0, 1.0);
    var nu = acos(t);
    if (p.y < 0.0) {
        nu = -nu;
    }
    nu = nu / safe_power + two_pi / safe_power * floor(rng_nextf(rng) * abs_power);
    return vec2<f32>(cosh(mu) * cos(nu), sinh(mu) * sin(nu));
}
"#,
    wgsl_3d: r#"
fn variation_eJulia(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let power = get_param(xform_id, variation_id, 0u);
    let sign = get_param(xform_id, variation_id, 1u);
    let two_pi = 6.28318530717959;
    let safe_power = select(power, 1.0, abs(power) < 1e-30);
    let abs_power = max(abs(safe_power), 1.0);

    var r2 = p.y * p.y + p.x * p.x;
    var x: f32;
    if (sign == 1.0) {
        x = p.x;
    } else {
        r2 = 1.0 / max(r2, 1e-30);
        x = p.x * r2;
    }
    let tmp = r2 + 1.0;
    let tmp2 = 2.0 * x;
    let xmax_raw = (sqrt(max(tmp + tmp2, 0.0)) + sqrt(max(tmp - tmp2, 0.0))) * 0.5;
    let xmax = max(xmax_raw, 1.0);
    let mu = acosh(xmax) / safe_power;
    let t = clamp(x / xmax, -1.0, 1.0);
    var nu = acos(t);
    if (p.y < 0.0) {
        nu = -nu;
    }
    nu = nu / safe_power + two_pi / safe_power * floor(rng_nextf(rng) * abs_power);
    return vec3<f32>(cosh(mu) * cos(nu), sinh(mu) * sin(nu), p.z);
}
"#,
};

// =============================================================================
// eMotion (Faber)
//   2 user params: move, rotate
//   xmax computed as in eJulia (no sign branch)
//   mu = acosh(max(xmax, 1));  t = clamp(x/xmax, -1, 1);  nu = acos(t)
//   if y < 0: nu = -nu
//   if nu < 0: mu += move else: mu -= move
//   if mu <= 0: mu = -mu; nu = -nu
//   nu += rotate
//   out = (cosh(mu)·cos(nu), sinh(mu)·sin(nu))
// Clean factor through outer.
// =============================================================================
/// Elliptic-coordinate motion — converts the input to elliptic coordinates
/// `(μ, ν)`, adds `move` to μ (with sign determined by ν), folds μ negative
/// back onto positive (reflecting ν), then adds `rotate` to ν.
///
/// # Authors
/// - Michael Faber
pub static EMOTION: VariationDef = VariationDef {
    name: "eMotion",
    aliases: &[],
    display_name: "E-Motion",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("move", "Move", unlimited_float, 0.0, -10.0, 10.0, "Additive offset on μ (sign chosen by the sign of ν)."),
        param!("rotate", "Rotate", unlimited_float, 0.0, -10.0, 10.0, "Additive offset on ν."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_eMotion(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let move_p = get_param(xform_id, variation_id, 0u);
    let rotate = get_param(xform_id, variation_id, 1u);

    let tmp = p.y * p.y + p.x * p.x + 1.0;
    let tmp2 = 2.0 * p.x;
    let xmax_raw = (sqrt(max(tmp + tmp2, 0.0)) + sqrt(max(tmp - tmp2, 0.0))) * 0.5;
    let xmax = max(xmax_raw, 1.0);
    var mu = acosh(xmax);
    let t = clamp(p.x / xmax, -1.0, 1.0);
    var nu = acos(t);
    if (p.y < 0.0) {
        nu = -nu;
    }
    if (nu < 0.0) {
        mu = mu + move_p;
    } else {
        mu = mu - move_p;
    }
    if (mu <= 0.0) {
        mu = -mu;
        nu = -nu;
    }
    nu = nu + rotate;
    return vec2<f32>(cosh(mu) * cos(nu), sinh(mu) * sin(nu));
}
"#,
    wgsl_3d: r#"
fn variation_eMotion(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let move_p = get_param(xform_id, variation_id, 0u);
    let rotate = get_param(xform_id, variation_id, 1u);

    let tmp = p.y * p.y + p.x * p.x + 1.0;
    let tmp2 = 2.0 * p.x;
    let xmax_raw = (sqrt(max(tmp + tmp2, 0.0)) + sqrt(max(tmp - tmp2, 0.0))) * 0.5;
    let xmax = max(xmax_raw, 1.0);
    var mu = acosh(xmax);
    let t = clamp(p.x / xmax, -1.0, 1.0);
    var nu = acos(t);
    if (p.y < 0.0) {
        nu = -nu;
    }
    if (nu < 0.0) {
        mu = mu + move_p;
    } else {
        mu = mu - move_p;
    }
    if (mu <= 0.0) {
        mu = -mu;
        nu = -nu;
    }
    nu = nu + rotate;
    return vec3<f32>(cosh(mu) * cos(nu), sinh(mu) * sin(nu), p.z);
}
"#,
};

// =============================================================================
// flower_db (CozyG, original dark-beam)
//   r = w · sqrt(x² + y²)
//   t = atan2(y, x)
//   r *= |spread + sin(petals · t)| · cos(petal_split · petals · t)
//   fx = sin(t) · r
//   fy = cos(t) · r
//   fz = -stem_thickness · (2/r - 1)
//   rnew = sqrt(fx² + fy²)
//   if rnew > petal_fold_radius:
//       fz += (rnew - petal_fold_radius) · petal_fold_strength
//   if stem_length != 0 && fz <= -stem_length:
//       fz = -stem_length
// 7 user params; Full3D; needs_transform divide-out.
//
// Matches JWildfire `FlowerDbFunc.java` (`t = pAffineTP.getPrecalcAtanYX()`,
// i.e. standard `atan2(y, x)`) and its GPU code (`t = __theta`, also
// standard). The cpp port had `atan2(x, y)` — converter-introduced swap,
// fixed here. Note: Fractorium has a separately-evolved simpler
// `flowerdb` (3 params, no stem/fold, `(r·cos t, r·sin t)` output) that
// is a different variation despite sharing dark-beam's attribution.
// =============================================================================
/// Flower with stem — emits a flower-shaped XY pattern (radius modulated by
/// `|spread + sin(petals·t)| · cos(petal_split·petals·t)`) plus a stem Z
/// that grows downward with thickness controlled by `stem_thickness`.
/// Optionally folds the petal Z upward outside a radius, and caps the stem
/// at a maximum length.
///
/// # Authors
/// - CozyG
/// - DarkBeam
pub static FLOWER_DB: VariationDef = VariationDef {
    name: "flower_db",
    aliases: &[],
    display_name: "Flower DB",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("petals", "Petals", unlimited_float, 6.0, -50.0, 50.0, "Number of petal lobes (angular frequency of the sin modulator)."),
        param!("petal_split", "Petal Split", unlimited_float, 0.0, -10.0, 10.0, "Frequency multiplier on the inner cosine that creates per-petal sub-divisions."),
        param!("petal_spread", "Petal Spread", unlimited_float, 1.0, -10.0, 10.0, "Constant offset added to the sin modulator. Controls how filled the petals appear."),
        param!("stem_thickness", "Stem Thickness", unlimited_float, 1.0, -10.0, 10.0, "Magnitude of the Z output. Negative values point the stem downward."),
        param!("stem_length", "Stem Length", unlimited_float, 0.0, -10.0, 10.0, "Maximum stem length. 0 disables the cap (stem extends without bound)."),
        param!("petal_fold_strength", "Petal Fold Strength", unlimited_float, 0.0, -10.0, 10.0, "Z-axis fold strength applied to petals outside `petal_fold_radius`."),
        param!("petal_fold_radius", "Petal Fold Radius", unlimited_float, 1.0, -10.0, 10.0, "Radial threshold beyond which the petal fold kicks in."),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_flower_db(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let petals = get_param(xform_id, variation_id, 0u);
    let petal_split = get_param(xform_id, variation_id, 1u);
    let petal_spread = get_param(xform_id, variation_id, 2u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    var r = w * sqrt(p.x * p.x + p.y * p.y);
    let t = atan2(p.y, p.x);
    r = r * abs((petal_spread + sin(petals * t)) * cos(petal_split * petals * t));
    let fx = sin(t) * r;
    let fy = cos(t) * r;
    return vec2<f32>(fx * inv_w, fy * inv_w);
}
"#,
    wgsl_3d: r#"
fn variation_flower_db(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let petals = get_param(xform_id, variation_id, 0u);
    let petal_split = get_param(xform_id, variation_id, 1u);
    let petal_spread = get_param(xform_id, variation_id, 2u);
    let stem_thickness = get_param(xform_id, variation_id, 3u);
    let stem_length = get_param(xform_id, variation_id, 4u);
    let petal_fold_strength = get_param(xform_id, variation_id, 5u);
    let petal_fold_radius = get_param(xform_id, variation_id, 6u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    var r = w * sqrt(p.x * p.x + p.y * p.y);
    let t = atan2(p.y, p.x);
    r = r * abs((petal_spread + sin(petals * t)) * cos(petal_split * petals * t));
    let safe_r = select(r, 1e-30, abs(r) < 1e-30);
    let fx = sin(t) * r;
    let fy = cos(t) * r;
    var fz = -stem_thickness * (2.0 / safe_r - 1.0);
    let rnew = sqrt(fx * fx + fy * fy);
    if (rnew > petal_fold_radius) {
        fz = fz + (rnew - petal_fold_radius) * petal_fold_strength;
    }
    if (stem_length != 0.0 && fz <= -stem_length) {
        fz = -stem_length;
    }
    return vec3<f32>(fx * inv_w, fy * inv_w, fz * inv_w);
}
"#,
};

// =============================================================================
// julian2 (Xyrus02)
//   8 user params: power int, dist, a, b, c, d, e, f
//   2 init slots: absN = |power|;  cN = dist / (2·power)
//   if power == 0: pass-through
//   X = a·x + b·y + e;  Y = c·x + d·y + f
//   angle = (atan2(Y, X) + 2π · floor(rand · absN)) / power
//   r = w · pow(X² + Y², cN)
//   out = r · (cos angle, sin angle)
// Clean factor through outer.
// =============================================================================
/// JuliaN with affine pre-transform — applies a 2D affine `(X, Y) = (a·x +
/// b·y + e, c·x + d·y + f)` first, then runs standard JuliaN: picks a
/// random angular branch from `|power|`, computes the new angle `(atan2(Y,
/// X) + 2π·k)/power`, and emits at radius `(X²+Y²)^(dist/(2·power))`.
///
/// # Authors
/// - Xyrus02
pub static JULIAN2: VariationDef = VariationDef {
    name: "julian2",
    aliases: &[],
    display_name: "JuliaN 2",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("power", "Power", int, 0.0, -100.0, 100.0, "Number of angular branches. 0 produces zero output."),
        param!("dist", "Distance", unlimited_float, 1.0, -100.0, 100.0, "Radial-power exponent — output radius is `(X²+Y²)^(dist/(2·power))`."),
        param!("a", "A", unlimited_float, 1.0, -10.0, 10.0, "Pre-affine xx coefficient."),
        param!("b", "B", unlimited_float, 0.0, -10.0, 10.0, "Pre-affine xy coefficient."),
        param!("c", "C", unlimited_float, 0.0, -10.0, 10.0, "Pre-affine yx coefficient."),
        param!("d", "D", unlimited_float, 1.0, -10.0, 10.0, "Pre-affine yy coefficient."),
        param!("e", "E", unlimited_float, 0.0, -10.0, 10.0, "Pre-affine x offset."),
        param!("f", "F", unlimited_float, 0.0, -10.0, 10.0, "Pre-affine y offset."),
    ],
    needs_transform: false,
    writes_color: false,
    // 2 derived values at slots 8..10:
    //   8: absN = |power|
    //   9: cN   = dist / (2·power)
    init_param_count: 2,
    wgsl_init: Some(r#"
fn init_julian2(user: array<f32, 8>) -> array<f32, 2> {
    let safe_power = select(user[0], 1.0, abs(user[0]) < 1e-30);
    var out: array<f32, 2>;
    out[0] = abs(safe_power);
    out[1] = user[1] / (2.0 * safe_power);
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_julian2(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let power = get_param(xform_id, variation_id, 0u);
    let a = get_param(xform_id, variation_id, 2u);
    let b = get_param(xform_id, variation_id, 3u);
    let c = get_param(xform_id, variation_id, 4u);
    let d = get_param(xform_id, variation_id, 5u);
    let e = get_param(xform_id, variation_id, 6u);
    let f = get_param(xform_id, variation_id, 7u);
    let absN = max(get_param(xform_id, variation_id, 8u), 1.0);
    let cN = get_param(xform_id, variation_id, 9u);
    let two_pi = 6.28318530717959;

    if (abs(power) < 1e-30) {
        return vec2<f32>(0.0, 0.0);
    }
    let x = a * p.x + b * p.y + e;
    let y = c * p.x + d * p.y + f;
    let angle = (atan2(y, x) + two_pi * floor(rng_nextf(rng) * absN)) / power;
    let r = pow(max(x * x + y * y, 1e-30), cN);
    return vec2<f32>(r * cos(angle), r * sin(angle));
}
"#,
    wgsl_3d: r#"
fn variation_julian2(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let power = get_param(xform_id, variation_id, 0u);
    let a = get_param(xform_id, variation_id, 2u);
    let b = get_param(xform_id, variation_id, 3u);
    let c = get_param(xform_id, variation_id, 4u);
    let d = get_param(xform_id, variation_id, 5u);
    let e = get_param(xform_id, variation_id, 6u);
    let f = get_param(xform_id, variation_id, 7u);
    let absN = max(get_param(xform_id, variation_id, 8u), 1.0);
    let cN = get_param(xform_id, variation_id, 9u);
    let two_pi = 6.28318530717959;

    if (abs(power) < 1e-30) {
        return vec3<f32>(0.0, 0.0, p.z);
    }
    let x = a * p.x + b * p.y + e;
    let y = c * p.x + d * p.y + f;
    let angle = (atan2(y, x) + two_pi * floor(rng_nextf(rng) * absN)) / power;
    let r = pow(max(x * x + y * y, 1e-30), cN);
    return vec3<f32>(r * cos(angle), r * sin(angle), p.z);
}
"#,
};
